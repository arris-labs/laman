//! Solver options and the solution of a solve.

use crate::enums::{algorithm_from_py, algorithm_to_py, status_to_py};
use crate::model::{Coordinates, coordinate_name_of};
use pyo3::prelude::*;

/// How a solve runs: the algorithm and its hard caps. Defaults are Rust's.
#[pyclass(frozen, eq, module = "laman")]
#[derive(Clone, PartialEq)]
pub struct SolverOptions {
    pub inner: laman::solution::SolverOptions,
}

#[pymethods]
impl SolverOptions {
    #[new]
    #[pyo3(signature = (*, algorithm=None, max_iterations=None, max_evaluations=None, max_analysis_solves=None, max_bisections=None))]
    fn new(
        algorithm: Option<&Bound<'_, PyAny>>,
        max_iterations: Option<usize>,
        max_evaluations: Option<usize>,
        max_analysis_solves: Option<usize>,
        max_bisections: Option<usize>,
    ) -> PyResult<Self> {
        let mut inner = laman::solution::SolverOptions::default();
        if let Some(algorithm) = algorithm {
            inner.algorithm = algorithm_from_py(algorithm)?;
        }
        if let Some(n) = max_iterations {
            inner.max_iterations = n;
        }
        if let Some(n) = max_evaluations {
            inner.max_evaluations = n;
        }
        if let Some(n) = max_analysis_solves {
            inner.max_analysis_solves = n;
        }
        if let Some(n) = max_bisections {
            inner.max_bisections = n;
        }
        Ok(Self { inner })
    }

    /// The step each iteration proposes.
    #[getter]
    fn algorithm(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        algorithm_to_py(py, self.inner.algorithm)
    }

    /// Accepted and rejected steps in all, across components.
    #[getter]
    fn max_iterations(&self) -> usize {
        self.inner.max_iterations
    }

    /// Residual and Jacobian evaluations in all, across components.
    #[getter]
    fn max_evaluations(&self) -> usize {
        self.inner.max_evaluations
    }

    /// Re-solves the conflict analysis may spend in all.
    #[getter]
    fn max_analysis_solves(&self) -> usize {
        self.inner.max_analysis_solves
    }

    /// Halvings of a drag step that flips a branch.
    #[getter]
    fn max_bisections(&self) -> usize {
        self.inner.max_bisections
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let algorithm = algorithm_to_py(py, self.inner.algorithm)?;
        Ok(format!(
            "SolverOptions(algorithm={}, max_iterations={}, max_evaluations={}, max_analysis_solves={}, max_bisections={})",
            algorithm.bind(py).str()?,
            self.inner.max_iterations,
            self.inner.max_evaluations,
            self.inner.max_analysis_solves,
            self.inner.max_bisections
        ))
    }
}

fn tag_repr(tag: &Option<String>) -> String {
    tag.as_ref()
        .map_or_else(|| "None".to_owned(), |t| format!("{t:?}"))
}

/// An entity as a diagnostic names it: the caller's id, its kind and its tag.
#[pyclass(frozen, eq, hash, get_all, module = "laman")]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct EntityRef {
    /// The entity's id.
    id: u32,
    /// Its kind: `"point"`, `"line"`, `"circle"` or `"arc"`.
    kind: String,
    /// Its tag, when it has one.
    tag: Option<String>,
}

impl EntityRef {
    /// An entity of a sketch by id, kind and tag, for `Sketch.entities`.
    pub fn named(id: u32, kind: String, tag: Option<String>) -> Self {
        Self { id, kind, tag }
    }

    fn of(r: &laman::solution::EntityRef) -> Self {
        Self {
            id: r.id.0,
            kind: r.kind.to_string(),
            tag: r.tag.clone(),
        }
    }
}

#[pymethods]
impl EntityRef {
    fn __repr__(&self) -> String {
        format!(
            "EntityRef(id={}, kind={:?}, tag={})",
            self.id,
            self.kind,
            tag_repr(&self.tag)
        )
    }
}

/// A constraint as a diagnostic names it: the caller's id, its kind and its
/// tag.
#[pyclass(frozen, eq, hash, get_all, module = "laman")]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ConstraintRef {
    /// The constraint's id.
    id: u32,
    /// Its kind name, as the serde form spells it (`"distance"`).
    kind: String,
    /// Its tag, when it has one.
    tag: Option<String>,
}

impl ConstraintRef {
    /// A constraint of a sketch by id, kind and tag, for `Sketch.constraints`.
    pub fn named(id: u32, kind: String, tag: Option<String>) -> Self {
        Self { id, kind, tag }
    }

    fn of(r: &laman::solution::ConstraintRef) -> Self {
        Self {
            id: r.id.0,
            kind: r.kind.to_string(),
            tag: r.tag.clone(),
        }
    }
}

