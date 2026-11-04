//! Solving a sketch and applying the answer: [`Sketch::solve`],
//! [`Sketch::apply`], [`SolverOptions`], and the [`Solution`] with its
//! [`Status`] and [`Diagnostics`] (`docs/ARCHITECTURE.md` §Solve,
//! `docs/DATA-MODEL.md` §Diagnostics).
//!
//! ```
//! use laman::model::Sketch;
//! use laman::solution::{SolverOptions, Status};
//!
//! let mut sk = Sketch::new();
//! let o = sk.point_fixed(0.0, 0.0)?;
//! let p = sk.point(3.0, 3.0)?;
//! let l = sk.line(o, p)?;
//! sk.horizontal(l)?;
//! sk.distance(o, p, 5.0)?;
//! let solution = sk.solve(&SolverOptions::default());
//! assert_eq!(solution.status, Status::FullyConstrained);
//! sk.apply(&solution)?;
//! # Ok::<(), laman::model::ModelError>(())
//! ```

use crate::branch::{self, Observations};
use crate::diagnose::{self, Diagnosis};
use crate::model::tolerance::Source;
use crate::model::{
    ConstraintId, ConstraintKind, Coordinate, Coordinates, EntityId, EntityKind, ModelError, Sketch,
};
use crate::solve::{self, Options, Outcome, Path};
use crate::stage::Decompositions;
use crate::system::Goal;

#[cfg(feature = "serde")]
mod form;

pub use crate::diagnose::{Collapse, Status};
pub use crate::solve::Algorithm;

/// How a solve runs: the algorithm and its hard caps.
///
/// Every cap is a promise: a solve that exhausts one returns its best
/// iterate and `Status::NotConverged`, never an error and never a hang. The
/// defaults are measured (`docs/ARCHITECTURE.md` §LM engine, §DogLeg engine,
/// §Diagnose). Fields are public to read and set; the struct is
/// `#[non_exhaustive]`, so build it from [`Default`].
///
/// ```
/// use laman::solution::{Algorithm, SolverOptions};
/// let mut options = SolverOptions::default();
/// options.algorithm = Algorithm::DogLeg;
/// assert!(options.max_iterations > 0);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct SolverOptions {
    /// The step each iteration proposes. The conflict analysis re-solves
    /// its tests by Levenberg–Marquardt whatever this is, from this solve's
    /// best effort and under its caps.
    pub algorithm: Algorithm,
    /// Accepted and rejected steps in all, across components. A component
    /// solved by stages spends the share of its parameters a step moves, so
    /// the cap bounds its work as a whole component's steps would. A staged
    /// component that fails is solved again whole from its start with its
    /// counters restored, so its work can reach twice the cap.
    pub max_iterations: usize,
    /// Residual and Jacobian evaluations in all, across components, spent
    /// the same way.
    pub max_evaluations: usize,
    /// Re-solves the conflict analysis may spend in all; running out names
    /// no conflict and leaves the status `NotConverged`.
    pub max_analysis_solves: usize,
    /// Halvings of a drag step that flips a branch or jumps an arc's sweep,
    /// toward the last good target, before the step gives up and stays put
    /// (ADR-0006). Zero tries the target alone. A static solve ignores it.
    pub max_bisections: usize,
}

/// Default halvings of a flipping drag step: the step shrinks to 1/256, a
/// fraction of a pixel of a frame's motion, so a flip refused at that size
/// is a boundary the point sits on rather than one it skips over; the
/// most a refused frame costs is this many extra solves
/// (`docs/ARCHITECTURE.md` §Drag sessions).
const MAX_BISECTIONS: usize = 8;

impl Default for SolverOptions {
    /// Levenberg–Marquardt with the measured caps.
    fn default() -> Self {
        let internal = Options::default();
        Self {
            algorithm: internal.algorithm,
            max_iterations: internal.max_iterations,
            max_evaluations: internal.max_evaluations,
            max_analysis_solves: internal.max_analysis_solves,
            max_bisections: MAX_BISECTIONS,
        }
    }
}

