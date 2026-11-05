#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::*;
use crate::model::{ConstraintId, Point, Sketch};
use laman_debug::fixture::Fixture;
use proptest::prelude::*;
use std::path::PathBuf;

/// A part as the sources it holds and the parameters it owns.
type Shape = (Kind, Vec<Source>, Vec<Param>);

fn decompositions(sk: &Sketch) -> Vec<(Component, Decomposition, System<'_>)> {
    // `System` borrows the sketch; hand each component its own.
    System::new(sk)
        .components
        .iter()
        .map(|c| {
            let system = System::new(sk);
            (c.clone(), decompose(&system, c), system)
        })
        .collect()
}

fn shape(system: &System<'_>, d: &Decomposition) -> Vec<Shape> {
    d.parts
        .iter()
        .map(|p| {
            let mut sources: Vec<Source> =
                p.rows.iter().map(|&r| system.equations[r].source).collect();
            sources.dedup();
            (p.kind, sources, p.params.clone())
        })
        .collect()
}

fn c(n: u32) -> Source {
    Source::Constraint(ConstraintId(n))
}

/// The structural rank by a different route: augmenting paths from the
/// parameters' side, recursively.
fn reference_rank(system: &System<'_>, component: &Component, rows: &[usize]) -> usize {
    fn try_col(
        c: usize,
        by_col: &[Vec<usize>],
        taken: &mut [Option<usize>],
        seen: &mut [bool],
    ) -> bool {
        for &r in &by_col[c] {
            if seen[r] {
                continue;
            }
            seen[r] = true;
            if taken[r].is_none_or(|c2| try_col(c2, by_col, taken, seen)) {
                taken[r] = Some(c);
                return true;
            }
        }
        false
    }
    let mut cols = component.params.clone();
    cols.sort();
    let mut by_col: Vec<Vec<usize>> = vec![vec![]; cols.len()];
    for (k, &r) in rows.iter().enumerate() {
        for p in support(system, &system.equations[r]) {
            if let Ok(i) = cols.binary_search(&p) {
                by_col[i].push(k);
            }
        }
    }
    let mut taken = vec![None; rows.len()];
    (0..cols.len())
        .filter(|&i| try_col(i, &by_col, &mut taken, &mut vec![false; rows.len()]))
        .count()
}

/// What every decomposition promises.
fn check_invariants(system: &System<'_>, component: &Component, d: &Decomposition) {
    let rows: Vec<usize> = system
        .equations
        .iter()
        .enumerate()
        .filter(|(_, e)| component.sources.contains(&e.source))
        .map(|(i, _)| i)
        .collect();
    // A partition of the rows and of the parameters.
    let mut seen_rows: Vec<usize> = d.parts.iter().flat_map(|p| p.rows.clone()).collect();
    seen_rows.sort_unstable();
    assert_eq!(seen_rows, rows);
    let mut seen_params: Vec<Param> = d.parts.iter().flat_map(|p| p.params.clone()).collect();
    seen_params.sort();
    let mut params = component.params.clone();
    params.sort();
    assert_eq!(seen_params, params);
    // Each part reads only its own and earlier parts' parameters.
    for (i, part) in d.parts.iter().enumerate() {
        let allowed: BTreeSet<Param> = d.parts[..=i]
            .iter()
            .flat_map(|p| p.params.iter().copied())
            .collect();
        for &r in &part.rows {
            for p in support(system, &system.equations[r]) {
                assert!(
                    allowed.contains(&p) || !params.contains(&p),
                    "part {i} reads {p:?} of a later part"
                );
            }
        }
        match part.kind {
            Kind::Stage => {
                assert_eq!(part.rows.len(), part.params.len());
                assert_eq!(part.matched, part.rows.len());
            }
            Kind::Head => assert!(part.matched < part.rows.len()),
            Kind::Remainder => assert!(part.params.len() > part.matched),
        }
    }
    // A constraint's equations never split.
    let mut home: BTreeMap<Source, usize> = BTreeMap::new();
    for (i, part) in d.parts.iter().enumerate() {
        for &r in &part.rows {
            assert_eq!(*home.entry(system.equations[r].source).or_insert(i), i);
        }
    }
    // The matching has the structural rank.
    assert_eq!(d.parts.iter().map(|p| p.matched).sum::<usize>(), d.rank);
    assert_eq!(d.rank, reference_rank(system, component, &rows));
}

