//! Profiles of random planar subdivisions built by construction
//! (plans/m13-branched-profiles step 7, ADR-0017). A rectangle is split
//! into columns by walls, each a line or, when no wall runs across, an arc
//! bulging into either neighbour; a horizontal wall may cross every column,
//! making joins of degree 4. A cell may hold a quarter-arc fillet in its
//! bottom-right corner, tangent to both walls it meets (two tangent joins),
//! and an island rectangle tied by a line to the wall above it (a bridge).
//! A block outside the rectangle may hang from its right wall by another
//! bridge. Lines are drawn in random directions and the edges in a random
//! order, and the whole sketch is rotated, scaled by 1e-6 to 1e6 and
//! translated up to a thousand times its scale.
//!
//! The construction says which area lies on each side of every edge (a
//! probe a thousandth of a unit off its middle, classified by the closed
//! form), so it says every profile's loop, every hole, every bridge and
//! every area. Asserted: the profile count; each profile's loops as sets of
//! directed edges; its areas; the bounded areas summing to the outline's;
//! each shared edge in two loops, once in each direction; the bridges; and
//! `laman_check::check_profiles`. The same subdivision with undeclared
//! crossing diagonals in one cell is held to what ADR-0017 D5 still
//! promises there. Each margin is at least a fiftieth of a cell, far above
//! the length tolerance (ADR-0003). A failure is reduced by proptest and
//! committed as a fixture under `tests/fixtures/regression/` with the
//! desired assertion and an `#[ignore]` (`.agents/rules/solver.md`), never
//! a wider tolerance here.
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a failing unwrap is the test failing (clippy.toml)

use laman::model::profile::Profiles;
use laman::model::{EntityId, Point, Sketch};
use laman::profile::profiles;
use proptest::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::PI;

/// How close an area must be, relative to the rectangle's. Coordinates are
/// rounded once by the transform, and translations up to a thousand times
/// the extent keep the cross products to about 1e-13 of it.
const AREA_RELATIVE: f64 = 1e-9;

/// How far off an edge's middle the construction probes the area on each
/// side, in design units: a thousandth of a unit, against margins of half a
/// unit or more.
const PROBE: f64 = 1e-3;

/// The block hanging outside the rectangle: its gap and its lower edge,
/// as fractions of the rectangle's height, and the bridge's height.
const BLOCK_GAP: f64 = 5.0;
const BLOCK_LOW: f64 = 0.8;
const BRIDGE_AT: f64 = 0.9;

/// The island inside a cell, as fractions of the cell: left, right, bottom
/// and top. Its bridge leaves the middle of its top edge.
const ISLAND: [f64; 4] = [0.4, 0.65, 0.6, 0.85];

/// What a cell holds.
#[derive(Debug, Clone, Copy)]
struct CellSpec {
    /// A fillet's radius as a fraction of the cell's smaller side.
    fillet: Option<f64>,
    island: bool,
}

/// One subdivision, in design units.
#[derive(Debug, Clone)]
struct Spec {
    widths: Vec<f64>,
    height: f64,
    /// The horizontal wall's height as a fraction of the rectangle's.
    split: Option<f64>,
    /// Per interior wall, in -1..1: below 0.3 in size a line, else an arc
    /// bulging toward +x (positive) or -x. Read only without a split.
    bulges: Vec<f64>,
    cells: Vec<CellSpec>,
    /// The hanging block's width; it spans `BLOCK_LOW` to the top.
    block: Option<f64>,
    flips: Vec<bool>,
    keys: Vec<u32>,
    /// A cell crossed by both its diagonals, undeclared (the crossing test).
    crossed: Option<usize>,
}

/// The similarity every coordinate goes through.
#[derive(Debug, Clone, Copy)]
struct Transform {
    angle: f64,
    scale: f64,
    shift: [f64; 2],
}