impl SolverOptions {
    fn internal_on(&self, path: Path) -> Options {
        Options {
            algorithm: self.algorithm,
            max_iterations: self.max_iterations,
            max_evaluations: self.max_evaluations,
            max_analysis_solves: self.max_analysis_solves,
            path,
            ..Options::default()
        }
    }
}

/// [`Sketch::solve`] by `path`.
pub(crate) fn solve_on(sketch: &Sketch, options: &SolverOptions, path: Path) -> Solution {
    let before = Observations::at(sketch, &sketch.coordinates());
    solve_observed(sketch, &[], options, &before, sketch.provenance(), path).0
}

/// An entity as a diagnostic names it: the caller's id, its kind and its tag.
///
/// ```
/// use laman::model::Sketch;
/// use laman::solution::SolverOptions;
/// let mut sk = Sketch::new();
/// let p = sk.point(1.0, 2.0)?;
/// sk.tag(p.id(), "corner")?;
/// let solution = sk.solve(&SolverOptions::default());
/// assert_eq!(solution.diagnostics.entities[0].entity.tag.as_deref(), Some("corner"));
/// # Ok::<(), laman::model::ModelError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntityRef {
    /// The entity's id.
    pub id: EntityId,
    /// Its kind.
    pub kind: EntityKind,
    /// Its tag, when it has one.
    pub tag: Option<String>,
}

/// A constraint as a diagnostic names it: the caller's id, its kind and
/// its tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConstraintRef {
    /// The constraint's id.
    pub id: ConstraintId,
    /// Its kind.
    pub kind: ConstraintKind,
    /// Its tag, when it has one.
    pub tag: Option<String>,
}

/// Where an equation comes from: a constraint, or an arc's own equation
/// (that its two end-points are one radius from its centre).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    /// An equation of this constraint.
    Constraint(ConstraintRef),
    /// The intrinsic equation of this arc.
    Arc(EntityRef),
}

/// An entity's remaining freedom.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntityFreedom {
    /// The entity.
    pub entity: EntityRef,
    /// Its degrees of freedom: 0 when it cannot move relative to the ground.
    pub dof: usize,
}

/// One parameter's share of a free direction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    /// The entity that moves.
    pub entity: EntityId,
    /// The parameter of it.
    pub coordinate: Coordinate,
    /// Its component of the unit direction; never within rounding of zero.
    pub amount: f64,
}

/// An entity that collapsed at the solution, and what that makes
/// ill-conditioned.
#[derive(Clone, Debug, PartialEq)]
pub struct Degenerate {
    /// The collapsed entity.
    pub entity: EntityRef,
    /// How it collapsed.
    pub collapse: Collapse,
    /// Its size at the solution, in the sketch's units.
    pub size: f64,
    /// The equations that read it.
    pub equations: Vec<Origin>,
}

/// What a solve establishes about the sketch (DATA-MODEL.md §Diagnostics).
///
/// Everything here is deterministic and ordered: entities by id, free
/// directions component by component in compiled order, redundant
/// constraints by id, conflict sets in the order found and each in source
/// order.
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostics {
    /// Total geometric degrees of freedom; whole-sketch motions count.
    pub dof: usize,
    /// Every entity's DoF, in id order.
    pub entities: Vec<EntityFreedom>,
    /// An orthonormal basis of the free motions, each as the parameters it
    /// moves.
    pub free_directions: Vec<Vec<Motion>>,
    /// Satisfied constraints whose removal leaves the DoF unchanged.
    pub redundant: Vec<ConstraintRef>,
    /// Inclusion-minimal sets of constraints established to be jointly
    /// unsatisfiable (ADR-0005). Not every conflict is listed.
    pub conflicts: Vec<Vec<ConstraintRef>>,
    /// Entities collapsed below the length tolerance.
    pub degenerate: Vec<Degenerate>,
    /// Equations undefined or non-finite at the solution.
    pub ill_conditioned: Vec<Origin>,
    /// Branches the solution left that no equation holds (ADR-0006): empty
    /// when it sits on the branch the sketch was drawn on. A static solve
    /// compares with the guesses. In subject order: entities by id, then
    /// constraints by id.
    pub flips: Vec<Flip>,
    /// Set on a drag frame that was halved toward its target (ADR-0006): see
    /// [`Bisection`]. `None` for a static solve and for a frame solved at
    /// its own target.
    pub bisection: Option<Bisection>,
}

