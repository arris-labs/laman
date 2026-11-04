//! Dumps: a sketch at some coordinates written out so it can be read
//! (`text`, which diffs in a test) or looked at (`svg`). Neither is an
//! oracle; a picture is never a test (`docs/README.md`).
//!
//! Both take the coordinates to show and the checker's [`Report`] there, so
//! what is off is marked. A parameter the coordinates lack is shown as `?`
//! in the text and left out of the picture.

use laman::model::{
    Constraint, Contact, Coordinate, Coordinates, Entity, EntityId, Sense, Sketch, Span,
};
use laman::solution::{ConstraintRef, EntityRef, FlipSubject, Origin, Solution};
use laman_check::{Report, Source, check};
use std::collections::BTreeSet;
use std::fmt::Write as _;

/// Fraction of the drawing's larger side added as margin on every side.
const MARGIN: f64 = 0.1;
/// Smallest side of the view box, so a lone point still has a frame.
const MIN_SIDE: f64 = 1.0;
/// Stroke width as a fraction of the view box's larger side.
const STROKE: f64 = 0.004;
/// Label height as a fraction of the view box's larger side.
const LABEL: f64 = 0.03;
/// Dash length as a fraction of the view box's larger side.
const DASH: f64 = 0.012;
/// Marker radius of a point, as a fraction of the view box's larger side.
const DOT: f64 = 0.006;

fn value(values: &Coordinates, id: EntityId, c: Coordinate) -> Option<f64> {
    values.get(&(id, c)).copied()
}

fn point_at(values: &Coordinates, id: EntityId) -> Option<(f64, f64)> {
    Some((
        value(values, id, Coordinate::X)?,
        value(values, id, Coordinate::Y)?,
    ))
}

fn num(v: Option<f64>) -> String {
    v.map_or_else(|| "?".to_string(), |v| format!("{v}"))
}

fn sign(s: Sense) -> &'static str {
    match s {
        Sense::Positive => "+",
        Sense::Negative => "-",
    }
}

/// The entity ids a constraint names, with its values and senses, as the
/// text dump prints them after the kind.
fn describe(c: &Constraint) -> String {
    let id = |e: EntityId| format!("#{}", e.0);
    match c {
        Constraint::Coincident { p, q } => format!("{} {}", id(p.id()), id(q.id())),
        Constraint::PointOnLine { p, line } => format!("{} {}", id(p.id()), id(line.id())),
        Constraint::PointOnCircle { p, on } => format!("{} {}", id(p.id()), id(on.id())),
        Constraint::Horizontal(span) | Constraint::Vertical(span) => match span {
            Span::Line(l) => id(l.id()),
            Span::Points(p, q) => format!("{} {}", id(p.id()), id(q.id())),
        },
        Constraint::Parallel { a, b }
        | Constraint::Perpendicular { a, b }
        | Constraint::EqualLength { a, b } => format!("{} {}", id(a.id()), id(b.id())),
        Constraint::EqualRadius { a, b } => format!("{} {}", id(a.id()), id(b.id())),
        Constraint::TangentLineCircle { line, round, sense } => {
            format!(
                "{} {} sense={}",
                id(line.id()),
                id(round.id()),
                sign(*sense)
            )
        }
        Constraint::TangentCircles { a, b, contact } => {
            let how = match contact {
                Contact::External => "external".to_string(),
                Contact::Internal(s) => format!("internal({})", sign(*s)),
            };
            format!("{} {} {how}", id(a.id()), id(b.id()))
        }
        Constraint::Midpoint { p, line } => format!("{} {}", id(p.id()), id(line.id())),
        Constraint::Symmetric { p, q, axis } => {
            format!("{} {} axis={}", id(p.id()), id(q.id()), id(axis.id()))
        }
        Constraint::Fix { p, x, y } => format!("{} at ({x}, {y})", id(p.id())),
        Constraint::Distance { p, q, value } => {
            format!("{} {} value={value}", id(p.id()), id(q.id()))
        }
        Constraint::PointLineDistance {
            p,
            line,
            value,
            sense,
        } => format!(
            "{} {} value={value} sense={}",
            id(p.id()),
            id(line.id()),
            sign(*sense)
        ),
        Constraint::Radius { round, value } => format!("{} value={value}", id(round.id())),
        Constraint::Angle { a, b, value } => format!("{} {} value={value}", id(a.id()), id(b.id())),
    }
}