impl Transform {
    fn at(&self, [x, y]: [f64; 2]) -> [f64; 2] {
        let (s, c) = libm::sincos(self.angle);
        [
            self.scale * (c * x - s * y) + self.shift[0],
            self.scale * (s * x + c * y) + self.shift[1],
        ]
    }
}

/// The area a probe lands in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Region {
    Out,
    Cell(usize),
    Corner(usize),
    Island(usize),
    Block,
}

/// What an edge is to the construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    /// A wall, a fillet, a bridge or the block.
    Wall,
    /// A side of an island: in its cell's hole.
    Island,
    /// A crossing diagonal: its probes meet at the crossing, so the
    /// construction says only that it is no bridge.
    Diagonal,
}

/// An edge in design units.
#[derive(Debug, Clone, Copy)]
enum Edge {
    Line([f64; 2], [f64; 2]),
    /// Counter-clockwise from start to end around centre.
    Arc {
        centre: [f64; 2],
        start: [f64; 2],
        end: [f64; 2],
    },
}

/// An arc wall: its sagitta, signed toward +x, its radius and centre.
#[derive(Debug, Clone, Copy)]
struct ArcWall {
    sagitta: f64,
    radius: f64,
    centre: [f64; 2],
}

/// The subdivision's geometry in design units.
struct Layout {
    xs: Vec<f64>,
    height: f64,
    rows: Vec<(f64, f64)>,
    walls: Vec<Option<ArcWall>>,
    spec: Spec,
}

impl Layout {
    fn new(spec: &Spec) -> Self {
        let mut xs = vec![0.0];
        for w in &spec.widths {
            xs.push(xs.last().unwrap() + w);
        }
        let height = spec.height;
        let rows = match spec.split {
            Some(f) => vec![(0.0, f * height), (f * height, height)],
            None => vec![(0.0, height)],
        };
        let n = spec.widths.len();
        let walls = (0..=n)
            .map(|i| {
                if i == 0 || i == n || spec.split.is_some() {
                    return None;
                }
                let v = spec.bulges[i - 1];
                if v.abs() < 0.3 {
                    return None;
                }
                let room = spec.widths[i - 1].min(spec.widths[i]);
                let size = (0.08 + (v.abs() - 0.3) / 0.7 * 0.17) * room;
                let sagitta = size.copysign(v);
                let radius = (height * height / 4.0 + size * size) / (2.0 * size);
                let centre = [xs[i] - (radius - size).copysign(v), height / 2.0];
                Some(ArcWall {
                    sagitta,
                    radius,
                    centre,
                })
            })
            .collect();
        Layout {
            xs,
            height,
            rows,
            walls,
            spec: spec.clone(),
        }
    }

    fn columns(&self) -> usize {
        self.spec.widths.len()
    }

    fn cell(&self, row: usize, col: usize) -> usize {
        row * self.columns() + col
    }

    /// A cell's spec, with a fillet dropped where its right wall is an arc.
    fn holds(&self, row: usize, col: usize) -> CellSpec {
        let mut c = self.spec.cells[self.cell(row, col)];
        if self.walls[col + 1].is_some() {
            c.fillet = None;
        }
        c
    }

    /// Left, right, bottom and top of a cell, and its fillet radius.
    fn bounds(&self, row: usize, col: usize) -> ([f64; 4], Option<f64>) {
        let (y0, y1) = self.rows[row];
        let (x0, x1) = (self.xs[col], self.xs[col + 1]);
        let side = (x1 - x0).min(y1 - y0);
        let r = self.holds(row, col).fillet.map(|f| f * side);
        ([x0, x1, y0, y1], r)
    }

    fn island(&self, row: usize, col: usize) -> [f64; 4] {
        let ([x0, x1, y0, y1], _) = self.bounds(row, col);
        let (w, h) = (x1 - x0, y1 - y0);
        [
            x0 + ISLAND[0] * w,
            x0 + ISLAND[1] * w,
            y0 + ISLAND[2] * h,
            y0 + ISLAND[3] * h,
        ]
    }

    fn width(&self) -> f64 {
        *self.xs.last().unwrap()
    }