/// A drag frame that was bisected: halved toward the last target because
/// the full step flipped a branch or moved an arc's sweep past
/// `tolerance::MAX_SWEEP_STEP` (ADR-0006).
///
/// A frame the session stayed put on, because every halving flipped, is
/// bisected too: `reached` is then the target of the last halving tried and
/// `halvings` their number, and the coordinates are the previous frame's.
///
/// ```
/// use laman::model::{Coordinate, Sketch};
/// use laman::solution::SolverOptions;
///
/// // A line from the fixed (0, 1) to a tip held parallel to the fixed
/// // horizontal line (0, 0)-(1, 0): the tip may not cross x = 0.
/// let mut sk = Sketch::new();
/// let o = sk.point_fixed(0.0, 0.0)?;
/// let e = sk.point_fixed(1.0, 0.0)?;
/// let ground = sk.line(o, e)?;
/// let top = sk.point_fixed(0.0, 1.0)?;
/// let tip = sk.point(3.0, 1.0)?;
/// let arm = sk.line(top, tip)?;
/// sk.parallel(ground, arm)?;
/// let mut drag = sk.drag(tip.id(), &SolverOptions::default())?;
///
/// let frame = drag.to([2.0, 1.0]);
/// assert!(frame.diagnostics.bisection.is_none()); // solved at its own target
///
/// let frame = drag.to([-1.0, 1.0]); // would cross: halved once, to x = 0.5
/// let bisection = frame.diagnostics.bisection.unwrap();
/// assert_eq!(bisection.halvings, 1);
/// assert_eq!(bisection.reached, [0.5, 1.0]);
/// assert!((frame.coordinates[&(tip.id(), Coordinate::X)] - 0.5).abs() < 1e-9);
/// # Ok::<(), laman::model::ModelError>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bisection {
    /// The target the frame was solved at: the full step's, halved
    /// `halvings` times toward the previous frame's. For a frame that
    /// stayed put because every try flipped or jumped, the last target
    /// tried, never solved at: the coordinates are the previous frame's.
    pub reached: [f64; 2],
    /// How many times the step was halved: at least 1, except 0 for a
    /// frame refused with `max_bisections` 0.
    pub halvings: usize,
}

/// What changed branch: an arc or circle, or a `parallel`, `perpendicular`
/// or `angle` constraint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FlipSubject {
    /// An arc (it turned the other way round) or a circle (its radius
    /// changed sign).
    Entity(EntityRef),
    /// A constraint whose pair of lines swapped to the other orientation.
    Constraint(ConstraintRef),
}

/// A change of branch (`docs/DATA-MODEL.md` §Diagnostics, ADR-0006).
///
/// `was` and `now` are the observed quantity where the old branch was last
/// known and on the new one: an arc's sweep in radians, a circle's radius,
/// and for a line pair over its unit directions their `dot` (`parallel`),
/// their `cross` (`perpendicular`) or `cos(φ − θ)` (`angle`).
///
/// ```
/// use laman::model::Sketch;
/// use laman::solution::SolverOptions;
/// let mut sk = Sketch::new();
/// sk.point(1.0, 1.0)?;
/// assert!(sk.solve(&SolverOptions::default()).diagnostics.flips.is_empty());
/// # Ok::<(), laman::model::ModelError>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Flip {
    /// What flipped.
    pub subject: FlipSubject,
    /// The quantity where the old branch was last known.
    pub was: f64,
    /// The quantity on the new branch.
    pub now: f64,
}

