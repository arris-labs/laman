//! C2 sketches built from closed-form geometry, with deterministic displaced
//! guesses and independent expectations. This Rust-only tooling does not
//! change the solver or the C1 fixtures.

use crate::fixture::{Drag, DragStep, Expected};
use laman::model::{ConstraintId, Coordinate, Coordinates, EntityId, ModelError, Point, Sketch};

/// A generated static sketch, a dimension edit, and an optional drag.
#[derive(Debug)]
pub struct Case {
    /// Coordinates are displaced guesses, not solved coordinates.
    pub sketch: Sketch,
    /// Closed-form static answers.
    pub expected: Expected,
    /// One dimension changed in place by removing and re-adding its id.
    pub edit: Edit,
    /// A separate sketch with one dimension removed, starting at the construction.
    pub drag: Option<DragCase>,
}

/// One dimension edit and its closed-form answers.
#[derive(Debug)]
pub struct Edit {
    /// The unique tag of the dimension to replace.
    pub tag: String,
    /// The dimension's new value.
    pub value: f64,
    /// Closed-form answers after the edit.
    pub expected: Expected,
    /// Entities upstream of the edit: their solved coordinates must keep their bits.
    pub unmoved: Vec<EntityId>,
}

/// A generated drag, with its removed dimension and initial answers explicit.
#[derive(Debug)]
pub struct DragCase {
    /// The construction with one dimension removed; starts at exact coordinates.
    pub sketch: Sketch,
    /// Initial answers (one DoF).
    pub expected: Expected,
    /// Targets and every step's closed-form coordinates.
    pub drag: Drag,
}