    fn block(&self) -> Option<[f64; 4]> {
        self.spec.block.map(|w| {
            let x0 = self.width() + BLOCK_GAP;
            [x0, x0 + w, BLOCK_LOW * self.height, self.height]
        })
    }

    /// Where wall `i` is at height `y`.
    fn wall_x(&self, i: usize, y: f64) -> f64 {
        match self.walls[i] {
            None => self.xs[i],
            Some(a) => {
                let dy = y - a.centre[1];
                a.centre[0] + (a.radius * a.radius - dy * dy).sqrt().copysign(a.sagitta)
            }
        }
    }

    /// The region a point lies in, by the closed form.
    fn classify(&self, [x, y]: [f64; 2]) -> Region {
        if let Some([x0, x1, y0, y1]) = self.block() {
            if x > x0 && x < x1 && y > y0 && y < y1 {
                return Region::Block;
            }
        }
        if x <= 0.0 || x >= self.width() || y <= 0.0 || y >= self.height {
            return Region::Out;
        }
        let n = self.columns();
        let col = (1..n)
            .find(|&i| x < self.wall_x(i, y))
            .map_or(n - 1, |i| i - 1);
        let row = usize::from(self.rows.len() == 2 && y > self.rows[0].1);
        let c = self.cell(row, col);
        if self.holds(row, col).island {
            let [ix0, ix1, iy0, iy1] = self.island(row, col);
            if x > ix0 && x < ix1 && y > iy0 && y < iy1 {
                return Region::Island(c);
            }
        }
        let ([_, x1, y0, _], r) = self.bounds(row, col);
        if let Some(r) = r {
            let (cx, cy) = (x1 - r, y0 + r);
            if x > cx && y < cy && libm::hypot(x - cx, y - cy) > r {
                return Region::Corner(c);
            }
        }
        Region::Cell(c)
    }

    /// The area of a circular segment cut by wall `i`, and whether it bulges
    /// toward +x.
    fn segment(&self, i: usize) -> Option<(f64, bool)> {
        self.walls[i].map(|a| {
            let r = a.radius;
            let s = self.height / (2.0 * r);
            let c = (r - a.sagitta.abs()) / r;
            (r * r * (libm::asin(s) - s * c), a.sagitta > 0.0)
        })
    }

    /// Every bounded area that is a profile, with its outer area and its
    /// hole's area, by the closed form.
    fn areas(&self) -> BTreeMap<Region, (f64, f64)> {
        let mut out = BTreeMap::new();
        for row in 0..self.rows.len() {
            for col in 0..self.columns() {
                let c = self.cell(row, col);
                let ([x0, x1, y0, y1], r) = self.bounds(row, col);
                let mut area = (x1 - x0) * (y1 - y0);
                if let Some((seg, right)) = self.segment(col) {
                    area += if right { -seg } else { seg };
                }
                if let Some((seg, right)) = self.segment(col + 1) {
                    area += if right { seg } else { -seg };
                }
                if let Some(r) = r {
                    let corner = r * r * (1.0 - PI / 4.0);
                    area -= corner;
                    out.insert(Region::Corner(c), (corner, 0.0));
                }
                let hole = if self.holds(row, col).island {
                    let [ix0, ix1, iy0, iy1] = self.island(row, col);
                    (ix1 - ix0) * (iy1 - iy0)
                } else {
                    0.0
                };
                out.insert(Region::Cell(c), (area, hole));
            }
        }
        if let Some([x0, x1, y0, y1]) = self.block() {
            out.insert(Region::Block, ((x1 - x0) * (y1 - y0), 0.0));
        }
        out
    }