fn load(slug: &str) -> Sketch {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/c1")
        .join(slug);
    let fixture = Fixture::load(&path).unwrap();
    serde_json::from_str(&serde_json::to_string(&fixture.sketch).unwrap()).unwrap()
}

fn p(id: u32, coordinate: Coordinate) -> Param {
    (crate::model::EntityId(id), coordinate)
}

#[test]
fn sss_triangle_is_two_stages() {
    let sk = load("sss-triangle");
    let all = decompositions(&sk);
    assert_eq!(all.len(), 1);
    let (component, d, system) = &all[0];
    check_invariants(system, component, d);
    // horizontal(base) fixes a's y, base then its x (each alone: neither
    // reads the other's equation back); left and right together fix b.
    assert_eq!(
        shape(system, d),
        [
            (Kind::Stage, vec![c(0)], vec![p(1, Coordinate::Y)]),
            (Kind::Stage, vec![c(1)], vec![p(1, Coordinate::X)]),
            (
                Kind::Stage,
                vec![c(2), c(3)],
                vec![p(2, Coordinate::X), p(2, Coordinate::Y)]
            ),
        ]
    );
}

#[test]
fn the_redundant_twin_is_all_head() {
    let sk = load("sss-triangle-redundant");
    let all = decompositions(&sk);
    let (component, d, system) = &all[0];
    check_invariants(system, component, d);
    assert_eq!(d.parts.len(), 1);
    assert_eq!(d.parts[0].kind, Kind::Head);
    assert_eq!(d.parts[0].rows.len(), 5);
    assert_eq!(d.parts[0].params.len(), 4);
    assert_eq!(d.rank, 4);
}

#[test]
fn the_plate_is_stages_and_a_slot() {
    let sk = load("slotted-plate");
    let all = decompositions(&sk);
    assert_eq!(all.len(), 1);
    let (component, d, system) = &all[0];
    check_invariants(system, component, d);
    let arcs: Vec<Source> = system
        .equations
        .iter()
        .filter(|e| matches!(e.source, Source::Arc(_)))
        .map(|e| e.source)
        .collect();
    assert_eq!(arcs.len(), 2);
    let mut got: Vec<(Kind, Vec<Source>, usize)> = shape(system, d)
        .into_iter()
        .map(|(k, mut s, params)| {
            s.sort();
            (k, s, params.len())
        })
        .collect();
    got.sort();
    // By analysis (ROADMAP fixture 3), a stage being a strongly connected
    // set: bottom fixes P1's y and width its x; left fixes P3's x and height
    // its y; right then P2's x, top P2's y; each midpoint its point; the
    // symmetry, the slot length and the slot height place the two centres
    // (four equations, four parameters); and the arcs' intrinsic equations,
    // the four tangents, the equal radius and the radius fix the arcs'
    // ends (eight, eight).
    let mut slot: Vec<Source> = (8..=13).map(c).chain(arcs).collect();
    slot.sort();
    let mut want = vec![
        (Kind::Stage, vec![c(0)], 1),
        (Kind::Stage, vec![c(4)], 1),
        (Kind::Stage, vec![c(3)], 1),
        (Kind::Stage, vec![c(5)], 1),
        (Kind::Stage, vec![c(1)], 1),
        (Kind::Stage, vec![c(2)], 1),
        (Kind::Stage, vec![c(6)], 2),
        (Kind::Stage, vec![c(7)], 2),
        (Kind::Stage, vec![c(14), c(15), c(16)], 4),
        (Kind::Stage, slot, 8),
    ];
    want.sort();
    assert_eq!(got, want);
    assert_eq!(d.rank, 22);
}