/// The sketch a solution was solved from, so [`Sketch::apply`] can refuse
/// one that was not solved for the sketch it lands on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Provenance {
    revision: u64,
    digest: u64,
}

/// A solve toward `goals` and its branches, before it is diagnosed: what a
/// drag step needs to decide whether to keep it.
pub(crate) struct Attempt {
    outcome: Outcome,
    /// The branches at the outcome's coordinates, observed after the
    /// earlier frame's.
    pub(crate) observed: Observations,
    /// The flips from that earlier frame to this one.
    pub(crate) flips: Vec<branch::Flip>,
    /// How it is diagnosed.
    path: Path,
}

/// One solve of `sketch` toward `goals`, its branches observed after
/// `before` (the guesses' for a static solve, the last frame's for a drag
/// step) and compared with them, to be diagnosed by `path`.
pub(crate) fn attempt(
    sketch: &Sketch,
    goals: &[Goal],
    options: &SolverOptions,
    before: &Observations,
    path: Path,
    kept: Option<&Decompositions>,
) -> Attempt {
    let outcome = solve::solve_kept(sketch, goals, options.internal_on(path), kept);
    let observed = before.next(sketch, &outcome.coordinates);
    let flips = before.flips(&observed);
    Attempt {
        outcome,
        observed,
        flips,
        path,
    }
}

/// An attempt diagnosed into the solution that claims `source`, with the
/// observations that are the next `before`.
pub(crate) fn conclude(
    sketch: &Sketch,
    options: &SolverOptions,
    attempt: Attempt,
    source: Provenance,
    kept: Option<&Decompositions>,
) -> (Solution, Observations) {
    let diagnosis = diagnose::analyse_kept(
        sketch,
        &attempt.outcome,
        &options.internal_on(attempt.path),
        kept,
    );
    let solution = Solution {
        coordinates: attempt.outcome.coordinates,
        status: diagnosis.status,
        diagnostics: diagnostics(sketch, &diagnosis, &attempt.flips),
        source,
    };
    (solution, attempt.observed)
}

/// [`attempt`] and [`conclude`] in one.
pub(crate) fn solve_observed(
    sketch: &Sketch,
    goals: &[Goal],
    options: &SolverOptions,
    before: &Observations,
    source: Provenance,
    path: Path,
) -> (Solution, Observations) {
    conclude(
        sketch,
        options,
        attempt(sketch, goals, options, before, path, None),
        source,
        None,
    )
}

/// The diagnosis by `path` of `sketch` at `coordinates`, as a solve that
/// ended there would give it: what `laman::reference` compares a staged
/// solve's diagnostics with at the staged coordinates.
#[cfg(feature = "reference")]
pub(crate) fn diagnose_on(
    sketch: &Sketch,
    coordinates: &Coordinates,
    options: &SolverOptions,
    path: Path,
) -> Solution {
    let before = Observations::at(sketch, &sketch.coordinates());
    let outcome = Outcome::start(coordinates.clone());
    let observed = before.next(sketch, &outcome.coordinates);
    let flips = before.flips(&observed);
    let attempt = Attempt {
        outcome,
        observed,
        flips,
        path,
    };
    conclude(sketch, options, attempt, sketch.provenance(), None).0
}

/// The answer to a solve: coordinates for every parameter, a status and the
/// diagnostics. It always exists; an unsatisfiable or under-constrained
/// sketch gets best-effort coordinates and a status saying so.
///
/// A solution remembers the sketch it was solved from (revision and
/// contents); only [`Sketch::apply`] on that sketch, unedited, accepts it.
/// The same holds for one read back from its serde form.
///
/// ```
/// use laman::model::Sketch;
/// use laman::solution::{SolverOptions, Status};
/// let mut sk = Sketch::new();
/// sk.point(1.0, 1.0)?;
/// let solution = sk.solve(&SolverOptions::default());
/// assert_eq!(solution.status, Status::UnderConstrained);
/// assert_eq!(solution.diagnostics.dof, 2);
/// # Ok::<(), laman::model::ModelError>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Solution {
    /// Every parameter of the sketch, fixed ones at their values.
    pub coordinates: Coordinates,
    /// The verdict.
    pub status: Status,
    /// What the solve established.
    pub diagnostics: Diagnostics,
    pub(crate) source: Provenance,
}