    /// Every edge, and its role.
    fn edges(&self) -> Vec<(Edge, Role)> {
        let (w, h) = (self.width(), self.height);
        let n = self.columns();
        let mut edges = Vec::new();
        // Breakpoints along each straight run, keyed by its fixed coordinate.
        let mut across: BTreeMap<u64, (f64, Vec<f64>)> = BTreeMap::new(); // y = const
        let mut up: BTreeMap<u64, (f64, Vec<f64>)> = BTreeMap::new(); // x = const
        let at = |map: &mut BTreeMap<u64, (f64, Vec<f64>)>, fixed: f64, t: f64| {
            map.entry(fixed.to_bits())
                .or_insert_with(|| (fixed, Vec::new()))
                .1
                .push(t);
        };
        for &(y0, y1) in &self.rows {
            for (i, &x) in self.xs.iter().enumerate() {
                at(&mut across, y0, x);
                at(&mut across, y1, x);
                if self.walls[i].is_none() {
                    at(&mut up, x, y0);
                    at(&mut up, x, y1);
                }
            }
        }
        for row in 0..self.rows.len() {
            for col in 0..n {
                let ([x0, x1, y0, y1], r) = self.bounds(row, col);
                if let Some(r) = r {
                    at(&mut across, y0, x1 - r);
                    at(&mut up, x1, y0 + r);
                    edges.push((
                        Edge::Arc {
                            centre: [x1 - r, y0 + r],
                            start: [x1 - r, y0],
                            end: [x1, y0 + r],
                        },
                        Role::Wall,
                    ));
                }
                if self.holds(row, col).island {
                    let [ix0, ix1, iy0, iy1] = self.island(row, col);
                    let mid = x0 + 0.5 * (ISLAND[0] + ISLAND[1]) * (x1 - x0);
                    at(&mut across, y1, mid);
                    let ring = [[ix0, iy0], [ix1, iy0], [ix1, iy1], [mid, iy1], [ix0, iy1]];
                    for k in 0..5 {
                        edges.push((Edge::Line(ring[k], ring[(k + 1) % 5]), Role::Island));
                    }
                    edges.push((Edge::Line([mid, iy1], [mid, y1]), Role::Wall));
                }
            }
        }
        for (i, wall) in self.walls.iter().enumerate() {
            if let Some(a) = wall {
                let (bottom, top) = ([self.xs[i], 0.0], [self.xs[i], h]);
                let (start, end) = if a.sagitta > 0.0 {
                    (bottom, top)
                } else {
                    (top, bottom)
                };
                edges.push((
                    Edge::Arc {
                        centre: a.centre,
                        start,
                        end,
                    },
                    Role::Wall,
                ));
            }
        }
        if let Some([x0, x1, y0, y1]) = self.block() {
            let yb = BRIDGE_AT * h;
            at(&mut up, w, yb);
            let ring = [[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, yb]];
            for k in 0..5 {
                edges.push((Edge::Line(ring[k], ring[(k + 1) % 5]), Role::Wall));
            }
            edges.push((Edge::Line([w, yb], [x0, yb]), Role::Wall));
        }
        for (y, mut xs) in across.into_values() {
            xs.sort_by(f64::total_cmp);
            xs.dedup();
            for pair in xs.windows(2) {
                edges.push((Edge::Line([pair[0], y], [pair[1], y]), Role::Wall));
            }
        }
        for (x, mut ys) in up.into_values() {
            ys.sort_by(f64::total_cmp);
            ys.dedup();
            for pair in ys.windows(2) {
                edges.push((Edge::Line([x, pair[0]], [x, pair[1]]), Role::Wall));
            }
        }
        if let Some(c) = self.spec.crossed {
            let (row, col) = (c / n, c % n);
            let ([x0, x1, y0, y1], _) = self.bounds(row, col);
            edges.push((Edge::Line([x0, y0], [x1, y1]), Role::Diagonal));
            edges.push((Edge::Line([x1, y0], [x0, y1]), Role::Diagonal));
        }
        edges
    }

