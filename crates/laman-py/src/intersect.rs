//! Intersections, `laman.intersect`: the value classes over
//! `laman::model::intersection`.
//!
//! Each class holds the Rust value, so a check and a digest read the same
//! bits `intersections` wrote. A float crosses unchanged. An id is a `u32`: a
//! negative or oversized one raises `OverflowError` on construction.

use crate::errors::intersect_error;
use crate::model::{Coordinates, Sketch};
use laman::model::EntityId;
use laman::model::intersection::{Along, Intersection, Intersections, Meeting, MeetingKind};
use pyo3::prelude::*;

fn pair(v: [f64; 2]) -> String {
    format!("({:?}, {:?})", v[0], v[1])
}

/// How two curves meet: a crossing, a touch or an overlap.
#[pyclass(frozen, eq, name = "MeetingKind", module = "laman.intersect")]
#[derive(Clone, PartialEq)]
pub struct PyMeetingKind {
    pub inner: MeetingKind,
}

#[pymethods]
impl PyMeetingKind {
    /// A simple root: the curves' directions differ there.
    #[staticmethod]
    fn crossing() -> Self {
        Self {
            inner: MeetingKind::Crossing,
        }
    }

    /// Tangent within the length tolerance: one point.
    #[staticmethod]
    fn touch() -> Self {
        Self {
            inner: MeetingKind::Touch,
        }
    }

    /// A shared stretch, from the meeting's point to `to`, with its
    /// parameters on the first and second curve.
    #[staticmethod]
    #[pyo3(signature = (to, to_first, to_second))]
    fn overlap(to: (f64, f64), to_first: f64, to_second: f64) -> Self {
        Self {
            inner: MeetingKind::Overlap {
                to: [to.0, to.1],
                to_first,
                to_second,
            },
        }
    }

    /// `"crossing"`, `"touch"` or `"overlap"`.
    #[getter]
    fn name(&self) -> &'static str {
        match self.inner {
            MeetingKind::Crossing => "crossing",
            MeetingKind::Touch => "touch",
            MeetingKind::Overlap { .. } => "overlap",
        }
    }

    /// An overlap's other end, else `None`.
    #[getter]
    fn to(&self) -> Option<(f64, f64)> {
        match self.inner {
            MeetingKind::Overlap { to, .. } => Some((to[0], to[1])),
            MeetingKind::Crossing | MeetingKind::Touch => None,
        }
    }

    /// An overlap's other end's parameter on the first curve, else `None`.
    #[getter]
    fn to_first(&self) -> Option<f64> {
        match self.inner {
            MeetingKind::Overlap { to_first, .. } => Some(to_first),
            MeetingKind::Crossing | MeetingKind::Touch => None,
        }
    }

    /// An overlap's other end's parameter on the second curve, else `None`.
    #[getter]
    fn to_second(&self) -> Option<f64> {
        match self.inner {
            MeetingKind::Overlap { to_second, .. } => Some(to_second),
            MeetingKind::Crossing | MeetingKind::Touch => None,
        }
    }

    fn __repr__(&self) -> String {
        match self.inner {
            MeetingKind::Crossing => "MeetingKind.crossing()".to_owned(),
            MeetingKind::Touch => "MeetingKind.touch()".to_owned(),
            MeetingKind::Overlap {
                to,
                to_first,
                to_second,
            } => format!(
                "MeetingKind.overlap(to={}, to_first={to_first:?}, to_second={to_second:?})",
                pair(to)
            ),
        }
    }
}

/// One place where two curves meet, with its parameter on each.
#[pyclass(frozen, eq, name = "Meeting", module = "laman.intersect")]
#[derive(Clone, PartialEq)]
pub struct PyMeeting {
    pub inner: Meeting,
}

#[pymethods]
impl PyMeeting {
    #[new]
    #[pyo3(signature = (kind, at, on_first, on_second))]
    fn new(kind: PyMeetingKind, at: (f64, f64), on_first: f64, on_second: f64) -> Self {
        Self {
            inner: Meeting {
                kind: kind.inner,
                at: [at.0, at.1],
                on_first,
                on_second,
            },
        }
    }

    /// What kind of meeting.
    #[getter]
    fn kind(&self) -> PyMeetingKind {
        PyMeetingKind {
            inner: self.inner.kind,
        }
    }

    /// Where, in the caller's coordinates.
    #[getter]
    fn at(&self) -> (f64, f64) {
        (self.inner.at[0], self.inner.at[1])
    }

    /// Its parameter on the first curve.
    #[getter]
    fn on_first(&self) -> f64 {
        self.inner.on_first
    }