#[pymethods]
impl ConstraintRef {
    fn __repr__(&self) -> String {
        format!(
            "ConstraintRef(id={}, kind={:?}, tag={})",
            self.id,
            self.kind,
            tag_repr(&self.tag)
        )
    }
}

/// An entity's remaining freedom.
#[pyclass(frozen, eq, hash, get_all, module = "laman")]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct EntityFreedom {
    /// The entity.
    entity: EntityRef,
    /// Its degrees of freedom: 0 when it cannot move relative to the ground.
    dof: usize,
}

#[pymethods]
impl EntityFreedom {
    fn __repr__(&self) -> String {
        format!(
            "EntityFreedom(entity={}, dof={})",
            self.entity.__repr__(),
            self.dof
        )
    }
}

/// One parameter's share of a free direction.
#[pyclass(frozen, eq, get_all, module = "laman")]
#[derive(Clone, PartialEq)]
pub struct Motion {
    /// The id of the entity that moves.
    entity: u32,
    /// The parameter of it: `"x"`, `"y"` or `"r"`.
    coordinate: String,
    /// Its component of the unit direction; never within rounding of zero.
    amount: f64,
}

#[pymethods]
impl Motion {
    fn __repr__(&self) -> String {
        format!(
            "Motion(entity={}, coordinate={:?}, amount={})",
            self.entity, self.coordinate, self.amount
        )
    }
}

/// Where an equation comes from, as Python holds it: a `ConstraintRef`, or
/// the `EntityRef` of an arc whose own equation it is.
fn origin(py: Python<'_>, o: &laman::solution::Origin) -> PyResult<Py<PyAny>> {
    match o {
        laman::solution::Origin::Constraint(c) => Ok(Py::new(py, ConstraintRef::of(c))?.into_any()),
        laman::solution::Origin::Arc(e) => Ok(Py::new(py, EntityRef::of(e))?.into_any()),
    }
}

/// An entity that collapsed at the solution, and what that makes
/// ill-conditioned.
#[pyclass(frozen, get_all, module = "laman")]
pub struct Degenerate {
    /// The collapsed entity.
    entity: EntityRef,
    /// How it collapsed: `"zero_length_line"`, `"zero_radius"`,
    /// `"zero_radius_arc"` or `"empty_arc"`.
    collapse: String,
    /// Its size at the solution, in the sketch's units.
    size: f64,
    /// The equations that read it: each a `ConstraintRef`, or the
    /// `EntityRef` of an arc whose own equation it is.
    equations: Vec<Py<PyAny>>,
}

#[pymethods]
impl Degenerate {
    fn __repr__(&self) -> String {
        format!(
            "Degenerate(entity={}, collapse={:?}, size={})",
            self.entity.__repr__(),
            self.collapse,
            self.size
        )
    }
}

/// What changed branch: an entity (`.entity`, an arc or circle) or a
/// constraint (`.constraint`); the other is `None`.
#[pyclass(frozen, eq, get_all, module = "laman")]
#[derive(Clone, PartialEq, Eq)]
pub struct FlipSubject {
    /// The arc or circle that flipped, else `None`.
    entity: Option<EntityRef>,
    /// The `parallel`, `perpendicular` or `angle` constraint that flipped,
    /// else `None`.
    constraint: Option<ConstraintRef>,
}

#[pymethods]
impl FlipSubject {
    fn __repr__(&self) -> String {
        match (&self.entity, &self.constraint) {
            (Some(e), _) => format!("FlipSubject(entity={})", e.__repr__()),
            (None, Some(c)) => format!("FlipSubject(constraint={})", c.__repr__()),
            (None, None) => "FlipSubject()".to_owned(),
        }
    }
}

/// A change of branch.
#[pyclass(frozen, eq, get_all, module = "laman")]
#[derive(Clone, PartialEq)]
pub struct Flip {
    /// What flipped.
    subject: FlipSubject,
    /// The quantity where the old branch was last known.
    was: f64,
    /// The quantity on the new branch.
    now: f64,
}

#[pymethods]
impl Flip {
    fn __repr__(&self) -> String {
        format!(
            "Flip(subject={}, was={}, now={})",
            self.subject.__repr__(),
            self.was,
            self.now
        )
    }
}