    /// The regions left of an edge as drawn and as reversed.
    fn sides(&self, edge: &Edge) -> (Region, Region) {
        let (mid, toward_left) = match *edge {
            Edge::Line(a, b) => {
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let len = libm::hypot(dx, dy);
                (
                    [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0],
                    [-dy / len, dx / len],
                )
            }
            Edge::Arc { centre, start, end } => {
                let r = libm::hypot(start[0] - centre[0], start[1] - centre[1]);
                let t0 = libm::atan2(start[1] - centre[1], start[0] - centre[0]);
                let mut t1 = libm::atan2(end[1] - centre[1], end[0] - centre[0]);
                if t1 <= t0 {
                    t1 += 2.0 * PI;
                }
                let (s, c) = libm::sincos(0.5 * (t0 + t1));
                ([centre[0] + r * c, centre[1] + r * s], [-c, -s])
            }
        };
        let probe = |k: f64| {
            self.classify([
                mid[0] + k * PROBE * toward_left[0],
                mid[1] + k * PROBE * toward_left[1],
            ])
        };
        (probe(1.0), probe(-1.0))
    }
}

/// A directed edge as a sortable key: its id and whether it is reversed.
type Directed = (u32, bool);

/// The sketch built from a layout, and the construction's facts about it.
struct Built {
    sketch: Sketch,
    /// Per edge id: the region left of it as drawn and as reversed, and
    /// its role.
    sides: BTreeMap<u32, (Region, Region, Role)>,
}

fn build(layout: &Layout, t: &Transform) -> Built {
    let spec = &layout.spec;
    let mut sketch = Sketch::new();
    let mut points: BTreeMap<(u64, u64), Point> = BTreeMap::new();
    let mut point = |sk: &mut Sketch, p: [f64; 2]| -> Point {
        *points
            .entry((p[0].to_bits(), p[1].to_bits()))
            .or_insert_with(|| {
                let [x, y] = t.at(p);
                sk.point(x, y).unwrap()
            })
    };
    let edges = layout.edges();
    let mut order: Vec<usize> = (0..edges.len()).collect();
    order.sort_by_key(|&i| (spec.keys[i % spec.keys.len()], i));
    let mut sides = BTreeMap::new();
    for i in order {
        let (edge, role) = edges[i];
        let (id, edge) = match edge {
            Edge::Line(a, b) => {
                let (a, b) = if spec.flips[i % spec.flips.len()] {
                    (b, a)
                } else {
                    (a, b)
                };
                let (pa, pb) = (point(&mut sketch, a), point(&mut sketch, b));
                (sketch.line(pa, pb).unwrap().id(), Edge::Line(a, b))
            }
            Edge::Arc { centre, start, end } => {
                let c = point(&mut sketch, centre);
                let (s, e) = (point(&mut sketch, start), point(&mut sketch, end));
                (sketch.arc(c, s, e).unwrap().id(), edge)
            }
        };
        let (left, right) = layout.sides(&edge);
        sides.insert(id.0, (left, right, role));
    }
    Built { sketch, sides }
}

/// What the construction says the profiles are: per profile region, its
/// outer loop and its holes as sets of directed edges; and the bridges.
struct Expected {
    outers: BTreeMap<BTreeSet<Directed>, Region>,
    holes: BTreeMap<Region, Vec<BTreeSet<Directed>>>,
    bridges: Vec<EntityId>,
}

fn expected(built: &Built) -> Expected {
    let mut outers: BTreeMap<Region, BTreeSet<Directed>> = BTreeMap::new();
    let mut holes: BTreeMap<Region, BTreeSet<Directed>> = BTreeMap::new();
    let mut bridges = Vec::new();
    for (&id, &(left, right, role)) in &built.sides {
        if role == Role::Diagonal {
            continue;
        }
        if left == right {
            bridges.push(EntityId(id));
            continue;
        }
        for (region, reversed) in [(left, false), (right, true)] {
            if matches!(region, Region::Out | Region::Island(_)) {
                continue;
            }
            let into = if role == Role::Island {
                &mut holes
            } else {
                &mut outers
            };
            into.entry(region).or_default().insert((id, reversed));
        }
    }
    let holes = outers
        .keys()
        .map(|r| (*r, holes.get(r).into_iter().cloned().collect()))
        .collect();
    Expected {
        outers: outers.into_iter().map(|(r, s)| (s, r)).collect(),
        holes,
        bridges,
    }
}