#[test]
fn without_the_width_the_x_direction_is_the_remainder() {
    let sk = load("slotted-plate-drag");
    let all = decompositions(&sk);
    for (component, d, system) in &all {
        check_invariants(system, component, d);
    }
    // The stray point: each coordinate its own component, no equation,
    // nothing but a remainder.
    let strays: Vec<_> = all.iter().filter(|(c, ..)| c.sources.is_empty()).collect();
    assert_eq!(strays.len(), 2);
    for (_, q, _) in strays {
        assert_eq!(q.parts.len(), 1);
        assert_eq!(q.parts[0].kind, Kind::Remainder);
        assert!(q.parts[0].rows.is_empty());
        assert_eq!(q.parts[0].params.len(), 1);
    }
    // The plate: no head, one remainder with one more parameter than rows,
    // holding P1's x (what `width` used to fix); P1's y and P3 stay stages.
    let (component, d, system) = all.iter().find(|(c, ..)| !c.sources.is_empty()).unwrap();
    assert!(d.parts.iter().all(|part| part.kind != Kind::Head));
    let last = d.parts.last().unwrap();
    assert_eq!(last.kind, Kind::Remainder);
    assert_eq!(last.params.len(), last.rows.len() + 1);
    assert!(last.params.contains(&p(1, Coordinate::X)));
    assert_eq!(
        d.parts.iter().filter(|x| x.kind == Kind::Remainder).count(),
        1
    );
    let stage_params: Vec<Param> = d
        .parts
        .iter()
        .filter(|x| x.kind == Kind::Stage)
        .flat_map(|x| x.params.clone())
        .collect();
    assert!(stage_params.contains(&p(1, Coordinate::Y)));
    assert!(stage_params.contains(&p(3, Coordinate::X)));
    assert!(stage_params.contains(&p(3, Coordinate::Y)));
    assert_eq!(component.params.len(), 22);
    assert_eq!(system.equations.len(), 21);
}

#[test]
fn a_constraints_equations_merge_their_parts() {
    // The midpoint's two equations would be two singleton stages (x and y
    // each read their own coordinate of B), but one constraint stays whole.
    let mut sk = Sketch::new();
    let a = sk.point_fixed(0.0, 0.0).unwrap();
    let b = sk.point(3.0, 0.5).unwrap();
    let m = sk.point(1.0, 1.0).unwrap();
    let ab = sk.line(a, b).unwrap();
    sk.horizontal(ab).unwrap();
    sk.distance(a, b, 4.0).unwrap();
    sk.midpoint(m, ab).unwrap();
    let all = decompositions(&sk);
    let (component, d, system) = &all[0];
    check_invariants(system, component, d);
    assert_eq!(
        shape(system, d),
        [
            (Kind::Stage, vec![c(0)], vec![p(1, Coordinate::Y)]),
            (Kind::Stage, vec![c(1)], vec![p(1, Coordinate::X)]),
            (
                Kind::Stage,
                vec![c(2)],
                vec![p(2, Coordinate::X), p(2, Coordinate::Y)]
            ),
        ]
    );
}

#[test]
fn degenerate_ends_still_have_a_structure() {
    // Coincident ends make a distance's gradient vanish where it stands;
    // its structure is still its two points.
    let mut sk = Sketch::new();
    let a = sk.point_fixed(1.0, 1.0).unwrap();
    let b = sk.point(1.0, 1.0).unwrap();
    sk.distance(a, b, 2.0).unwrap();
    let all = decompositions(&sk);
    let (component, d, system) = &all[0];
    check_invariants(system, component, d);
    assert_eq!(d.rank, 1);
    assert_eq!(d.parts.len(), 1);
    assert_eq!(d.parts[0].kind, Kind::Remainder);
    assert_eq!(d.parts[0].params.len(), 2);
}

/// One construction step: where the point starts, the earlier points it is
/// tied to by distance (0 to 3, so it may be free or over-determined), and
/// whether it is the midpoint of the line through two earlier points
/// instead.
#[derive(Debug, Clone)]
struct Step {
    at: (f64, f64),
    ties: Vec<usize>,
    mid: bool,
}

