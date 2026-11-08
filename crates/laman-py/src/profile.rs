//! Profiles, `laman.profile`: the value classes over `laman::model::profile`.
//!
//! Each class holds the Rust value, so a check and a digest read the same
//! bits extraction wrote. A float crosses unchanged. An id is a `u32`: a
//! negative or oversized one raises `OverflowError` on construction.

use laman::model::profile::{Branched, Loop, Profile, Profiles, Traversal, Unjoined};
use laman::model::{ConstraintId, EntityId};
use pyo3::prelude::*;

fn ids(v: &[EntityId]) -> Vec<u32> {
    v.iter().map(|e| e.0).collect()
}

fn entity_ids(v: Vec<u32>) -> Vec<EntityId> {
    v.into_iter().map(EntityId).collect()
}

fn constraint_ids(v: &[ConstraintId]) -> Vec<u32> {
    v.iter().map(|c| c.0).collect()
}

fn list<T: std::fmt::Display>(v: &[T]) -> String {
    let parts: Vec<String> = v.iter().map(ToString::to_string).collect();
    format!("[{}]", parts.join(", "))
}

fn py_bool(b: bool) -> &'static str {
    if b { "True" } else { "False" }
}

/// An edge as a loop traverses it.
#[pyclass(frozen, eq, get_all, name = "Traversal", module = "laman.profile")]
#[derive(Clone, PartialEq)]
pub struct PyTraversal {
    /// The line or arc.
    pub entity: u32,
    /// Traversed from its end to its start rather than as drawn.
    pub reversed: bool,
}

#[pymethods]
impl PyTraversal {
    #[new]
    #[pyo3(signature = (entity, reversed))]
    fn new(entity: u32, reversed: bool) -> Self {
        Self { entity, reversed }
    }

    fn __repr__(&self) -> String {
        format!(
            "Traversal(entity={}, reversed={})",
            self.entity,
            py_bool(self.reversed)
        )
    }
}

impl PyTraversal {
    fn from_rust(t: &Traversal) -> Self {
        Self {
            entity: t.entity.0,
            reversed: t.reversed,
        }
    }

    fn to_rust(&self) -> Traversal {
        Traversal {
            entity: EntityId(self.entity),
            reversed: self.reversed,
        }
    }
}

/// A closed boundary: edges end to end, or one circle.
#[pyclass(frozen, eq, name = "Loop", module = "laman.profile")]
#[derive(Clone, PartialEq)]
pub struct PyLoop {
    pub inner: Loop,
}

#[pymethods]
impl PyLoop {
    #[new]
    #[pyo3(signature = (edges, signed_area, joins=Vec::new()))]
    fn new(edges: Vec<PyTraversal>, signed_area: f64, joins: Vec<u32>) -> Self {
        Self {
            inner: Loop {
                edges: edges.iter().map(PyTraversal::to_rust).collect(),
                signed_area,
                joins: joins.into_iter().map(ConstraintId).collect(),
            },
        }
    }

    /// The directed edges in loop order.
    #[getter]
    fn edges(&self) -> Vec<PyTraversal> {
        self.inner
            .edges
            .iter()
            .map(PyTraversal::from_rust)
            .collect()
    }

    /// The area enclosed, positive counter-clockwise; saturates at `±f64::MAX`.
    #[getter]
    fn signed_area(&self) -> f64 {
        self.inner.signed_area
    }

    /// The `coincident` constraints that join this loop's ends, by id.
    #[getter]
    fn joins(&self) -> Vec<u32> {
        constraint_ids(&self.inner.joins)
    }

    fn __repr__(&self) -> String {
        let edges: Vec<String> = self.edges().iter().map(PyTraversal::__repr__).collect();
        format!(
            "Loop(edges=[{}], signed_area={:?}, joins={})",
            edges.join(", "),
            self.inner.signed_area,
            list(&constraint_ids(&self.inner.joins))
        )
    }
}

/// An outer loop, counter-clockwise, with the loops it holds as holes.
#[pyclass(frozen, eq, name = "Profile", module = "laman.profile")]
#[derive(Clone, PartialEq)]
pub struct PyProfile {
    pub inner: Profile,
}

#[pymethods]
impl PyProfile {
    #[new]
    #[pyo3(signature = (outer, holes, net_area))]
    fn new(outer: PyLoop, holes: Vec<PyLoop>, net_area: f64) -> Self {
        Self {
            inner: Profile {
                outer: outer.inner,
                holes: holes.into_iter().map(|h| h.inner).collect(),
                net_area,
            },
        }
    }

    /// The outer boundary.
    #[getter]
    fn outer(&self) -> PyLoop {
        PyLoop {
            inner: self.inner.outer.clone(),
        }
    }

    /// The holes directly inside it.
    #[getter]
    fn holes(&self) -> Vec<PyLoop> {
        self.inner
            .holes
            .iter()
            .map(|h| PyLoop { inner: h.clone() })
            .collect()
    }

    /// The outer area less the holes' areas.
    #[getter]
    fn net_area(&self) -> f64 {
        self.inner.net_area
    }

    fn __repr__(&self) -> String {
        let holes: Vec<String> = self.holes().iter().map(PyLoop::__repr__).collect();
        format!(
            "Profile(outer={}, holes=[{}], net_area={:?})",
            self.outer().__repr__(),
            holes.join(", "),
            self.inner.net_area
        )
    }
}