fn directed(l: &laman::model::profile::Loop) -> BTreeSet<Directed> {
    l.edges.iter().map(|t| (t.entity.0, t.reversed)).collect()
}

/// What ADR-0017 promises for every finite input, crossing or not: nothing
/// is left unjoined, the bridges are the edges with one area on both sides,
/// every directed edge is in at most one loop, every other edge in some
/// loop, every area is finite, and a second call gives the same.
fn invariants(built: &Built, want_bridges: &[EntityId]) -> Result<Profiles, TestCaseError> {
    let sk = &built.sketch;
    let found = profiles(sk, &sk.coordinates()).unwrap();
    prop_assert_eq!(&found, &profiles(sk, &sk.coordinates()).unwrap());
    prop_assert!(found.open_chains.is_empty());
    prop_assert!(found.dangling.is_empty());
    prop_assert!(found.degenerate.is_empty());
    prop_assert!(found.unjoined.is_empty());
    prop_assert_eq!(&found.bridges, want_bridges);
    let mut seen: BTreeMap<Directed, usize> = BTreeMap::new();
    for p in &found.profiles {
        prop_assert!(p.net_area.is_finite());
        for l in std::iter::once(&p.outer).chain(&p.holes) {
            prop_assert!(l.signed_area.is_finite());
            for t in &l.edges {
                *seen.entry((t.entity.0, t.reversed)).or_default() += 1;
            }
        }
    }
    prop_assert!(seen.values().all(|&k| k == 1), "{seen:?}");
    for id in built.sides.keys() {
        let listed = seen.contains_key(&(*id, false)) || seen.contains_key(&(*id, true));
        prop_assert_eq!(
            listed,
            !want_bridges.contains(&EntityId(*id)),
            "edge {}",
            id
        );
    }
    Ok(found)
}

/// Builds the subdivision and holds `profiles` to the construction: the
/// loops and bridges always, the areas when `areas` (they saturate or
/// underflow at f64's extremes).
fn check(spec: &Spec, t: &Transform, areas: bool) -> Result<(), TestCaseError> {
    let layout = Layout::new(spec);
    let built = build(&layout, t);
    let want = expected(&built);
    let found = invariants(&built, &want.bridges)?;
    let sk = &built.sketch;
    let report = laman_check::check_profiles(sk, &sk.coordinates(), &found);
    prop_assert!(report.is_clean(), "{report:#?}");

    let closed = layout.areas();
    prop_assert_eq!(found.profiles.len(), want.outers.len());
    prop_assert_eq!(found.profiles.len(), closed.len());
    let s2 = t.scale * t.scale;
    let whole = layout.width() * layout.height * s2;
    let near = |found: f64, want: f64| (found - want).abs() <= AREA_RELATIVE * whole;
    let mut met = BTreeSet::new();
    let mut inside = 0.0;
    for p in &found.profiles {
        let region = want.outers.get(&directed(&p.outer));
        prop_assert!(region.is_some(), "an outer loop not built: {:?}", p.outer);
        let region = *region.unwrap();
        prop_assert!(met.insert(region), "{region:?} twice");
        let holes: Vec<_> = p.holes.iter().map(directed).collect();
        prop_assert_eq!(&holes, &want.holes[&region], "{:?}", region);
        prop_assert!(p.outer.signed_area >= 0.0);
        prop_assert!(p.holes.iter().all(|h| h.signed_area <= 0.0));
        if areas {
            let (outer, hole) = closed[&region];
            prop_assert!(near(p.outer.signed_area, outer * s2), "{region:?}");
            prop_assert!(near(p.net_area, (outer - hole) * s2), "{region:?}");
            for h in &p.holes {
                prop_assert!(near(h.signed_area, -hole * s2), "{region:?}");
            }
            if region != Region::Block {
                inside += p.outer.signed_area;
            }
        }
    }
    // The bounded areas of the rectangle's piece fill its outline.
    if areas {
        prop_assert!(near(inside, whole), "{inside} against {whole}");
    }
    // Each wall between two profiles is in both, once in each direction.
    let mut uses: BTreeMap<u32, Vec<bool>> = BTreeMap::new();
    for p in &found.profiles {
        for l in std::iter::once(&p.outer).chain(&p.holes) {
            for t in &l.edges {
                uses.entry(t.entity.0).or_default().push(t.reversed);
            }
        }
    }
    for (id, &(left, right, _)) in &built.sides {
        let bounded = |r: Region| !matches!(r, Region::Out | Region::Island(_));
        if left != right && bounded(left) && bounded(right) {
            let mut u = uses[id].clone();
            u.sort_unstable();
            prop_assert_eq!(u, vec![false, true], "edge {}", id);
        }
    }
    Ok(())
}