/// The sketch at `values` as text: every entity and constraint with its id,
/// tag and flags, then the checker's findings. One line each, ids in order,
/// floats in Rust's shortest round-trip form, so two runs that agree to the
/// bit print the same text.
///
/// ```
/// use laman::model::Sketch;
/// use laman_check::check;
/// use laman_debug::dump::text;
///
/// let mut sk = Sketch::new();
/// let p = sk.point_fixed(0.0, 0.0)?;
/// let q = sk.point(3.0, 4.0)?;
/// sk.distance(p, q, 5.0)?;
/// let values = sk.coordinates();
/// let out = text(&sk, &values, &check(&sk, &values));
/// assert!(out.contains("#1 point (3, 4)"));
/// assert!(out.ends_with("report: clean\n"));
/// # Ok::<(), laman::model::ModelError>(())
/// ```
pub fn text(sketch: &Sketch, values: &Coordinates, report: &Report) -> String {
    let mut out = String::new();
    let entities: Vec<_> = sketch.entities().collect();
    let constraints: Vec<_> = sketch.constraints().collect();
    let _ = writeln!(out, "entities: {}", entities.len());
    for (id, entity) in &entities {
        let id = *id;
        let _ = write!(out, "  #{} ", id.0);
        match entity {
            Entity::Point { .. } => {
                let _ = write!(
                    out,
                    "point ({}, {})",
                    num(value(values, id, Coordinate::X)),
                    num(value(values, id, Coordinate::Y))
                );
            }
            Entity::Line { a, b } => {
                let _ = write!(out, "line #{} -> #{}", a.id().0, b.id().0);
            }
            Entity::Circle { c, .. } => {
                let _ = write!(
                    out,
                    "circle c=#{} r={}",
                    c.id().0,
                    num(value(values, id, Coordinate::R))
                );
            }
            Entity::Arc { c, s, e } => {
                let _ = write!(out, "arc c=#{} s=#{} e=#{}", c.id().0, s.id().0, e.id().0);
            }
        }
        let fixed: Vec<&str> = [
            (Coordinate::X, "x"),
            (Coordinate::Y, "y"),
            (Coordinate::R, "r"),
        ]
        .iter()
        .filter(|(c, _)| sketch.is_fixed(id, *c))
        .map(|(_, n)| *n)
        .collect();
        if !fixed.is_empty() {
            let _ = write!(out, " fixed={}", fixed.join(","));
        }
        if sketch.is_construction(id) {
            out.push_str(" construction");
        }
        if let Some(tag) = sketch.tag_of(id) {
            let _ = write!(out, " tag={tag:?}");
        }
        out.push('\n');
    }
    let _ = writeln!(out, "constraints: {}", constraints.len());
    for (id, c) in &constraints {
        let _ = write!(out, "  c{} {} {}", id.0, c.kind(), describe(c));
        if let Some(tag) = sketch.constraint_tag_of(*id) {
            let _ = write!(out, " tag={tag:?}");
        }
        out.push('\n');
    }
    if report.is_clean() {
        out.push_str("report: clean\n");
    } else {
        out.push_str("report:\n");
        for v in &report.violations {
            let what = match v.id {
                Source::Constraint(c) => format!("c{}", c.0),
                Source::Arc(a) => format!("arc #{}", a.0),
            };
            let _ = write!(out, "  violated {what} {:?}", v.kind);
            if let Some(tag) = &v.tag {
                let _ = write!(out, " tag={tag:?}");
            }
            let _ = writeln!(out, " measured={} wanted={}", v.measured, v.wanted);
        }
        for d in &report.degenerate {
            let _ = writeln!(out, "  degenerate #{} {:?} size={}", d.id.0, d.what, d.size);
        }
        for m in &report.missing {
            match m.coordinate {
                Some(c) => {
                    let _ = writeln!(out, "  missing #{} {c:?}", m.id.0);
                }
                None => {
                    let _ = writeln!(out, "  missing #{} (not in sketch)", m.id.0);
                }
            }
        }
    }
    out
}

