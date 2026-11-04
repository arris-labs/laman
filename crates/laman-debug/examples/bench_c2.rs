//! C2 baseline, including diagnostics. Run with `cargo run --release -p
//! laman-debug --example bench_c2`. Every row runs in a separate process,
//! bounded even during setup. Budget misses and timeouts are data, not an
//! exit failure. Triangle drags have no construction and are printed N/A.
use laman::model::{Constraint, Coordinate, Entity, Sketch};
use laman::solution::{Algorithm, SolverOptions};
use laman_debug::corpus::{Case, rect_chain, slot_grid, triangle_strip};
use std::io::{BufRead, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

// ROADMAP.md C2's per-frame budget, including diagnostics.
const BUDGET: Duration = Duration::from_millis(4);
// Slow first samples already establish a large miss; do not repeat them.
const FIRST_LIMIT: Duration = Duration::from_millis(100);
// Five samples give a median without making a baseline a prolonged load test.
const RUNS: usize = 5;
// Includes construction, initial solve/session opening, and all timed samples.
// At most 48 workers: the entire run takes at most 240 s plus process overhead.
const WORKER_LIMIT: Duration = Duration::from_secs(5);
const FAMILIES: [&str; 3] = ["rectangles", "slots", "triangles"];
const OPERATIONS: [&str; 4] = ["static", "edit", "drag", "bisect"];
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

/// The slot grid's drag sketch with the middle slot's left arc let loose of
/// its tangent lines, and the target that turns the arc's start a quarter
/// turn about its centre: the arc's sweep moves by a quarter turn (π/2),
/// more than `MAX_SWEEP_STEP`, so the first frame is bisected
/// (plans/m7-drag-conflicts step 10).
fn bisecting_drag(case: &Case) -> Result<(Sketch, laman::model::EntityId, [f64; 2]), Error> {
    let drag = case.drag.as_ref().ok_or("no constructed drag")?;
    let mut sketch = drag.sketch.clone();
    let tag = format!("slot-{}-left", sketch.entities().count() / 10 / 2);
    let tagged = sketch
        .entities()
        .find(|(id, _)| sketch.tag_of(*id) == Some(tag.as_str()))
        .ok_or("missing slot arc")?;
    let (arc, (c, s)) = match tagged {
        (id, Entity::Arc { c, s, .. }) => (id, (*c, *s)),
        _ => return Err("slot tag is not an arc".into()),
    };
    let tangents: Vec<_> = sketch
        .constraints()
        .filter(|(_, constraint)| {
            matches!(constraint, Constraint::TangentLineCircle { round, .. } if round.id() == arc)
        })
        .map(|(id, _)| id)
        .collect();
    for id in tangents {
        sketch.remove_constraint(id)?;
    }
    let at = |p: laman::model::Point, axis| drag.expected.coordinates[&(p.id(), axis)];
    let (cx, cy) = (at(c, Coordinate::X), at(c, Coordinate::Y));
    let (dx, dy) = (at(s, Coordinate::X) - cx, at(s, Coordinate::Y) - cy);
    Ok((sketch, s.id(), [cx + dy, cy - dx]))
}

fn signal(message: &str) -> Result<(), Error> {
    println!("{message}");
    std::io::stdout().flush()?;
    Ok(())
}

fn worker(family: usize, size: usize, algorithm: Algorithm, operation: &str) -> Result<(), Error> {
    let case = case(family, size)?;
    if case.sketch.entities().count() != COUNTS[family][size] {
        return Err("construction entity count differs from benchmark recipe".into());
    }
    let mut options = SolverOptions::default();
    options.algorithm = algorithm;
    for _ in 0..RUNS {
        // Re-create each warm start outside the measured operation.
        let mut edited = case.sketch.clone();
        let mut session = None;
        let mut bisect_target = None;
        if operation == "edit" {
            let initial = edited.solve(&options);
            edited.apply(&initial)?;
            let (id, dimension) = edited
                .constraints()
                .find(|(id, _)| edited.constraint_tag_of(*id) == Some(case.edit.tag.as_str()))
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
            edited.remove_constraint(id)?;
            edited.add_constraint_with_id(id, replacement)?;
            edited.tag_constraint(id, &case.edit.tag)?;
        } else if operation == "bisect" {
            let (sketch, dragged, target) = bisecting_drag(&case)?;
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
        if let Some(mut session) = session {
            if let Some(target) = bisect_target {
                let frame = std::hint::black_box(session.to(target));
                // The cell times a bisected frame; one that is not is a bug.
                if frame.diagnostics.bisection.is_none() {
                    return Err("the bisect cell's first frame was not bisected".into());
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
        signal(&format!("sample {}", took.as_nanos()))?;
        if took > FIRST_LIMIT {
            break;
        }
    }
    Ok(())
}

#[derive(Default)]
struct Measurement {
    samples: Vec<Duration>,
    timing_since: Option<Instant>,
}

impl Measurement {
    fn receive(&mut self, line: &str) -> Result<(), Error> {
        if line == "timing" {
            self.timing_since = Some(Instant::now());
        } else if let Some(nanos) = line.strip_prefix("sample ") {
            self.samples.push(Duration::from_nanos(nanos.parse()?));
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

    fn completed(mut self) -> Result<String, Error> {
        if self.timing_since.is_some() || self.samples.is_empty() {
            return Err("worker ended without a completed sample".into());
        }
        self.samples.sort();
        let count = self.samples.len();
        let median = self.samples[count / 2];
        let worst = self.samples[count - 1];
        Ok(format!(
            "{}; median {:.3} ms, worst {:.3} ms (n={count}{})",
            if median > BUDGET { "over" } else { "under" },
            median.as_secs_f64() * 1000.0,
            worst.as_secs_f64() * 1000.0,
            if count == 1 {
                "; first run exceeds repeat limit"
            } else {
                ""
            }
        ))
    }
}

fn monitor(mut child: Child, limit: Duration) -> Result<String, Error> {
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
                    measured.completed()
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
        if args.len() != 5 || args[0] != "--worker" {
            return Err("invalid worker arguments".into());
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
        return worker(family, size, algorithm, &args[4]);
    }
    println!(
        "machine: {} ({} {}, {} logical CPUs)",
        cpu(),
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::thread::available_parallelism().map_or(1, usize::from)
    );
    println!(
        "budget: {BUDGET:?}; repeat limit: {FIRST_LIMIT:?}; samples: {RUNS}; worker wall limit including setup: {WORKER_LIMIT:?}"
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
                    } else if operation == "bisect" && (family != 1 || size != 2) {
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
                        monitor(child, WORKER_LIMIT)?
                    };
                    println!(
                        "{} {} {algorithm} {operation}: {result}",
                        FAMILIES[family], entities
                    );
                }
            }
        }
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
        monitor(child, Duration::from_millis(50)).unwrap()
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
        measurement.receive("sample 200000000").unwrap();
        let report = measurement.completed().unwrap();
        assert!(report.contains("median 200.000 ms, worst 200.000 ms (n=1"));
        assert!(report.starts_with("over;"));
    }
}