    /// Its parameter on the second curve.
    #[getter]
    fn on_second(&self) -> f64 {
        self.inner.on_second
    }

    fn __repr__(&self) -> String {
        format!(
            "Meeting(kind={}, at={}, on_first={:?}, on_second={:?})",
            self.kind().__repr__(),
            pair(self.inner.at),
            self.inner.on_first,
            self.inner.on_second
        )
    }
}

/// The meetings of one pair of curves, smaller id first.
#[pyclass(frozen, eq, name = "Intersection", module = "laman.intersect")]
#[derive(Clone, PartialEq)]
pub struct PyIntersection {
    pub inner: Intersection,
}

#[pymethods]
impl PyIntersection {
    #[new]
    #[pyo3(signature = (first, second, meetings))]
    fn new(first: u32, second: u32, meetings: Vec<PyMeeting>) -> Self {
        Self {
            inner: Intersection {
                first: EntityId(first),
                second: EntityId(second),
                meetings: meetings.into_iter().map(|m| m.inner).collect(),
            },
        }
    }

    /// The smaller id.
    #[getter]
    fn first(&self) -> u32 {
        self.inner.first.0
    }

    /// The larger id.
    #[getter]
    fn second(&self) -> u32 {
        self.inner.second.0
    }

    /// Where they meet, in order of their parameter on the first.
    #[getter]
    fn meetings(&self) -> Vec<PyMeeting> {
        self.inner
            .meetings
            .iter()
            .map(|&inner| PyMeeting { inner })
            .collect()
    }

    fn __repr__(&self) -> String {
        let meetings: Vec<String> = self.meetings().iter().map(PyMeeting::__repr__).collect();
        format!(
            "Intersection(first={}, second={}, meetings=[{}])",
            self.inner.first.0,
            self.inner.second.0,
            meetings.join(", ")
        )
    }
}

/// A meeting seen from one curve of its pair.
#[pyclass(frozen, eq, name = "Along", module = "laman.intersect")]
#[derive(Clone, PartialEq)]
pub struct PyAlong {
    pub inner: Along,
}

#[pymethods]
impl PyAlong {
    #[new]
    #[pyo3(signature = (other, meeting))]
    fn new(other: u32, meeting: PyMeeting) -> Self {
        Self {
            inner: Along {
                other: EntityId(other),
                meeting: meeting.inner,
            },
        }
    }

    /// The curve it meets.
    #[getter]
    fn other(&self) -> u32 {
        self.inner.other.0
    }

    /// The meeting with the asked curve as its first: `on_first` is the
    /// position along it.
    #[getter]
    fn meeting(&self) -> PyMeeting {
        PyMeeting {
            inner: self.inner.meeting,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Along(other={}, meeting={})",
            self.inner.other.0,
            self.meeting().__repr__()
        )
    }
}

/// What intersection measuring finds in a sketch at some coordinates.
#[pyclass(frozen, eq, name = "Intersections", module = "laman.intersect")]
#[derive(Clone, PartialEq)]
pub struct PyIntersections {
    pub inner: Intersections,
}

#[pymethods]
impl PyIntersections {
    #[new]
    #[pyo3(signature = (pairs=Vec::new()))]
    fn new(pairs: Vec<PyIntersection>) -> Self {
        Self {
            inner: Intersections {
                pairs: pairs.into_iter().map(|p| p.inner).collect(),
            },
        }
    }

    /// The pairs that meet, ordered by their first id, then their second's.
    #[getter]
    fn pairs(&self) -> Vec<PyIntersection> {
        self.inner
            .pairs
            .iter()
            .map(|p| PyIntersection { inner: p.clone() })
            .collect()
    }

    /// The meetings on the curve `id`, in order of their parameter along it,
    /// each with `id` as the first curve.
    fn along(&self, id: u32) -> Vec<PyAlong> {
        self.inner
            .along(EntityId(id))
            .into_iter()
            .map(|inner| PyAlong { inner })
            .collect()
    }

    fn __repr__(&self) -> String {
        let pairs: Vec<String> = self.pairs().iter().map(PyIntersection::__repr__).collect();
        format!("Intersections(pairs=[{}])", pairs.join(", "))
    }
}

/// Where the curves of `sketch` meet at `coordinates`
/// (`laman::intersect::intersections`).
#[pyfunction]
pub fn intersections(
    py: Python<'_>,
    sketch: &Sketch,
    coordinates: &Coordinates,
) -> PyResult<PyIntersections> {
    laman::intersect::intersections(&sketch.inner, &coordinates.inner)
        .map(|inner| PyIntersections { inner })
        .map_err(|e| intersect_error(py, &e))
}
