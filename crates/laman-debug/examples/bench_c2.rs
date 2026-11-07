//! C2 baseline, including diagnostics. Run with `cargo run --release -p
//! laman-debug --example bench_c2`. Every row runs in a separate process,
//! bounded even during setup. Budget misses and timeouts are data, not an
//! exit failure. Triangle drags have no construction and are printed N/A.
//! A cell is over budget when its worst sample is, and a bisected frame is
//! measured per try (its time over `halvings + 1`).
use laman::model::{Constraint, Sketch};
use laman::profile::profiles;
use laman::solution::{Algorithm, SolverOptions};
use laman_debug::corpus::{
    Case, bisecting_drag, rect_chain, refused_drag, slot_grid, triangle_strip,
};
use std::io::{BufRead, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

// ROADMAP.md C2's budget per try, including diagnostics: a frame is one try, or
// one more than its halvings when it was bisected, and its time is divided.
const BUDGET: Duration = Duration::from_millis(4);
// ADR-0014's number for the profile cells at 500 entities: extraction is not
// a frame step, so its cells are read against this, not the 4 ms.
const PROFILE_BUDGET: Duration = Duration::from_micros(1500);
// Slow first samples already establish a large miss; do not repeat them.
const FIRST_LIMIT: Duration = Duration::from_millis(100);
// Five samples give a median without making a baseline a prolonged load test.
const RUNS: usize = 5;
// Includes construction, initial solve/session opening, and all timed samples.
// At most 62 workers (52 solver cells, 10 profile cells): the entire run takes
// at most 310 s plus process overhead.
const WORKER_LIMIT: Duration = Duration::from_secs(5);
const FAMILIES: [&str; 3] = ["rectangles", "slots", "triangles"];
const OPERATIONS: [&str; 5] = ["static", "edit", "drag", "bisect", "bisect-deep"];
// Profile extraction is timed beside them: the three families solved once, and
// a row of disjoint circles built for the purpose (family 3), the shape the
// quadratic containment is worst on (ADR-0014).
const CIRCLE_ROW: usize = 249;
const COUNTS: [[usize; 3]; 3] = [[98, 248, 498], [98, 248, 498], [99, 249, 501]];

type Error = Box<dyn std::error::Error>;

fn case(family: usize, size: usize) -> Result<Case, Error> {
    Ok(match family {
        0 => rect_chain([19, 49, 99][size])?,
        1 => {
            let (rows, cols) = [(3, 3), (4, 6), (7, 7)][size];
            slot_grid(rows, cols)?
        }
        2 => triangle_strip([32, 82, 166][size])?,
        _ => return Err("unknown family".into()),
    })
}

fn signal(message: &str) -> Result<(), Error> {
    println!("{message}");
    std::io::stdout().flush()?;
    Ok(())
}

/// Times `profiles` at the solved coordinates of a family (or the circle row).
/// The solve is outside the timed operation.
fn profiles_worker(family: usize, size: usize, samples: usize) -> Result<(), Error> {
    let (sketch, values) = if family == FAMILIES.len() {
        let mut sketch = Sketch::new();
        for i in 0..CIRCLE_ROW {
            let c = sketch.point(3.0 * i as f64, 0.0)?;
            sketch.circle(c, 1.0)?;
        }
        let values = sketch.coordinates();
        (sketch, values)
    } else {
        let sketch = case(family, size)?.sketch;
        let solved = sketch.solve(&SolverOptions::default());
        (sketch, solved.coordinates)
    };
    for _ in 0..samples {
        let start = Instant::now();
        signal("timing")?;
        let found = std::hint::black_box(profiles(&sketch, &values));
        let took = start.elapsed();
        signal(&format!("sample {} 1", took.as_nanos()))?;
        if found.is_err() {
            return Err("the profiles cell's extraction was refused".into());
        }
        if took > FIRST_LIMIT {
            break;
        }
    }
    Ok(())
}

fn worker(
    family: usize,
    size: usize,
    algorithm: Algorithm,
    operation: &str,
    samples: usize,
) -> Result<(), Error> {
    let case = case(family, size)?;
    if case.sketch.entities().count() != COUNTS[family][size] {
        return Err("construction entity count differs from benchmark recipe".into());
    }
    let mut options = SolverOptions::default();
    options.algorithm = algorithm;
    // The edited sketch is built once; each sample solves a clone of it, so a
    // profile of many samples holds the edit's solve and not its set-up.
    let mut prepared = case.sketch.clone();
    if operation == "edit" {
        let initial = prepared.solve(&options);
        prepared.apply(&initial)?;
        let (id, dimension) = prepared
            .constraints()
            .find(|(id, _)| prepared.constraint_tag_of(*id) == Some(case.edit.tag.as_str()))
            .map(|(id, value)| (id, *value))
            .ok_or("missing edit dimension")?;
        let replacement = if let Constraint::Distance { p, q, .. } = dimension {
            Constraint::Distance {
                p,
                q,
                value: case.edit.value,
            }
        } else if let Constraint::PointLineDistance { p, line, sense, .. } = dimension {
            Constraint::PointLineDistance {
                p,
                line,
                sense,
                value: case.edit.value,
            }
        } else {
            return Err("edit dimension has unsupported kind".into());
        };
        prepared.remove_constraint(id)?;
        prepared.add_constraint_with_id(id, replacement)?;
        prepared.tag_constraint(id, &case.edit.tag)?;
    }
    for _ in 0..samples {
        // Re-create each warm start outside the measured operation.
        let edited = prepared.clone();
        let mut session = None;
        let mut bisect_target = None;
        let mut rest = None;
        if operation == "bisect" {
            let (sketch, dragged, target) = bisecting_drag(&case)?.ok_or("no bisecting drag")?;
            session = Some(sketch.drag(dragged, &options)?);
            bisect_target = Some(target);
        } else if operation == "bisect-deep" {
            let (sketch, dragged, target) = refused_drag(&case)?.ok_or("no refused drag")?;
            rest = Some(sketch.coordinates());
            session = Some(sketch.drag(dragged, &options)?);
            bisect_target = Some(target);
        } else if operation == "drag" {
            let drag = case.drag.as_ref().ok_or("no constructed drag")?;
            session = Some(drag.sketch.drag(drag.drag.dragged, &options)?);
        }
        // Include the tiny marker flush so parent-received time is a true
        // lower bound on this interval, even at the timeout boundary.
        let start = Instant::now();
        signal("timing")?;
        let mut tries = 1;
        if let Some(mut session) = session {
            if let Some(target) = bisect_target {
                let frame = std::hint::black_box(session.to(target));
                // The cell times a bisected frame; one that is not is a bug.
                let Some(bisection) = frame.diagnostics.bisection else {
                    return Err("the bisect cell's first frame was not bisected".into());
                };
                tries = bisection.halvings + 1;
                // The deep cell times a frame refused at every halving.
                if let Some(rest) = &rest
                    && (bisection.halvings != options.max_bisections || frame.coordinates != *rest)
                {
                    return Err(
                        "the bisect-deep cell's frame was not refused at every halving".into(),
                    );
                }
            } else {
                let target = case
                    .drag
                    .as_ref()
                    .and_then(|d| d.drag.targets.first())
                    .ok_or("no drag target")?;
                std::hint::black_box(session.to(*target));
            }
        } else {
            std::hint::black_box(edited.solve(&options));
        }
        let took = start.elapsed();
        signal(&format!("sample {} {tries}", took.as_nanos()))?;
        if took > FIRST_LIMIT {
            break;
        }
    }
    Ok(())
}

#[derive(Default)]
struct Measurement {
    /// Each sample's time and its tries.
    samples: Vec<(Duration, u32)>,
    timing_since: Option<Instant>,
}

impl Measurement {
    fn receive(&mut self, line: &str) -> Result<(), Error> {
        if line == "timing" {
            self.timing_since = Some(Instant::now());
        } else if let Some(sample) = line.strip_prefix("sample ") {
            let (nanos, tries) = sample.split_once(' ').unwrap_or((sample, "1"));
            self.samples
                .push((Duration::from_nanos(nanos.parse()?), tries.parse()?));
            self.timing_since = None;
        } else {
            return Err(format!("invalid worker output: {line}").into());
        }
        Ok(())
    }

    fn timeout(&self) -> String {
        if let Some(since) = self.timing_since {
            // Receiving the marker happens after the worker started: this
            // elapsed time is a conservative lower bound, never a sample.
            format!(
                "over; timed operation >= {:.3} ms; no completed median/worst ({} samples)",
                since.elapsed().as_secs_f64() * 1000.0,
                self.samples.len()
            )
        } else {
            format!(
                "setup timeout; no timing verdict; no completed median/worst ({} samples)",
                self.samples.len()
            )
        }
    }

    fn completed(mut self, budget: Duration) -> Result<String, Error> {
        if self.timing_since.is_some() || self.samples.is_empty() {
            return Err("worker ended without a completed sample".into());
        }
        // The budget is per try; a sample is its frame's time over its tries.
        let per_try = |(took, tries): &(Duration, u32)| *took / (*tries).max(1);
        self.samples.sort_by_key(per_try);
        let count = self.samples.len();
        let median = per_try(&self.samples[count / 2]);
        let worst = per_try(&self.samples[count - 1]);
        let tries = self.samples[count - 1].1;
        let mut report = format!(
            "{}; median {:.3} ms, worst {:.3} ms (n={count}{})",
            if worst > budget { "over" } else { "under" },
            median.as_secs_f64() * 1000.0,
            worst.as_secs_f64() * 1000.0,
            if count == 1 {
                "; first run exceeds repeat limit"
            } else {
                ""
            }
        );
        if tries > 1 {
            report += &format!(
                "; per try of {tries} tries (the worst frame {:.3} ms)",
                self.samples[count - 1].0.as_secs_f64() * 1000.0
            );
        }
        Ok(report)
    }
}

fn monitor(mut child: Child, limit: Duration, budget: Duration) -> Result<String, Error> {
    let stdout = child.stdout.take().ok_or("worker stdout missing")?;
    let (send, receive) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout).lines() {
            if send.send(line).is_err() {
                break;
            }
        }
    });
    let deadline = Instant::now() + limit;
    let mut measured = Measurement::default();
    let result = loop {
        match receive.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(line) => {
                if let Err(error) = line
                    .map_err(Error::from)
                    .and_then(|line| measured.receive(&line))
                {
                    break Err(error);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => break Ok(measured.timeout()),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break if child.wait()?.success() {
                    measured.completed(budget)
                } else {
                    Err("worker failed".into())
                };
            }
        }
    };
    // Also reap completed workers. A timeout never leaves a solver running.
    let _ = child.kill();
    let _ = child.wait();
    let _ = reader.join();
    result
}

