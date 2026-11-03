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