impl Solution {
    /// The revision of the sketch this was solved from.
    ///
    /// ```
    /// use laman::model::Sketch;
    /// use laman::solution::SolverOptions;
    /// let sk = Sketch::new();
    /// assert_eq!(sk.solve(&SolverOptions::default()).source_revision(), sk.revision());
    /// ```
    pub fn source_revision(&self) -> u64 {
        self.source.revision
    }
}

impl Sketch {
    /// Solves the sketch from its current coordinates, which are the guesses
    /// or the last applied solution (the warm start), and never changes it.
    ///
    /// Always returns a [`Solution`]: under-constrained, over-constrained
    /// and unconverged sketches are statuses with best-effort coordinates.
    /// A satisfied component keeps its coordinates bit for bit. The result
    /// depends only on the sketch and `options`, identically on every target.
    ///
    /// ```
    /// use laman::model::Sketch;
    /// use laman::solution::{SolverOptions, Status};
    /// let mut sk = Sketch::new();
    /// let a = sk.point_fixed(0.0, 0.0)?;
    /// let b = sk.point_fixed(1.0, 0.0)?;
    /// sk.distance(a, b, 2.0)?; // contradicts the fixed points
    /// let solution = sk.solve(&SolverOptions::default());
    /// assert_eq!(solution.status, Status::Unsatisfiable);
    /// assert_eq!(solution.diagnostics.conflicts.len(), 1);
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn solve(&self, options: &SolverOptions) -> Solution {
        solve_on(self, options, Path::Staged)
    }

    pub(crate) fn provenance(&self) -> Provenance {
        Provenance {
            revision: self.revision(),
            digest: self.source_digest(),
        }
    }

    /// Writes a solution's coordinates into the sketch, moving its revision.
    ///
    /// Refuses, changing nothing, a solution that was not solved from this
    /// sketch as it is now: `Stale` (another revision), `ForeignSolution`
    /// (this revision, other contents). It refuses one that names a
    /// parameter the sketch lacks (`UnknownId`, `NoSuchParameter`), omits
    /// one it has (`IncompleteSolution`), holds a non-finite value
    /// (`NonFinite`), a negative radius (`DimensionOutOfRange`) or moves a
    /// fixed parameter (`FixedMoved`). Solving after applying warm-starts
    /// from the solution.
    ///
    /// ```
    /// use laman::model::{ModelError, Sketch};
    /// use laman::solution::SolverOptions;
    /// let mut sk = Sketch::new();
    /// let a = sk.point_fixed(0.0, 0.0)?;
    /// let b = sk.point(3.0, 1.0)?;
    /// sk.distance(a, b, 5.0)?;
    /// let solution = sk.solve(&SolverOptions::default());
    /// sk.apply(&solution)?;
    /// // The sketch moved on: the same solution is now stale.
    /// assert!(matches!(sk.apply(&solution), Err(ModelError::Stale { .. })));
    /// # Ok::<(), ModelError>(())
    /// ```
    pub fn apply(&mut self, solution: &Solution) -> Result<(), ModelError> {
        if solution.source.revision != self.revision() {
            return Err(ModelError::Stale {
                solved: solution.source.revision,
                current: self.revision(),
            });
        }
        if solution.source.digest != self.source_digest() {
            return Err(ModelError::ForeignSolution);
        }
        let own = self.coordinates();
        for key in solution.coordinates.keys() {
            if !own.contains_key(key) {
                return Err(match self.entity(key.0) {
                    None => ModelError::UnknownId { id: key.0 },
                    Some(_) => ModelError::NoSuchParameter {
                        id: key.0,
                        coordinate: key.1,
                    },
                });
            }
        }
        for (&(id, coordinate), &current) in &own {
            let Some(&given) = solution.coordinates.get(&(id, coordinate)) else {
                return Err(ModelError::IncompleteSolution { id, coordinate });
            };
            if !given.is_finite() {
                return Err(ModelError::NonFinite {
                    what: "solution coordinate",
                    value: given,
                });
            }
            if self.is_fixed(id, coordinate) && given.to_bits() != current.to_bits() {
                return Err(ModelError::FixedMoved {
                    id,
                    coordinate,
                    fixed: current,
                    given,
                });
            }
            if coordinate == Coordinate::R && given < 0.0 {
                return Err(ModelError::DimensionOutOfRange {
                    what: "radius",
                    value: given,
                    wanted: "r >= 0",
                });
            }
        }
        self.replace_coordinates(&solution.coordinates);
        Ok(())
    }
}