fn constraint_label(c: &ConstraintRef) -> String {
    match &c.tag {
        Some(tag) => format!("c{} {tag:?}", c.id.0),
        None => format!("c{}", c.id.0),
    }
}

fn entity_label(e: &EntityRef) -> String {
    match &e.tag {
        Some(tag) => format!("#{} {tag:?}", e.id.0),
        None => format!("#{}", e.id.0),
    }
}

fn origin_label(o: &Origin) -> String {
    match o {
        Origin::Constraint(c) => constraint_label(c),
        Origin::Arc(e) => format!("arc {}", entity_label(e)),
    }
}

/// A solution as text, for [`solution_text`]: status, DoF, each entity's
/// DoF, the free directions, and every redundant, conflicting, degenerate and
/// ill-conditioned finding, and every flip of branch with its constraint ids and tags.
fn diagnostics_text(solution: &Solution) -> String {
    let d = &solution.diagnostics;
    let mut out = String::new();
    let _ = writeln!(out, "solution: {} dof={}", solution.status, d.dof);
    for e in &d.entities {
        let _ = writeln!(out, "  dof {} = {}", entity_label(&e.entity), e.dof);
    }
    for (i, direction) in d.free_directions.iter().enumerate() {
        let parts: Vec<String> = direction
            .iter()
            .map(|m| format!("#{}.{:?}={}", m.entity.0, m.coordinate, m.amount))
            .collect();
        let _ = writeln!(out, "  free {i}: {}", parts.join(" "));
    }
    for c in &d.redundant {
        let _ = writeln!(out, "  redundant {}", constraint_label(c));
    }
    for set in &d.conflicts {
        let parts: Vec<String> = set.iter().map(constraint_label).collect();
        let _ = writeln!(out, "  conflict {}", parts.join(", "));
    }
    for g in &d.degenerate {
        let eqs: Vec<String> = g.equations.iter().map(origin_label).collect();
        let _ = writeln!(
            out,
            "  degenerate {} {:?} size={} reads {}",
            entity_label(&g.entity),
            g.collapse,
            g.size,
            eqs.join(", ")
        );
    }
    for o in &d.ill_conditioned {
        let _ = writeln!(out, "  ill-conditioned {}", origin_label(o));
    }
    for f in &d.flips {
        let subject = match &f.subject {
            FlipSubject::Entity(e) => entity_label(e),
            FlipSubject::Constraint(c) => constraint_label(c),
        };
        let _ = writeln!(out, "  flip {subject} {} -> {}", f.was, f.now);
    }
    if let Some(b) = &d.bisection {
        let _ = writeln!(
            out,
            "  bisected {} halvings, reached ({}, {})",
            b.halvings, b.reached[0], b.reached[1]
        );
    }
    out
}

/// The sketch at the solution's coordinates as text ([`text`], with the
/// checker's report there), followed by what the solve established:
/// status, DoF, per-entity DoF, free directions, redundant and conflicting
/// constraints (by id and tag), degeneracies.
///
/// ```
/// use laman::model::Sketch;
/// use laman::solution::SolverOptions;
/// use laman_debug::dump::solution_text;
///
/// let mut sk = Sketch::new();
/// let a = sk.point_fixed(0.0, 0.0)?;
/// let b = sk.point(3.0, 1.0)?;
/// sk.distance(a, b, 5.0)?;
/// let out = solution_text(&sk, &sk.solve(&SolverOptions::default()));
/// assert!(out.contains("solution: UnderConstrained dof=1"));
/// # Ok::<(), laman::model::ModelError>(())
/// ```
pub fn solution_text(sketch: &Sketch, solution: &Solution) -> String {
    let report = check(sketch, &solution.coordinates);
    let mut out = text(sketch, &solution.coordinates, &report);
    out.push_str(&diagnostics_text(solution));
    out
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            other => out.push(other),
        }
    }
    out
}

/// What the picture is drawn in: y is flipped so counter-clockwise is
/// counter-clockwise on screen, and `top` is the largest y.
struct Frame {
    min_x: f64,
    top: f64,
    side: f64,
}

impl Frame {
    fn x(&self, x: f64) -> f64 {
        x - self.min_x
    }
    fn y(&self, y: f64) -> f64 {
        self.top - y
    }
}