/// What a solve established about the sketch.
#[pyclass(frozen, get_all, module = "laman")]
pub struct Diagnostics {
    /// The geometric degrees of freedom left.
    dof: usize,
    /// Every entity's freedom, in id order.
    entities: Vec<EntityFreedom>,
    /// An orthonormal basis of the free motions.
    free_directions: Vec<Vec<Motion>>,
    /// Satisfied constraints whose removal leaves the DoF unchanged.
    redundant: Vec<ConstraintRef>,
    /// Inclusion-minimal sets of constraints jointly unsatisfiable.
    conflicts: Vec<Vec<ConstraintRef>>,
    /// Entities collapsed below the length tolerance.
    degenerate: Vec<Py<Degenerate>>,
    /// Equations undefined or non-finite at the solution: each a
    /// `ConstraintRef`, or the `EntityRef` of an arc.
    ill_conditioned: Vec<Py<PyAny>>,
    /// Branches the solution left that no equation holds.
    flips: Vec<Flip>,
}

impl Diagnostics {
    fn of(py: Python<'_>, d: &laman::solution::Diagnostics) -> PyResult<Self> {
        let refs = |v: &[laman::solution::ConstraintRef]| v.iter().map(ConstraintRef::of).collect();
        Ok(Self {
            dof: d.dof,
            entities: d
                .entities
                .iter()
                .map(|f| EntityFreedom {
                    entity: EntityRef::of(&f.entity),
                    dof: f.dof,
                })
                .collect(),
            free_directions: d
                .free_directions
                .iter()
                .map(|v| {
                    v.iter()
                        .map(|m| Motion {
                            entity: m.entity.0,
                            coordinate: coordinate_name_of(m.coordinate).to_owned(),
                            amount: m.amount,
                        })
                        .collect()
                })
                .collect(),
            redundant: refs(&d.redundant),
            conflicts: d.conflicts.iter().map(|set| refs(set)).collect(),
            degenerate: d
                .degenerate
                .iter()
                .map(|g| {
                    Py::new(
                        py,
                        Degenerate {
                            entity: EntityRef::of(&g.entity),
                            collapse: collapse_name(g.collapse).to_owned(),
                            size: g.size,
                            equations: g
                                .equations
                                .iter()
                                .map(|o| origin(py, o))
                                .collect::<PyResult<_>>()?,
                        },
                    )
                })
                .collect::<PyResult<_>>()?,
            ill_conditioned: d
                .ill_conditioned
                .iter()
                .map(|o| origin(py, o))
                .collect::<PyResult<_>>()?,
            flips: d.flips.iter().map(flip).collect(),
        })
    }
}

fn flip(f: &laman::solution::Flip) -> Flip {
    let subject = match &f.subject {
        laman::solution::FlipSubject::Entity(e) => FlipSubject {
            entity: Some(EntityRef::of(e)),
            constraint: None,
        },
        laman::solution::FlipSubject::Constraint(c) => FlipSubject {
            entity: None,
            constraint: Some(ConstraintRef::of(c)),
        },
    };
    Flip {
        subject,
        was: f.was,
        now: f.now,
    }
}

// `Collapse` is exhaustive today; the names are the checker's spelling.
fn collapse_name(c: laman::solution::Collapse) -> &'static str {
    match c {
        laman::solution::Collapse::ZeroLengthLine => "zero_length_line",
        laman::solution::Collapse::ZeroRadius => "zero_radius",
        laman::solution::Collapse::ZeroRadiusArc => "zero_radius_arc",
        laman::solution::Collapse::EmptyArc => "empty_arc",
    }
}

#[pymethods]
impl Diagnostics {
    fn __repr__(&self) -> String {
        format!(
            "Diagnostics(dof={}, redundant={}, conflicts={}, flips={})",
            self.dof,
            self.redundant.len(),
            self.conflicts.len(),
            self.flips.len()
        )
    }
}

/// The result of a solve: coordinates, a status and the diagnostics.
#[pyclass(frozen, module = "laman")]
pub struct Solution {
    pub inner: laman::solution::Solution,
    diagnostics: Py<Diagnostics>,
}

impl Solution {
    pub fn new(py: Python<'_>, inner: laman::solution::Solution) -> PyResult<Self> {
        let diagnostics = Py::new(py, Diagnostics::of(py, &inner.diagnostics)?)?;
        Ok(Self { inner, diagnostics })
    }
}

#[pymethods]
impl Solution {
    /// Every parameter of the sketch, fixed ones at their values.
    #[getter]
    fn coordinates(&self) -> Coordinates {
        Coordinates {
            inner: self.inner.coordinates.clone(),
        }
    }

    /// The verdict.
    #[getter]
    fn status(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        status_to_py(py, self.inner.status)
    }

    /// What the solve established.
    #[getter]
    fn diagnostics(&self, py: Python<'_>) -> Py<Diagnostics> {
        self.diagnostics.clone_ref(py)
    }

    /// The revision of the sketch this was solved from.
    #[getter]
    fn source_revision(&self) -> u64 {
        self.inner.source_revision()
    }

    fn __repr__(&self) -> String {
        format!(
            "Solution(status={}, dof={})",
            self.inner.status, self.inner.diagnostics.dof
        )
    }
}