/// A refused construction, before any solve.
#[derive(Debug, thiserror::Error)]
pub enum CorpusError {
    /// A family needs at least one cell or edge.
    #[error("{0} must be positive")]
    Count(&'static str),
    /// A length must be positive and finite.
    #[error("{name} must be positive and finite, got {value}")]
    Dimension {
        /// The dimension's name.
        name: &'static str,
        /// The refused value.
        value: f64,
    },
    /// The derived geometry is not representable by the model.
    #[error(transparent)]
    Model(#[from] ModelError),
}

// Coordinate acceptance is the C1 fixture requirement, independent of solver tolerance.
const COORDINATE_TOLERANCE: f64 = 1e-9;
// Each coordinate moves at most .03 of the smallest dimension, below the plan's tenth.
const GUESS_FRACTION: f64 = 0.01;
// Edits and the last drag target increase the selected dimension by one tenth.
const EDIT_FRACTION: f64 = 0.1;
// Three small steps exercise warm starts without an expensive drag trajectory.
const DRAG_STEPS: usize = 3;

fn dimension(name: &'static str, value: f64) -> Result<(), CorpusError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(CorpusError::Dimension { name, value })
    }
}

fn expected(coordinates: Coordinates, dof: u32) -> Expected {
    Expected {
        status: if dof == 0 {
            "FullyConstrained"
        } else {
            "UnderConstrained"
        }
        .into(),
        dof,
        coordinates,
        tolerance: COORDINATE_TOLERANCE,
        redundant: Vec::new(),
        conflicts: Vec::new(),
        flips: Vec::new(),
        digest: None,
    }
}

fn point(
    sketch: &mut Sketch,
    coordinates: &mut Coordinates,
    xy: [f64; 2],
    smallest: f64,
    displaced: bool,
    fixed: bool,
) -> Result<Point, ModelError> {
    let index = sketch.entities().count();
    let mut guess = xy;
    if displaced && !fixed {
        for (axis, v) in guess.iter_mut().enumerate() {
            let offset = ((index + axis * 3) % 7) as f64 - 3.0;
            *v += GUESS_FRACTION * smallest * offset;
        }
    }
    let p = if fixed {
        sketch.point_fixed(guess[0], guess[1])?
    } else {
        sketch.point(guess[0], guess[1])?
    };
    coordinates.insert((p.id(), Coordinate::X), xy[0]);
    coordinates.insert((p.id(), Coordinate::Y), xy[1]);
    Ok(p)
}

struct RectangleBuild {
    sketch: Sketch,
    coordinates: Coordinates,
    dimension: ConstraintId,
    dragged: EntityId,
    unmoved: Vec<EntityId>,
}

fn rectangles(
    n: usize,
    width: f64,
    height: f64,
    changed: f64,
    displaced: bool,
) -> Result<RectangleBuild, CorpusError> {
    let mut sketch = Sketch::new();
    let mut coordinates = Coordinates::new();
    let smallest = width.min(height);
    let mut bottom = point(
        &mut sketch,
        &mut coordinates,
        [0.0, 0.0],
        smallest,
        displaced,
        true,
    )?;
    let mut top = point(
        &mut sketch,
        &mut coordinates,
        [0.0, height],
        smallest,
        displaced,
        false,
    )?;
    let left = sketch.line(bottom, top)?;
    sketch.vertical(left)?;
    sketch.distance(bottom, top, height)?;
    let mut selected = None;
    let mut unmoved = Vec::new();
    let mut x = 0.0;
    for i in 0..n {
        if i == n / 2 {
            unmoved = sketch.entities().map(|(id, _)| id).collect();
        }
        let w = if i == n / 2 { changed } else { width };
        x += w;
        let next_bottom = point(
            &mut sketch,
            &mut coordinates,
            [x, 0.0],
            smallest,
            displaced,
            false,
        )?;
        let next_top = point(
            &mut sketch,
            &mut coordinates,
            [x, height],
            smallest,
            displaced,
            false,
        )?;
        let lower = sketch.line(bottom, next_bottom)?;
        let upper = sketch.line(top, next_top)?;
        let right = sketch.line(next_bottom, next_top)?;
        sketch.horizontal(lower)?;
        sketch.horizontal(upper)?;
        sketch.vertical(right)?;
        let id = sketch.distance(bottom, next_bottom, w)?;
        sketch.tag_constraint(id, &format!("width-{i}"))?;
        if i == n / 2 {
            selected = Some((id, next_top.id()));
        }
        bottom = next_bottom;
        top = next_top;
    }
    // The public constructor rejects n == 0; use a typed error even if called incorrectly internally.
    let (dimension, dragged) = selected.ok_or(CorpusError::Count("rectangles"))?;
    Ok(RectangleBuild {
        sketch,
        coordinates,
        dimension,
        dragged,
        unmoved,
    })
}

/// Builds `n` adjacent 20 by 10 rectangles, with shared edges (5n + 3 entities).
/// Refuses zero rectangles or unrepresentable geometry.
///
/// ```
/// let case = laman_debug::corpus::rect_chain(3)?;
/// assert_eq!(case.sketch.entities().count(), 18);
/// # Ok::<(), laman_debug::corpus::CorpusError>(())
/// ```
pub fn rect_chain(n: usize) -> Result<Case, CorpusError> {
    rect_chain_with(n, 20.0, 10.0)
}

/// Builds a rectangle chain with positive finite width and height.
/// Rectangle `n / 2` is edited and dragged; all previous entities are upstream.
///
/// ```
/// let case = laman_debug::corpus::rect_chain_with(3, 30.0, 12.0)?;
/// assert_eq!(case.edit.tag, "width-1");
/// # Ok::<(), laman_debug::corpus::CorpusError>(())
/// ```
pub fn rect_chain_with(n: usize, width: f64, height: f64) -> Result<Case, CorpusError> {
    if n == 0 {
        return Err(CorpusError::Count("rectangles"));
    }
    dimension("width", width)?;
    dimension("height", height)?;
    let built = rectangles(n, width, height, width, true)?;
    let new_value = width * (1.0 + EDIT_FRACTION);
    let edited = rectangles(n, width, height, new_value, false)?;
    let mut free = rectangles(n, width, height, width, false)?;
    free.sketch.remove_constraint(free.dimension)?;
    let mut targets = Vec::new();
    let mut after = Vec::new();
    for step in 1..=DRAG_STEPS {
        let value = width * (1.0 + EDIT_FRACTION * step as f64 / DRAG_STEPS as f64);
        let moved = rectangles(n, width, height, value, false)?;
        targets.push([moved.coordinates[&(built.dragged, Coordinate::X)], height]);
        after.push(DragStep {
            step,
            coordinates: moved.coordinates,
            flips: Vec::new(),
        });
    }
    Ok(Case {
        sketch: built.sketch,
        expected: expected(built.coordinates, 0),
        edit: Edit {
            tag: format!("width-{}", n / 2),
            value: new_value,
            expected: expected(edited.coordinates, 0),
            unmoved: built.unmoved,
        },
        drag: Some(DragCase {
            sketch: free.sketch,
            expected: expected(free.coordinates, 1),
            drag: Drag {
                dragged: built.dragged,
                targets,
                after,
            },
        }),
    })
}

struct SlotBuild {
    sketch: Sketch,
    coordinates: Coordinates,
    horizontal: ConstraintId,
    dragged: EntityId,
    unmoved: Vec<EntityId>,
    placement: [f64; 2],
}

fn slots(
    rows: usize,
    cols: usize,
    length: f64,
    radius: f64,
    shift: [f64; 2],
    displaced: bool,
) -> Result<SlotBuild, CorpusError> {
    let count = rows
        .checked_mul(cols)
        .ok_or(CorpusError::Count("slot count"))?;
    let selected = count / 2;
    // Leave a diameter between rows and enough room for a tenth-placement edit.
    let pitch = [2.0 * length + 4.0 * radius, 6.0 * radius];
    let margin = 4.0 * radius;
    let plate_width = cols as f64 * pitch[0] + margin;
    let plate_height = rows as f64 * pitch[1] + margin;
    let mut sketch = Sketch::new();
    let mut coordinates = Coordinates::new();
    let smallest = length.min(radius);
    let p0 = point(
        &mut sketch,
        &mut coordinates,
        [0.0, 0.0],
        smallest,
        displaced,
        true,
    )?;
    let p1 = point(
        &mut sketch,
        &mut coordinates,
        [plate_width, 0.0],
        smallest,
        displaced,
        false,
    )?;
    let p2 = point(
        &mut sketch,
        &mut coordinates,
        [plate_width, plate_height],
        smallest,
        displaced,
        false,
    )?;
    let p3 = point(
        &mut sketch,
        &mut coordinates,
        [0.0, plate_height],
        smallest,
        displaced,
        false,
    )?;
    let bottom = sketch.line(p0, p1)?;
    let right = sketch.line(p1, p2)?;
    let top = sketch.line(p2, p3)?;
    let left = sketch.line(p3, p0)?;
    sketch.horizontal(bottom)?;
    sketch.vertical(right)?;
    sketch.horizontal(top)?;
    sketch.vertical(left)?;
    sketch.distance(p0, p1, plate_width)?;
    sketch.distance(p0, p3, plate_height)?;
    let mut unmoved: Vec<_> = sketch.entities().map(|(id, _)| id).collect();
    let mut selection = None;
    for i in 0..count {
        let start = sketch.entities().count();
        let placement = [
            margin + (i % cols) as f64 * pitch[0],
            margin + (i / cols) as f64 * pitch[1],
        ];
        let x = placement[0] + if i == selected { shift[0] } else { 0.0 };
        let y = placement[1] + if i == selected { shift[1] } else { 0.0 };
        let c1 = point(
            &mut sketch,
            &mut coordinates,
            [x, y],
            smallest,
            displaced,
            false,
        )?;
        let s1 = point(
            &mut sketch,
            &mut coordinates,
            [x, y + radius],
            smallest,
            displaced,
            false,
        )?;
        let e1 = point(
            &mut sketch,
            &mut coordinates,
            [x, y - radius],
            smallest,
            displaced,
            false,
        )?;
        let c2 = point(
            &mut sketch,
            &mut coordinates,
            [x + length, y],
            smallest,
            displaced,
            false,
        )?;
        let s2 = point(
            &mut sketch,
            &mut coordinates,
            [x + length, y - radius],
            smallest,
            displaced,
            false,
        )?;
        let e2 = point(
            &mut sketch,
            &mut coordinates,
            [x + length, y + radius],
            smallest,
            displaced,
            false,
        )?;
        let a1 = sketch.arc(c1, s1, e1)?;
        let a2 = sketch.arc(c2, s2, e2)?;
        sketch.tag(a1.id(), &format!("slot-{i}-left"))?;
        sketch.tag(a2.id(), &format!("slot-{i}-right"))?;
        let upper = sketch.line(e2, s1)?;
        let lower = sketch.line(e1, s2)?;
        sketch.tangent_line_circle(upper, a1)?;
        sketch.tangent_line_circle(upper, a2)?;
        sketch.tangent_line_circle(lower, a1)?;
        sketch.tangent_line_circle(lower, a2)?;
        sketch.equal_radius(a1, a2)?;
        sketch.radius(a1, radius)?;
        sketch.horizontal((c1, c2))?;
        sketch.distance(c1, c2, length)?;
        let horizontal = sketch.point_line_distance(c1, left, x)?;
        let vertical = sketch.point_line_distance(c1, bottom, y)?;
        sketch.tag_constraint(horizontal, &format!("slot-{i}-x"))?;
        sketch.tag_constraint(vertical, &format!("slot-{i}-height"))?;
        if i == selected {
            selection = Some((horizontal, c1.id(), placement));
        } else {
            unmoved.extend(sketch.entities().skip(start).map(|(id, _)| id));
        }
    }
    let (horizontal, dragged, placement) = selection.ok_or(CorpusError::Count("slots"))?;
    Ok(SlotBuild {
        sketch,
        coordinates,
        horizontal,
        dragged,
        unmoved,
        placement,
    })
}

/// Builds a grounded plate with `rows * cols` slots (10 entities each, plus 8).
/// Slots have radius 5 and centres 20 apart, matching C1's slot geometry.
/// Refuses zero rows/columns or unrepresentable geometry.
///
/// ```
/// let case = laman_debug::corpus::slot_grid(2, 2)?;
/// assert_eq!(case.sketch.entities().count(), 48);
/// # Ok::<(), laman_debug::corpus::CorpusError>(())
/// ```
pub fn slot_grid(rows: usize, cols: usize) -> Result<Case, CorpusError> {
    slot_grid_with(rows, cols, 20.0, 5.0)
}

/// Builds a slot grid with positive finite centre separation and radius.
/// The middle slot's vertical placement increases by one tenth on edit;
/// the drag removes its horizontal placement and pulls its left centre.
///
/// ```
/// let case = laman_debug::corpus::slot_grid_with(2, 2, 40.0, 8.0)?;
/// assert_eq!(case.edit.tag, "slot-2-height");
/// # Ok::<(), laman_debug::corpus::CorpusError>(())
/// ```
pub fn slot_grid_with(
    rows: usize,
    cols: usize,
    length: f64,
    radius: f64,
) -> Result<Case, CorpusError> {
    if rows == 0 {
        return Err(CorpusError::Count("rows"));
    }
    if cols == 0 {
        return Err(CorpusError::Count("columns"));
    }
    dimension("length", length)?;
    dimension("radius", radius)?;
    let built = slots(rows, cols, length, radius, [0.0, 0.0], true)?;
    let edited = slots(
        rows,
        cols,
        length,
        radius,
        [0.0, built.placement[1] * EDIT_FRACTION],
        false,
    )?;
    let mut free = slots(rows, cols, length, radius, [0.0, 0.0], false)?;
    free.sketch.remove_constraint(free.horizontal)?;
    let mut targets = Vec::new();
    let mut after = Vec::new();
    for step in 1..=DRAG_STEPS {
        let dx = length * EDIT_FRACTION * step as f64 / DRAG_STEPS as f64;
        let moved = slots(rows, cols, length, radius, [dx, 0.0], false)?;
        targets.push([built.placement[0] + dx, built.placement[1]]);
        after.push(DragStep {
            step,
            coordinates: moved.coordinates,
            flips: Vec::new(),
        });
    }
    let selected = rows * cols / 2;
    Ok(Case {
        sketch: built.sketch,
        expected: expected(built.coordinates, 0),
        edit: Edit {
            tag: format!("slot-{selected}-height"),
            value: edited.placement[1] * (1.0 + EDIT_FRACTION),
            expected: expected(edited.coordinates, 0),
            unmoved: built.unmoved,
        },
        drag: Some(DragCase {
            sketch: free.sketch,
            expected: expected(free.coordinates, 1),
            drag: Drag {
                dragged: built.dragged,
                targets,
                after,
            },
        }),
    })
}