fn cell() -> impl Strategy<Value = CellSpec> {
    (
        prop_oneof![Just(None), (0.2..0.45_f64).prop_map(Some)],
        any::<bool>(),
    )
        .prop_map(|(fillet, island)| CellSpec { fillet, island })
}

fn spec() -> impl Strategy<Value = Spec> {
    (1..=4_usize)
        .prop_flat_map(|n| {
            (
                proptest::collection::vec(20.0..40.0_f64, n),
                20.0..40.0_f64,
                prop_oneof![Just(None), (0.4..0.7_f64).prop_map(Some)],
                proptest::collection::vec(-1.0..1.0_f64, n - 1),
                proptest::collection::vec(cell(), 2 * n),
                prop_oneof![Just(None), (10.0..30.0_f64).prop_map(Some)],
                proptest::collection::vec(any::<bool>(), 64),
                proptest::collection::vec(any::<u32>(), 64),
            )
        })
        .prop_map(
            |(widths, height, split, bulges, mut cells, block, flips, keys)| {
                cells.truncate(if split.is_some() { 2 } else { 1 } * widths.len());
                Spec {
                    widths,
                    height,
                    split,
                    bulges,
                    cells,
                    block,
                    flips,
                    keys,
                    crossed: None,
                }
            },
        )
}

fn transform() -> impl Strategy<Value = Transform> {
    (
        0.0..(2.0 * PI),
        -6.0..6.0_f64,
        -1000.0..1000.0_f64,
        -1000.0..1000.0_f64,
    )
        .prop_map(|(angle, exponent, sx, sy)| {
            let scale = libm::pow(10.0, exponent);
            Transform {
                angle,
                scale,
                shift: [sx * scale, sy * scale],
            }
        })
}

proptest! {
    // No explicit case count: proptest's default and PROPTEST_CASES both apply.
    #[test]
    fn subdivisions_come_out_as_built(spec in spec(), t in transform()) {
        check(&spec, &t, true)?;
    }

    /// Two diagonals crossing undeclared in one cell (ADR-0017 D5): the
    /// result is deterministic, every loop keeps a role and no edge is
    /// dropped, though the nesting is not specified.
    #[test]
    fn crossing_diagonals_drop_nothing(
        spec in spec(),
        pick in any::<usize>(),
        t in transform(),
    ) {
        let mut spec = spec;
        spec.crossed = Some(pick % spec.cells.len());
        let built = build(&Layout::new(&spec), &t);
        let want = expected(&built);
        invariants(&built, &want.bridges)?;
    }
}

/// Three columns, the second wall an arc bulging into the middle one;
/// fillets in the outer columns, islands in the first two, a block hanging
/// off the right.
fn fixed() -> Spec {
    Spec {
        widths: vec![30.0, 25.0, 35.0],
        height: 30.0,
        split: None,
        bulges: vec![0.1, -0.6],
        cells: vec![
            CellSpec {
                fillet: Some(0.4),
                island: true,
            },
            CellSpec {
                fillet: None,
                island: true,
            },
            CellSpec {
                fillet: Some(0.3),
                island: false,
            },
        ],
        block: Some(20.0),
        flips: vec![true, false, false, true, true],
        keys: vec![7, 3, 9, 1, 4, 4, 8, 2, 6, 5, 0],
        crossed: None,
    }
}