/// The sketch at `values` as an SVG document, drawn to scale: lines, circles
/// and arcs (an arc counter-clockwise from its start to its end), points as
/// dots, construction geometry dashed, tags as labels, each constraint
/// marked at its entities with its tag or `c<id>`. What the report flags is
/// red: a violated constraint's mark and entities, a degenerate entity.
/// The view box is the bounds of the drawing plus a margin; y points up.
///
/// ```
/// use laman::model::Sketch;
/// use laman_check::check;
/// use laman_debug::dump::svg;
///
/// let mut sk = Sketch::new();
/// let (a, b) = (sk.point(0.0, 0.0)?, sk.point(4.0, 3.0)?);
/// sk.line(a, b)?;
/// let values = sk.coordinates();
/// let out = svg(&sk, &values, &check(&sk, &values));
/// assert!(out.starts_with("<svg"));
/// assert!(out.contains("<line"));
/// # Ok::<(), laman::model::ModelError>(())
/// ```
pub fn svg(sketch: &Sketch, values: &Coordinates, report: &Report) -> String {
    let mut red: BTreeSet<EntityId> = BTreeSet::new();
    let mut red_constraints = BTreeSet::new();
    for v in &report.violations {
        match v.id {
            Source::Constraint(c) => {
                red_constraints.insert(c);
                if let Some(constraint) = sketch.constraint(c) {
                    red.extend(constraint.entities());
                }
            }
            Source::Arc(a) => {
                red.insert(a);
            }
        }
    }
    red.extend(report.degenerate.iter().map(|d| d.id));

    // Bounds over points, and over each circle's extent.
    let (mut lo_x, mut hi_x, mut lo_y, mut hi_y) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    let mut grow = |x: f64, y: f64, r: f64| {
        lo_x = lo_x.min(x - r);
        hi_x = hi_x.max(x + r);
        lo_y = lo_y.min(y - r);
        hi_y = hi_y.max(y + r);
    };
    for (id, entity) in sketch.entities() {
        match entity {
            Entity::Point { .. } => {
                if let Some((x, y)) = point_at(values, id) {
                    grow(x, y, 0.0);
                }
            }
            Entity::Circle { c, .. } => {
                if let (Some((x, y)), Some(r)) =
                    (point_at(values, c.id()), value(values, id, Coordinate::R))
                {
                    grow(x, y, r.abs());
                }
            }
            Entity::Arc { c, s, .. } => {
                if let (Some((x, y)), Some((sx, sy))) =
                    (point_at(values, c.id()), point_at(values, s.id()))
                {
                    grow(x, y, libm::hypot(sx - x, sy - y));
                }
            }
            Entity::Line { .. } => {}
        }
    }
    if lo_x > hi_x {
        (lo_x, hi_x, lo_y, hi_y) = (0.0, 0.0, 0.0, 0.0);
    }
    let extent = (hi_x - lo_x).max(hi_y - lo_y).max(MIN_SIDE);
    let margin = extent * MARGIN;
    let frame = Frame {
        min_x: lo_x - margin,
        top: hi_y + margin,
        side: extent + 2.0 * margin,
    };
    let (width, height) = ((hi_x - lo_x) + 2.0 * margin, (hi_y - lo_y) + 2.0 * margin);
    let (width, height) = (width.max(MIN_SIDE), height.max(MIN_SIDE));
    let stroke = frame.side * STROKE;
    let label = frame.side * LABEL;
    let dash = frame.side * DASH;
    let dot = frame.side * DOT;

    let mut out = String::new();
    let _ = writeln!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {width:.4} {height:.4}\" \
         width=\"800\" font-family=\"monospace\" font-size=\"{label:.4}\">"
    );
    let paint = |id: EntityId| if red.contains(&id) { "red" } else { "black" };
    let style = |id: EntityId| {
        let dashed = if sketch.is_construction(id) {
            format!(" stroke-dasharray=\"{dash:.4} {dash:.4}\"")
        } else {
            String::new()
        };
        format!(
            "stroke=\"{}\" stroke-width=\"{stroke:.4}\" fill=\"none\"{dashed}",
            paint(id)
        )
    };
    for (id, entity) in sketch.entities() {
        match entity {
            Entity::Point { .. } => {}
            Entity::Line { a, b } => {
                if let (Some((ax, ay)), Some((bx, by))) =
                    (point_at(values, a.id()), point_at(values, b.id()))
                {
                    let _ = writeln!(
                        out,
                        "<line x1=\"{:.4}\" y1=\"{:.4}\" x2=\"{:.4}\" y2=\"{:.4}\" {}/>",
                        frame.x(ax),
                        frame.y(ay),
                        frame.x(bx),
                        frame.y(by),
                        style(id)
                    );
                }
            }
            Entity::Circle { c, .. } => {
                if let (Some((x, y)), Some(r)) =
                    (point_at(values, c.id()), value(values, id, Coordinate::R))
                {
                    let _ = writeln!(
                        out,
                        "<circle cx=\"{:.4}\" cy=\"{:.4}\" r=\"{:.4}\" {}/>",
                        frame.x(x),
                        frame.y(y),
                        r.abs(),
                        style(id)
                    );
                }
            }
            Entity::Arc { c, s, e } => {
                if let (Some((cx, cy)), Some((sx, sy)), Some((ex, ey))) = (
                    point_at(values, c.id()),
                    point_at(values, s.id()),
                    point_at(values, e.id()),
                ) {
                    let r = libm::hypot(sx - cx, sy - cy);
                    // Counter-clockwise from s to e: the sweep exceeds a half
                    // turn exactly when cross(s - c, e - c) is negative. With y
                    // flipped it is drawn with sweep-flag 0.
                    let cross = (sx - cx) * (ey - cy) - (sy - cy) * (ex - cx);
                    let large = u8::from(cross < 0.0);
                    let _ = writeln!(
                        out,
                        "<path d=\"M {:.4} {:.4} A {r:.4} {r:.4} 0 {large} 0 {:.4} {:.4}\" {}/>",
                        frame.x(sx),
                        frame.y(sy),
                        frame.x(ex),
                        frame.y(ey),
                        style(id)
                    );
                }
            }
        }
    }
    for (id, entity) in sketch.entities() {
        if let Entity::Point { .. } = entity
            && let Some((x, y)) = point_at(values, id)
        {
            let _ = writeln!(
                out,
                "<circle cx=\"{:.4}\" cy=\"{:.4}\" r=\"{dot:.4}\" fill=\"{}\"/>",
                frame.x(x),
                frame.y(y),
                paint(id)
            );
        }
        if let Some(tag) = sketch.tag_of(id)
            && let Some((x, y)) = anchor(sketch, values, id)
        {
            let _ = writeln!(
                out,
                "<text x=\"{:.4}\" y=\"{:.4}\" fill=\"{}\">{}</text>",
                frame.x(x) + dot * 2.0,
                frame.y(y) - dot * 2.0,
                paint(id),
                escape(tag)
            );
        }
    }
    for (id, constraint) in sketch.constraints() {
        let mut sum = (0.0, 0.0);
        let mut n = 0.0;
        for e in constraint.entities() {
            if let Some((x, y)) = anchor(sketch, values, e) {
                sum = (sum.0 + x, sum.1 + y);
                n += 1.0;
            }
        }
        if n > 0.0 {
            let name = sketch
                .constraint_tag_of(id)
                .map_or_else(|| format!("c{}", id.0), str::to_string);
            let colour = if red_constraints.contains(&id) {
                "red"
            } else {
                "blue"
            };
            let _ = writeln!(
                out,
                "<text x=\"{:.4}\" y=\"{:.4}\" fill=\"{colour}\">{}</text>",
                frame.x(sum.0 / n),
                frame.y(sum.1 / n),
                escape(&name)
            );
        }
    }
    out.push_str("</svg>\n");
    out
}

/// Where an entity is marked: a point, a line's middle, a round's centre.
fn anchor(sketch: &Sketch, values: &Coordinates, id: EntityId) -> Option<(f64, f64)> {
    match sketch.entity(id)? {
        Entity::Point { .. } => point_at(values, id),
        Entity::Line { a, b } => {
            let (ax, ay) = point_at(values, a.id())?;
            let (bx, by) = point_at(values, b.id())?;
            Some(((ax + bx) / 2.0, (ay + by) / 2.0))
        }
        Entity::Circle { c, .. } | Entity::Arc { c, .. } => point_at(values, c.id()),
    }
}