fn cpu() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|text| {
            text.lines().find_map(|line| {
                line.strip_prefix("model name")
                    .and_then(|s| s.split_once(':'))
                    .map(|(_, name)| name.trim().to_owned())
            })
        })
        .unwrap_or_else(|| "unknown CPU".into())
}

fn run() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        // `--worker F S ALGO OP [--samples N]`: N defaults to RUNS; a profile
        // asks for more.
        let samples = match args.get(5..) {
            Some([]) | None => RUNS,
            Some([flag, n]) if flag == "--samples" => n.parse()?,
            Some(_) => return Err("invalid worker arguments".into()),
        };
        if args.len() < 5 || args[0] != "--worker" {
            return Err("invalid worker arguments".into());
        }
        if args[4] == "profiles" {
            return profiles_worker(args[1].parse()?, args[2].parse()?, samples);
        }
        let family: usize = args[1].parse()?;
        let size: usize = args[2].parse()?;
        if family >= FAMILIES.len() || size >= 3 || !OPERATIONS.contains(&args[4].as_str()) {
            return Err("unknown worker case".into());
        }
        let algorithm = match args[3].as_str() {
            "LM" => Algorithm::LevenbergMarquardt,
            "DogLeg" => Algorithm::DogLeg,
            _ => return Err("unknown algorithm".into()),
        };
        return worker(family, size, algorithm, &args[4], samples);
    }
    println!(
        "machine: {} ({} {}, {} logical CPUs)",
        cpu(),
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::thread::available_parallelism().map_or(1, usize::from)
    );
    println!(
        "budget: {BUDGET:?} (profiles {PROFILE_BUDGET:?}); repeat limit: {FIRST_LIMIT:?}; samples: {RUNS}; worker wall limit including setup: {WORKER_LIMIT:?}"
    );
    if cfg!(debug_assertions) {
        println!("warning: use --release for the baseline");
    }
    let executable = std::env::current_exe()?;
    for family in 0..FAMILIES.len() {
        for (size, entities) in COUNTS[family].iter().enumerate() {
            for algorithm in ["LM", "DogLeg"] {
                for operation in OPERATIONS {
                    let result = if family == 2 && operation == "drag" {
                        "N/A; no constructed drag".to_owned()
                    } else if operation.starts_with("bisect") && (family != 1 || size != 2) {
                        continue;
                    } else {
                        let child = Command::new(&executable)
                            .args([
                                "--worker",
                                &family.to_string(),
                                &size.to_string(),
                                algorithm,
                                operation,
                            ])
                            .stdout(Stdio::piped())
                            .spawn()?;
                        monitor(child, WORKER_LIMIT, BUDGET)?
                    };
                    println!(
                        "{} {} {algorithm} {operation}: {result}",
                        FAMILIES[family], entities
                    );
                }
            }
        }
    }
    // Extraction does not depend on the algorithm: one cell a family and size.
    let mut cells: Vec<(usize, usize, &str, usize)> = Vec::new();
    for (family, name) in FAMILIES.iter().enumerate() {
        for (size, entities) in COUNTS[family].iter().enumerate() {
            cells.push((family, size, name, *entities));
        }
    }
    cells.push((FAMILIES.len(), 2, "circles", 2 * CIRCLE_ROW));
    for (family, size, name, entities) in cells {
        let child = Command::new(&executable)
            .args([
                "--worker",
                &family.to_string(),
                &size.to_string(),
                "-",
                "profiles",
            ])
            .stdout(Stdio::piped())
            .spawn()?;
        println!(
            "{name} {entities} profiles: {}",
            monitor(child, WORKER_LIMIT, PROFILE_BUDGET)?
        );
    }
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("bench_c2: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    // The helper blocks without creating descendants, so killing it also
    // closes stdout. Exercise the real process deadline, not a solver speed.
    fn blocked(script: &str) -> String {
        let child = Command::new("sh")
            .args(["-c", script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        monitor(child, Duration::from_millis(50), BUDGET).unwrap()
    }
    #[test]
    fn deadlines_distinguish_setup_and_timed_operations() {
        assert!(blocked("read x").starts_with("setup timeout; no timing verdict"));
        assert!(blocked("echo timing; read x").starts_with("over; timed operation >="));
    }
    #[test]
    fn slow_first_sample_is_a_single_observation() {
        let mut measurement = Measurement::default();
        measurement.receive("timing").unwrap();
        measurement.receive("sample 200000000 1").unwrap();
        let report = measurement.completed(BUDGET).unwrap();
        assert!(report.contains("median 200.000 ms, worst 200.000 ms (n=1"));
        assert!(report.starts_with("over;"));
    }
    #[test]
    fn a_bisected_frame_is_judged_per_try() {
        let mut measurement = Measurement::default();
        // 27 ms over 9 tries is 3 ms a try: under; the same frame as one try is over.
        measurement.receive("timing").unwrap();
        measurement.receive("sample 27000000 9").unwrap();
        let report = measurement.completed(BUDGET).unwrap();
        assert!(report.starts_with("under;"), "{report}");
        assert!(
            report.contains("worst 3.000 ms") && report.contains("9 tries"),
            "{report}"
        );
        let mut measurement = Measurement::default();
        measurement.receive("timing").unwrap();
        measurement.receive("sample 5000000 1").unwrap();
        // The verdict is the worst sample's, not the median's.
        measurement.receive("timing").unwrap();
        measurement.receive("sample 1000000 1").unwrap();
        measurement.receive("timing").unwrap();
        measurement.receive("sample 1000000 1").unwrap();
        assert!(measurement.completed(BUDGET).unwrap().starts_with("over;"));
    }
}