/// The same subdivision split across, so its joins are of degree 4.
fn fixed_split() -> Spec {
    let mut spec = fixed();
    spec.split = Some(0.5);
    spec.cells.extend(spec.cells.clone());
    spec
}

/// At 1e300, with a tolerance relative to the sketch, the loops, holes and
/// bridges are the ones built and every area is finite with its sign.
#[test]
fn the_largest_scale_keeps_the_structure() {
    for spec in [fixed(), fixed_split()] {
        for angle in [0.0, 0.7] {
            let t = Transform {
                angle,
                scale: 1e300,
                shift: [1e300, -2e300],
            };
            check(&spec, &t, false).unwrap();
        }
    }
}

/// At 1e-300 the length tolerance is `LENGTH_FLOOR` (ADR-0003), larger than
/// the sketch, so ends at a join tie and go by id (ADR-0017's addendum):
/// the loops are not the ones built, but what holds for every finite input
/// still must, every edge but a bridge in some loop among them.
#[test]
#[ignore = "regression/below-floor-uncovered-edges: below the length floor the outline can walk an edge both ways, so it is in no profile"]
fn the_smallest_scale_keeps_every_edge() {
    for spec in [fixed(), fixed_split()] {
        for angle in [0.0, 0.7] {
            let t = Transform {
                angle,
                scale: 1e-300,
                shift: [1e-300, -2e-300],
            };
            let built = build(&Layout::new(&spec), &t);
            let want = expected(&built);
            invariants(&built, &want.bridges).unwrap();
            let sk = &built.sketch;
            let found = profiles(sk, &sk.coordinates()).unwrap();
            let report = laman_check::check_profiles(sk, &sk.coordinates(), &found);
            assert!(report.is_clean(), "{report:#?}");
        }
    }
}

/// The shrunk case: a square and one diagonal at 1e-299, lines drawn in
/// order. Desired: every edge is in a reported loop and the checker is
/// clean (ADR-0017's consequences); which loops is not specified.
#[test]
#[ignore = "regression/below-floor-uncovered-edges: no profile, all five edges uncovered"]
fn below_floor_uncovered_edges() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/regression/below-floor-uncovered-edges");
    let text = std::fs::read_to_string(dir.join("sketch.json")).unwrap();
    let sk = laman_debug::json::sketch_from_str(&text).unwrap();
    let found = profiles(&sk, &sk.coordinates()).unwrap();
    let report = laman_check::check_profiles(&sk, &sk.coordinates(), &found);
    assert!(report.is_clean(), "{found:#?} {report:#?}");
    let listed: BTreeSet<u32> = found
        .profiles
        .iter()
        .flat_map(|p| std::iter::once(&p.outer).chain(&p.holes))
        .flat_map(|l| l.edges.iter().map(|t| t.entity.0))
        .collect();
    assert_eq!(listed, (4..9).collect());
}

/// The fixed subdivisions at sketch scale, as a readable instance of the
/// property: six profiles and three bridges; split across, eleven and five.
#[test]
fn fixed_subdivisions_come_out_as_built() {
    for (spec, count, bridges) in [(fixed(), 6, 3), (fixed_split(), 11, 5)] {
        let t = Transform {
            angle: 0.3,
            scale: 1.0,
            shift: [5.0, 7.0],
        };
        check(&spec, &t, true).unwrap();
        let built = build(&Layout::new(&spec), &t);
        let found = profiles(&built.sketch, &built.sketch.coordinates()).unwrap();
        assert_eq!(found.profiles.len(), count);
        assert_eq!(found.bridges.len(), bridges);
    }
}