fn entity_ref(sketch: &Sketch, id: EntityId) -> Option<EntityRef> {
    Some(EntityRef {
        id,
        kind: sketch.entity(id)?.kind(),
        tag: sketch.tag_of(id).map(str::to_owned),
    })
}

fn constraint_ref(sketch: &Sketch, id: ConstraintId) -> Option<ConstraintRef> {
    Some(ConstraintRef {
        id,
        kind: sketch.constraint(id)?.kind(),
        tag: sketch.constraint_tag_of(id).map(str::to_owned),
    })
}

fn origin(sketch: &Sketch, source: Source) -> Option<Origin> {
    match source {
        Source::Constraint(id) => constraint_ref(sketch, id).map(Origin::Constraint),
        Source::Arc(id) => entity_ref(sketch, id).map(Origin::Arc),
    }
}

fn constraint_refs(sketch: &Sketch, ids: &[ConstraintId]) -> Vec<ConstraintRef> {
    ids.iter()
        .filter_map(|id| constraint_ref(sketch, *id))
        .collect()
}

pub(crate) fn flips_of(sketch: &Sketch, flips: &[branch::Flip]) -> Vec<Flip> {
    flips
        .iter()
        .filter_map(|f| {
            let subject = match f.subject {
                branch::Subject::Entity(id) => FlipSubject::Entity(entity_ref(sketch, id)?),
                branch::Subject::Constraint(id) => {
                    FlipSubject::Constraint(constraint_ref(sketch, id)?)
                }
            };
            Some(Flip {
                subject,
                was: f.was,
                now: f.now,
            })
        })
        .collect()
}

fn diagnostics(sketch: &Sketch, d: &Diagnosis, flips: &[branch::Flip]) -> Diagnostics {
    Diagnostics {
        dof: d.freedom.dof,
        entities: d
            .freedom
            .entities
            .iter()
            .filter_map(|(id, dof)| {
                Some(EntityFreedom {
                    entity: entity_ref(sketch, *id)?,
                    dof: *dof,
                })
            })
            .collect(),
        free_directions: d
            .freedom
            .directions
            .iter()
            .map(|v| {
                v.iter()
                    .map(|&(entity, coordinate, amount)| Motion {
                        entity,
                        coordinate,
                        amount,
                    })
                    .collect()
            })
            .collect(),
        redundant: constraint_refs(sketch, &d.redundant),
        conflicts: d
            .conflicts
            .iter()
            .map(|set| constraint_refs(sketch, set))
            .collect(),
        degenerate: d
            .degenerate
            .iter()
            .filter_map(|g| {
                Some(Degenerate {
                    entity: entity_ref(sketch, g.entity)?,
                    collapse: g.collapse,
                    size: g.size,
                    equations: g
                        .equations
                        .iter()
                        .filter_map(|s| origin(sketch, *s))
                        .collect(),
                })
            })
            .collect(),
        ill_conditioned: d
            .ill_conditioned
            .iter()
            .filter_map(|s| origin(sketch, *s))
            .collect(),
        flips: flips_of(sketch, flips),
        bisection: None,
    }
}