/// A core of edges where some join has three or more of them.
#[pyclass(frozen, eq, get_all, name = "Branched", module = "laman.profile")]
#[derive(Clone, PartialEq)]
pub struct PyBranched {
    /// The core's edges, in id order.
    pub edges: Vec<u32>,
    /// The joins where three or more edges meet, by smallest point id.
    pub joins: Vec<u32>,
}

#[pymethods]
impl PyBranched {
    #[new]
    #[pyo3(signature = (edges, joins))]
    fn new(edges: Vec<u32>, joins: Vec<u32>) -> Self {
        Self { edges, joins }
    }

    fn __repr__(&self) -> String {
        format!(
            "Branched(edges={}, joins={})",
            list(&self.edges),
            list(&self.joins)
        )
    }
}

/// Two free ends that coincide in the coordinates but are not joined.
#[pyclass(frozen, eq, get_all, name = "Unjoined", module = "laman.profile")]
#[derive(Clone, PartialEq)]
pub struct PyUnjoined {
    /// The smaller id of the two end points.
    pub first: u32,
    /// The larger id of the two end points.
    pub second: u32,
}

#[pymethods]
impl PyUnjoined {
    #[new]
    #[pyo3(signature = (first, second))]
    fn new(first: u32, second: u32) -> Self {
        Self { first, second }
    }

    fn __repr__(&self) -> String {
        format!("Unjoined(first={}, second={})", self.first, self.second)
    }
}

/// What profile extraction finds in a sketch at some coordinates.
#[pyclass(frozen, eq, name = "Profiles", module = "laman.profile")]
#[derive(Clone, PartialEq)]
pub struct PyProfiles {
    pub inner: Profiles,
}

#[pymethods]
impl PyProfiles {
    #[new]
    #[pyo3(signature = (
        profiles=Vec::new(),
        open_chains=Vec::new(),
        dangling=Vec::new(),
        branched=Vec::new(),
        degenerate=Vec::new(),
        unjoined=Vec::new(),
    ))]
    fn new(
        profiles: Vec<PyProfile>,
        open_chains: Vec<Vec<u32>>,
        dangling: Vec<u32>,
        branched: Vec<PyBranched>,
        degenerate: Vec<u32>,
        unjoined: Vec<PyUnjoined>,
    ) -> Self {
        Self {
            inner: Profiles {
                profiles: profiles.into_iter().map(|p| p.inner).collect(),
                open_chains: open_chains.into_iter().map(entity_ids).collect(),
                dangling: entity_ids(dangling),
                branched: branched
                    .into_iter()
                    .map(|b| Branched {
                        edges: entity_ids(b.edges),
                        joins: entity_ids(b.joins),
                    })
                    .collect(),
                degenerate: entity_ids(degenerate),
                unjoined: unjoined
                    .into_iter()
                    .map(|u| Unjoined {
                        first: EntityId(u.first),
                        second: EntityId(u.second),
                    })
                    .collect(),
            },
        }
    }

    /// The profiles, ordered by their outer loop's smallest edge id.
    #[getter]
    fn profiles(&self) -> Vec<PyProfile> {
        self.inner
            .profiles
            .iter()
            .map(|p| PyProfile { inner: p.clone() })
            .collect()
    }

    /// Chains of edges that join end to end without closing.
    #[getter]
    fn open_chains(&self) -> Vec<Vec<u32>> {
        self.inner.open_chains.iter().map(|c| ids(c)).collect()
    }

    /// Edges hanging from a component that still holds a loop.
    #[getter]
    fn dangling(&self) -> Vec<u32> {
        ids(&self.inner.dangling)
    }

    /// Cores no profile takes.
    #[getter]
    fn branched(&self) -> Vec<PyBranched> {
        self.inner
            .branched
            .iter()
            .map(|b| PyBranched {
                edges: ids(&b.edges),
                joins: ids(&b.joins),
            })
            .collect()
    }

    /// Self-joined edges, left out.
    #[getter]
    fn degenerate(&self) -> Vec<u32> {
        ids(&self.inner.degenerate)
    }

    /// Free ends that coincide without a join.
    #[getter]
    fn unjoined(&self) -> Vec<PyUnjoined> {
        self.inner
            .unjoined
            .iter()
            .map(|u| PyUnjoined {
                first: u.first.0,
                second: u.second.0,
            })
            .collect()
    }

    fn __repr__(&self) -> String {
        let profiles: Vec<String> = self.profiles().iter().map(PyProfile::__repr__).collect();
        let chains: Vec<String> = self
            .inner
            .open_chains
            .iter()
            .map(|c| list(&ids(c)))
            .collect();
        let branched: Vec<String> = self.branched().iter().map(PyBranched::__repr__).collect();
        let unjoined: Vec<String> = self.unjoined().iter().map(PyUnjoined::__repr__).collect();
        format!(
            "Profiles(profiles=[{}], open_chains=[{}], dangling={}, branched=[{}], degenerate={}, unjoined=[{}])",
            profiles.join(", "),
            chains.join(", "),
            list(&ids(&self.inner.dangling)),
            branched.join(", "),
            list(&ids(&self.inner.degenerate)),
            unjoined.join(", ")
        )
    }
}