fn step() -> impl Strategy<Value = Step> {
    (
        (-50.0..50.0_f64, -50.0..50.0_f64),
        proptest::collection::vec(0..64_usize, 0..=3),
        any::<bool>(),
    )
        .prop_map(|(at, ties, mid)| Step { at, ties, mid })
}

fn build(steps: &[Step]) -> Option<Sketch> {
    let mut sk = Sketch::new();
    let mut points: Vec<Point> = vec![sk.point_fixed(0.0, 0.0).ok()?];
    for s in steps {
        let n = points.len();
        let q = sk.point(s.at.0, s.at.1).ok()?;
        let mut ties: Vec<usize> = s.ties.iter().map(|t| t % n).collect();
        ties.sort_unstable();
        ties.dedup();
        if s.mid && n >= 2 && ties.len() >= 2 {
            let line = sk.line(points[ties[0]], points[ties[1]]).ok()?;
            sk.midpoint(q, line).ok()?;
        } else {
            for t in ties {
                sk.distance(points[t], q, 1.0 + (s.at.0 - s.at.1).abs())
                    .ok()?;
            }
        }
        points.push(q);
    }
    Some(sk)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn constructed_sketches_decompose_consistently(
        steps in proptest::collection::vec(step(), 1..12)
    ) {
        let Some(sk) = build(&steps) else { return Ok(()); };
        for (component, d, system) in &decompositions(&sk) {
            check_invariants(system, component, d);
        }
    }
}

/// A solve cuts each component once and its diagnosis reads that cut: the
/// outcome's decompositions are those of a fresh cut of the same sketch, and
/// a solve counts one cut per component (plans/m8-setup step 2).
fn assert_one_cut(name: &str, sk: &Sketch) {
    use crate::solution::SolverOptions;
    use crate::solve::{Options, solve_toward};
    let system = System::new(sk);
    let components = system
        .components
        .iter()
        .filter(|c| !c.params.is_empty())
        .count();
    let fresh = Decompositions::of(&system);
    let before = CUTS.with(std::cell::Cell::get);
    let out = solve_toward(sk, &[], Options::default());
    let after = CUTS.with(std::cell::Cell::get);
    assert!(after - before <= components, "{name}: solve cut twice");
    if let Some(cut) = &out.cut {
        assert_eq!(*cut, fresh, "{name}");
    }
    let before = CUTS.with(std::cell::Cell::get);
    let solution = sk.solve(&SolverOptions::default());
    let after = CUTS.with(std::cell::Cell::get);
    // The diagnosis of a conflict re-solves subsets and that of a redundancy
    // compiles the sketch without a constraint, each its own system.
    if solution.diagnostics.redundant.is_empty()
        && solution.diagnostics.conflicts.is_empty()
        && solution.status != crate::solution::Status::NotConverged
    {
        assert!(after - before <= components, "{name}: an edit cut twice");
    }
    let again = sk.solve(&SolverOptions::default());
    assert_eq!(solution, again, "{name}");
}

#[test]
fn an_edit_cuts_once_and_the_diagnosis_reads_the_solves_cut() {
    use laman_debug::corpus::{rect_chain, slot_grid, triangle_strip};
    for slug in [
        "belt",
        "belt-conflicting",
        "belt-redundant",
        "slotted-plate",
        "slotted-plate-conflicting",
        "slotted-plate-redundant",
        "sss-triangle",
        "sss-triangle-conflicting",
        "sss-triangle-redundant",
    ] {
        assert_one_cut(slug, &load(slug));
    }
    for (name, case) in [
        ("rect-chain-3", rect_chain(3)),
        ("rect-chain-19", rect_chain(19)),
        ("rect-chain-99", rect_chain(99)),
        ("slot-grid-2x2", slot_grid(2, 2)),
        ("slot-grid-4x6", slot_grid(4, 6)),
        ("triangle-strip-6", triangle_strip(6)),
        ("triangle-strip-82", triangle_strip(82)),
    ] {
        let case = case.unwrap();
        let sk: Sketch =
            serde_json::from_str(&serde_json::to_string(&case.sketch).unwrap()).unwrap();
        assert_one_cut(name, &sk);
    }
}
