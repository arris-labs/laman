//! Drag sessions: [`Sketch::drag`] and the [`DragSession`] it returns
//! (`docs/ARCHITECTURE.md` §Drag sessions).
//!
//! ```
//! use laman::model::Sketch;
//! use laman::solution::SolverOptions;
//!
//! let mut sk = Sketch::new();
//! let o = sk.point_fixed(0.0, 0.0)?;
//! let p = sk.point(3.0, 4.0)?;
//! sk.distance(o, p, 5.0)?;
//! let mut drag = sk.drag(p.id(), &SolverOptions::default())?;
//! let step = drag.to([0.0, 10.0]); // the rod swings toward the target
//! assert!((step.coordinates[&(p.id(), laman::model::Coordinate::Y)] - 5.0).abs() < 1e-9);
//! let done = drag.finish();
//! sk.apply(&done)?;
//! # Ok::<(), laman::model::ModelError>(())
//! ```

use crate::branch::Observations;
use crate::model::{Coordinate, EntityId, ModelError, Sketch};
use crate::solution::{Provenance, Solution, SolverOptions, Status, solve_observed};
use crate::system::Goal;

/// A point being dragged through a series of targets, one frame per
/// [`DragSession::to`].
///
/// The session owns a copy of the sketch, so the caller's stays untouched
/// (and editable) until it applies [`DragSession::finish`]'s solution. Each
/// step holds the target as a goal the sketch's constraints are never
/// pushed off by: the required equations are solved first and the target is
/// pursued only in the freedom they leave (`docs/ARCHITECTURE.md`
/// §Priorities), starting from the previous step with least change.
/// Geometry that no equation links to the dragged point keeps its bits.
/// Every solution a session returns claims the sketch the session was
/// opened on, so [`Sketch::apply`] accepts any of them on it, unedited.
///
/// ```
/// use laman::model::Sketch;
/// use laman::solution::{SolverOptions, Status};
///
/// let mut sk = Sketch::new();
/// let p = sk.point(1.0, 1.0)?;
/// let mut drag = sk.drag(p.id(), &SolverOptions::default())?;
/// let step = drag.to([4.0, 2.0]);
/// assert_eq!(step.status, Status::UnderConstrained);
/// # Ok::<(), laman::model::ModelError>(())
/// ```
#[derive(Clone, Debug)]
pub struct DragSession {
    /// The caller's sketch with the last solution's coordinates placed: the
    /// warm start of the next step.
    sketch: Sketch,
    point: EntityId,
    options: SolverOptions,
    /// The caller's sketch, which every solution claims.
    source: Provenance,
    last: Solution,
    /// The branches `last` sits on.
    observed: Observations,
}

impl Sketch {
    /// Opens a drag session on a point: the sketch solved once as it is, the
    /// session's first "last solution".
    ///
    /// Refuses an unknown id (`UnknownId`), an entity that is no point
    /// (`WrongKind`) and a point fixed in both coordinates (`FixedPoint`). A
    /// point with one fixed coordinate drags along the other.
    ///
    /// ```
    /// use laman::model::{ModelError, Sketch};
    /// use laman::solution::SolverOptions;
    /// let mut sk = Sketch::new();
    /// let ground = sk.point_fixed(0.0, 0.0)?;
    /// let o = SolverOptions::default();
    /// assert!(matches!(sk.drag(ground.id(), &o), Err(ModelError::FixedPoint { .. })));
    /// # Ok::<(), ModelError>(())
    /// ```
    pub fn drag(
        &self,
        point: EntityId,
        options: &SolverOptions,
    ) -> Result<DragSession, ModelError> {
        let handle = self.point_handle(point)?;
        let id = handle.id();
        if self.is_fixed(id, Coordinate::X) && self.is_fixed(id, Coordinate::Y) {
            return Err(ModelError::FixedPoint { id });
        }
        let source = self.provenance();
        let before = Observations::at(self, &self.coordinates());
        let (last, observed) = solve_observed(self, &[], options, &before, source);
        let mut sketch = self.clone();
        sketch.place(&last.coordinates);
        Ok(DragSession {
            sketch,
            point: id,
            options: *options,
            source,
            last,
            observed,
        })
    }
}

impl DragSession {
    /// One frame: solves with the point pulled toward `target`, from the
    /// previous step, and returns the full [`Solution`].
    ///
    /// Always returns a solution. The target is a goal, never a constraint:
    /// one the sketch makes unreachable ends at the nearest reachable place,
    /// `UnderConstrained` or better, not `NotConverged`. A target that is
    /// not finite is refused as a frame: the last solution's coordinates
    /// come back with `Status::NotConverged`.
    ///
    /// ```
    /// use laman::model::{Coordinate, Sketch};
    /// use laman::solution::{SolverOptions, Status};
    /// let mut sk = Sketch::new();
    /// let p = sk.point(0.0, 0.0)?;
    /// let mut drag = sk.drag(p.id(), &SolverOptions::default())?;
    /// let bad = drag.to([f64::NAN, 0.0]);
    /// assert_eq!(bad.status, Status::NotConverged);
    /// assert_eq!(bad.coordinates[&(p.id(), Coordinate::X)], 0.0);
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn to(&mut self, target: [f64; 2]) -> Solution {
        if !(target[0].is_finite() && target[1].is_finite()) {
            let mut kept = self.last.clone();
            kept.status = Status::NotConverged;
            return kept;
        }
        let goals = [
            Goal {
                param: (self.point, Coordinate::X),
                target: target[0],
            },
            Goal {
                param: (self.point, Coordinate::Y),
                target: target[1],
            },
        ];
        let (solution, observed) = solve_observed(
            &self.sketch,
            &goals,
            &self.options,
            &self.observed,
            self.source,
        );
        self.sketch.place(&solution.coordinates);
        self.observed = observed;
        self.last = solution.clone();
        solution
    }

    /// Ends the session and returns its last solution, which
    /// [`Sketch::apply`] accepts on the sketch the session was opened on if
    /// that has not been edited since.
    ///
    /// ```
    /// use laman::model::Sketch;
    /// use laman::solution::SolverOptions;
    /// let mut sk = Sketch::new();
    /// let p = sk.point(0.0, 0.0)?;
    /// let mut drag = sk.drag(p.id(), &SolverOptions::default())?;
    /// drag.to([2.0, 3.0]);
    /// let done = drag.finish();
    /// sk.apply(&done)?;
    /// assert_eq!(sk.coordinates()[&(p.id(), laman::model::Coordinate::X)], 2.0);
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn finish(self) -> Solution {
        self.last
    }
}
