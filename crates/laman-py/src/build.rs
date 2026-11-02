//! The builder side of the sketch: entity handles and what a builder
//! returns. Nothing here computes; every method is a call on the Rust
//! sketch.

use laman::model::{ConstraintId, EntityId, ModelError, Sense, Sketch};
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;

use crate::errors::model_error;

macro_rules! handle {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[pyclass(frozen, eq, hash, module = "laman")]
        #[derive(Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name {
            /// The entity's id in its sketch.
            #[pyo3(get)]
            pub id: u32,
        }

        #[pymethods]
        impl $name {
            fn __repr__(&self) -> String {
                format!(concat!(stringify!($name), "(id={})"), self.id)
            }
        }
    };
}

handle!(Point, "A point of a sketch, by id.");
handle!(Line, "A line segment of a sketch, by id.");
handle!(Circle, "A circle of a sketch, by id.");
handle!(Arc, "A circular arc of a sketch, by id.");

/// A circle or an arc, where a constraint treats them alike.
#[derive(FromPyObject)]
pub enum RoundArg<'py> {
    Circle(PyRef<'py, Circle>),
    Arc(PyRef<'py, Arc>),
}

/// A direction: a line, or (with a second argument) two points.
#[derive(FromPyObject)]
pub enum SpanArg<'py> {
    Line(PyRef<'py, Line>),
    Point(PyRef<'py, Point>),
}

/// What a cascading removal took out of a sketch, in id order.
#[pyclass(frozen, eq, get_all, module = "laman")]
#[derive(Clone, PartialEq, Eq)]
pub struct Removed {
    /// The entity ids removed, including the one asked for.
    pub entities: Vec<u32>,
    /// The constraint ids removed.
    pub constraints: Vec<u32>,
}

#[pymethods]
impl Removed {
    fn __repr__(&self) -> String {
        format!(
            "Removed(entities={:?}, constraints={:?})",
            self.entities, self.constraints
        )
    }
}

/// The error type a builder returns.
pub type Built<T> = Result<T, ModelError>;

/// Resolves handles against the sketch, so a handle from another sketch (or
/// of a removed entity) is refused with a typed error naming its id.
pub struct Resolve<'a>(pub &'a Sketch);

impl Resolve<'_> {
    pub fn point(&self, p: &Point) -> Built<laman::model::Point> {
        self.0.point_handle(EntityId(p.id))
    }

    pub fn line(&self, l: &Line) -> Built<laman::model::Line> {
        self.0.line_handle(EntityId(l.id))
    }

    pub fn round(&self, r: &RoundArg<'_>) -> Built<laman::model::Round> {
        Ok(match r {
            RoundArg::Circle(c) => self.0.circle_handle(EntityId(c.id))?.into(),
            RoundArg::Arc(a) => self.0.arc_handle(EntityId(a.id))?.into(),
        })
    }

    pub fn span(&self, a: &SpanArg<'_>, b: Option<&Point>) -> PyResult<Built<laman::model::Span>> {
        match (a, b) {
            (SpanArg::Line(l), None) => Ok(self.line(l).map(Into::into)),
            (SpanArg::Point(p), Some(q)) => Ok(self
                .point(p)
                .and_then(|p| self.point(q).map(|q| (p, q).into()))),
            (SpanArg::Line(_), Some(_)) => Err(PyTypeError::new_err(
                "a line is a direction by itself: pass a line, or two points",
            )),
            (SpanArg::Point(_), None) => Err(PyTypeError::new_err(
                "a direction needs a line, or two points",
            )),
        }
    }
}

/// The flags, tag and payload an entity builder applies after the entity
/// exists. A failure removes the entity again, so a refused builder leaves
/// no half-built entity.
pub struct Decor {
    pub fixed: Vec<laman::model::Coordinate>,
    pub construction: bool,
    pub tag: Option<String>,
    pub payload: Option<Vec<u8>>,
}

pub fn decorate_entity(sk: &mut Sketch, id: EntityId, d: &Decor) -> Built<()> {
    let result = (|| {
        for &c in &d.fixed {
            sk.set_fixed(id, c, true)?;
        }
        if d.construction {
            sk.set_construction(id, true)?;
        }
        if let Some(tag) = &d.tag {
            sk.tag(id, tag)?;
        }
        if let Some(bytes) = &d.payload {
            sk.payload(id, laman::model::Payload(bytes.clone()))?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = sk.remove(id);
    }
    result
}

/// Gives a freshly added constraint the id the caller asked for (the sketch
/// minted `minted`), then its tag and payload. A failure removes it again.
pub fn decorate_constraint(
    sk: &mut Sketch,
    minted: ConstraintId,
    id: Option<u32>,
    tag: Option<&str>,
    payload: Option<&[u8]>,
) -> Built<ConstraintId> {
    let mut at = minted;
    let result = (|| {
        if let Some(wanted) = id.map(ConstraintId) {
            if wanted != minted {
                if sk.constraint(wanted).is_some() {
                    return Err(ModelError::DuplicateConstraintId { id: wanted });
                }
                if let Some(c) = sk.constraint(minted).cloned() {
                    sk.remove_constraint(minted)?;
                    sk.add_constraint_with_id(wanted, c)?;
                    at = wanted;
                }
            }
        }
        if let Some(tag) = tag {
            sk.tag_constraint(at, tag)?;
        }
        if let Some(bytes) = payload {
            sk.payload_constraint(at, laman::model::Payload(bytes.to_vec()))?;
        }
        Ok(())
    })();
    match result {
        Ok(()) => Ok(at),
        Err(e) => {
            let _ = sk.remove_constraint(at);
            Err(e)
        }
    }
}

/// The Rust sense for a Python `Sense` member.
pub fn sense_from_py(value: &Bound<'_, PyAny>) -> PyResult<Sense> {
    let cls = value.py().import("laman._types")?.getattr("Sense")?;
    if !value.is_instance(&cls)? {
        return Err(PyTypeError::new_err("sense must be a laman.Sense"));
    }
    match value.getattr("name")?.extract::<String>()?.as_str() {
        "Positive" => Ok(Sense::Positive),
        "Negative" => Ok(Sense::Negative),
        other => Err(PyTypeError::new_err(format!("unknown sense {other}"))),
    }
}

/// A `ModelError` as a `PyErr`, for the builders' `?`.
pub fn refuse<T>(py: Python<'_>, r: Built<T>) -> PyResult<T> {
    r.map_err(|e| model_error(py, &e))
}
