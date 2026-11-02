//! The sketch, its coordinates and the serde form.

use crate::build::{
    Arc, Circle, Decor, Line, Point, Removed, Resolve, RoundArg, SpanArg, decorate_constraint,
    decorate_entity, refuse, sense_from_py,
};
use crate::errors::model_error;
use crate::solve::{Solution, SolverOptions};
use laman::model::{Constraint, ConstraintId, Contact, Coordinate, EntityId, ModelError, Payload};
use laman_debug::fixture::coordinate_name;
use laman_debug::json::{LoadError, sketch_from_str, sketch_to_string};
use pyo3::exceptions::{PyKeyError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList};

/// The parameter named `x`, `y` or `r`.
pub fn coordinate_from_name(name: &str) -> PyResult<Coordinate> {
    match name {
        "x" => Ok(Coordinate::X),
        "y" => Ok(Coordinate::Y),
        "r" => Ok(Coordinate::R),
        other => Err(PyValueError::new_err(format!(
            "unknown coordinate {other:?}: expected \"x\", \"y\" or \"r\""
        ))),
    }
}

/// A parameter's name: `x`, `y` or `r`.
pub fn coordinate_name_of(c: Coordinate) -> &'static str {
    coordinate_name(c)
}

/// The value of every parameter of a sketch, keyed by `(entity id,
/// "x" | "y" | "r")`, in id order.
#[pyclass(frozen, eq, module = "laman")]
#[derive(Clone, PartialEq)]
pub struct Coordinates {
    pub inner: laman::model::Coordinates,
}

fn key(id: EntityId, c: Coordinate) -> (u32, String) {
    (id.0, coordinate_name(c).to_owned())
}

#[pymethods]
impl Coordinates {
    /// Builds coordinates from a mapping (or pairs) of `(id, name)` to value.
    #[new]
    #[pyo3(signature = (items=None))]
    fn new(items: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let mut inner = laman::model::Coordinates::new();
        if let Some(items) = items {
            let pairs = match items.downcast::<PyDict>() {
                Ok(dict) => dict.items().into_any(),
                Err(_) => items.clone(),
            };
            for pair in pairs.try_iter()? {
                let ((id, name), value) = pair?.extract::<((u32, String), f64)>()?;
                inner.insert((EntityId(id), coordinate_from_name(&name)?), value);
            }
        }
        Ok(Self { inner })
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __getitem__(&self, key: (u32, String)) -> PyResult<f64> {
        let c = coordinate_from_name(&key.1)?;
        self.inner
            .get(&(EntityId(key.0), c))
            .copied()
            .ok_or_else(|| PyKeyError::new_err(key))
    }

    fn __contains__(&self, key: (u32, String)) -> bool {
        coordinate_from_name(&key.1).is_ok_and(|c| self.inner.contains_key(&(EntityId(key.0), c)))
    }

    fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, pyo3::types::PyIterator>> {
        self.keys(py)?.into_any().try_iter()
    }

    /// The `(id, name)` keys, in id order.
    fn keys<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        PyList::new(py, self.inner.keys().map(|&(id, c)| key(id, c)))
    }

    /// The values, in key order.
    fn values<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        PyList::new(py, self.inner.values().copied())
    }

    /// The `((id, name), value)` pairs, in key order.
    fn items<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        PyList::new(py, self.inner.iter().map(|(&(id, c), &v)| (key(id, c), v)))
    }

    fn __repr__(&self) -> String {
        format!("Coordinates(<{} parameters>)", self.inner.len())
    }
}

/// A sketch: entities and the constraints between them.
#[pyclass(module = "laman")]
pub struct Sketch {
    pub inner: laman::model::Sketch,
}

#[pymethods]
impl Sketch {
    /// An empty sketch.
    #[new]
    fn new() -> Self {
        Self {
            inner: laman::model::Sketch::new(),
        }
    }

    /// Loads a sketch from its JSON serde form. Raises `ValueError` for text
    /// that is not a sketch file and a `ModelError` for a sketch the model
    /// refuses.
    #[staticmethod]
    fn from_json(py: Python<'_>, text: &str) -> PyResult<Self> {
        let inner = sketch_from_str(text).map_err(|e| match e {
            LoadError::Json(e) => PyValueError::new_err(e.to_string()),
            LoadError::Model(e) => model_error(py, &e),
        })?;
        Ok(Self { inner })
    }

    /// The sketch in its JSON serde form.
    fn to_json(&self) -> String {
        sketch_to_string(&self.inner)
    }

    /// Counts every change to the sketch.
    #[getter]
    fn revision(&self) -> u64 {
        self.inner.revision()
    }

    /// The current value of every parameter.
    fn coordinates(&self) -> Coordinates {
        Coordinates {
            inner: self.inner.coordinates(),
        }
    }

    /// Solves from the current coordinates and changes nothing.
    #[pyo3(signature = (options=None))]
    fn solve(&self, py: Python<'_>, options: Option<&SolverOptions>) -> PyResult<Solution> {
        let options = options.map_or_else(laman::solution::SolverOptions::default, |o| o.inner);
        Solution::new(py, self.inner.solve(&options))
    }

    /// Writes a solution's coordinates into the sketch, moving its revision.
    /// Raises a `ModelError` (`Stale`, `ForeignSolution`, ...) for a
    /// solution that was not solved from this sketch as it is now.
    fn apply(&mut self, py: Python<'_>, solution: &Solution) -> PyResult<()> {
        self.inner
            .apply(&solution.inner)
            .map_err(|e| model_error(py, &e))
    }

    /// Opens a drag session on the point `point` (an id): a copy of the
    /// sketch is dragged, this one untouched until `apply`.
    #[pyo3(signature = (point, options=None))]
    fn drag(
        &self,
        py: Python<'_>,
        point: u32,
        options: Option<&SolverOptions>,
    ) -> PyResult<DragSession> {
        let options = options.map_or_else(laman::solution::SolverOptions::default, |o| o.inner);
        let session = self
            .inner
            .drag(EntityId(point), &options)
            .map_err(|e| model_error(py, &e))?;
        Ok(DragSession {
            inner: Some(session),
        })
    }

    // ---- entities ----

    /// Adds a point at `(x, y)`. `fixed` fixes both coordinates; `id`
    /// names it instead of letting the sketch mint one.
    #[pyo3(signature = (x, y, *, fixed=false, construction=false, tag=None, payload=None, id=None))]
    #[allow(clippy::too_many_arguments)]
    fn point(
        &mut self,
        py: Python<'_>,
        x: f64,
        y: f64,
        fixed: bool,
        construction: bool,
        tag: Option<String>,
        payload: Option<Vec<u8>>,
        id: Option<u32>,
    ) -> PyResult<Point> {
        let made = match id {
            Some(id) => self.inner.point_with_id(EntityId(id), x, y),
            None => self.inner.point(x, y),
        };
        let made = refuse(py, made)?;
        let decor = Decor {
            fixed: if fixed {
                vec![Coordinate::X, Coordinate::Y]
            } else {
                Vec::new()
            },
            construction,
            tag,
            payload,
        };
        refuse(py, decorate_entity(&mut self.inner, made.id(), &decor))?;
        Ok(Point { id: made.id().0 })
    }

    /// Adds a line segment from point `a` to point `b`.
    #[pyo3(signature = (a, b, *, construction=false, tag=None, payload=None, id=None))]
    #[allow(clippy::too_many_arguments)]
    fn line(
        &mut self,
        py: Python<'_>,
        a: &Point,
        b: &Point,
        construction: bool,
        tag: Option<String>,
        payload: Option<Vec<u8>>,
        id: Option<u32>,
    ) -> PyResult<Line> {
        let (pa, pb) = {
            let r = Resolve(&self.inner);
            (refuse(py, r.point(a))?, refuse(py, r.point(b))?)
        };
        let made = match id {
            Some(id) => self.inner.line_with_id(EntityId(id), pa, pb),
            None => self.inner.line(pa, pb),
        };
        let made = refuse(py, made)?;
        let decor = Decor {
            fixed: Vec::new(),
            construction,
            tag,
            payload,
        };
        refuse(py, decorate_entity(&mut self.inner, made.id(), &decor))?;
        Ok(Line { id: made.id().0 })
    }

    /// Adds a circle of radius `r` about the point `centre`. `fixed` fixes
    /// the radius.
    #[pyo3(signature = (centre, r, *, fixed=false, construction=false, tag=None, payload=None, id=None))]
    #[allow(clippy::too_many_arguments)]
    fn circle(
        &mut self,
        py: Python<'_>,
        centre: &Point,
        r: f64,
        fixed: bool,
        construction: bool,
        tag: Option<String>,
        payload: Option<Vec<u8>>,
        id: Option<u32>,
    ) -> PyResult<Circle> {
        let c = refuse(py, Resolve(&self.inner).point(centre))?;
        let made = match id {
            Some(id) => self.inner.circle_with_id(EntityId(id), c, r),
            None => self.inner.circle(c, r),
        };
        let made = refuse(py, made)?;
        let decor = Decor {
            fixed: if fixed {
                vec![Coordinate::R]
            } else {
                Vec::new()
            },
            construction,
            tag,
            payload,
        };
        refuse(py, decorate_entity(&mut self.inner, made.id(), &decor))?;
        Ok(Circle { id: made.id().0 })
    }

    /// Adds a circular arc about `centre` from `start` to `end`, counter-
    /// clockwise.
    #[pyo3(signature = (centre, start, end, *, construction=false, tag=None, payload=None, id=None))]
    #[allow(clippy::too_many_arguments)]
    fn arc(
        &mut self,
        py: Python<'_>,
        centre: &Point,
        start: &Point,
        end: &Point,
        construction: bool,
        tag: Option<String>,
        payload: Option<Vec<u8>>,
        id: Option<u32>,
    ) -> PyResult<Arc> {
        let (c, s, e) = {
            let r = Resolve(&self.inner);
            (
                refuse(py, r.point(centre))?,
                refuse(py, r.point(start))?,
                refuse(py, r.point(end))?,
            )
        };
        let made = match id {
            Some(id) => self.inner.arc_with_id(EntityId(id), c, s, e),
            None => self.inner.arc(c, s, e),
        };
        let made = refuse(py, made)?;
        let decor = Decor {
            fixed: Vec::new(),
            construction,
            tag,
            payload,
        };
        refuse(py, decorate_entity(&mut self.inner, made.id(), &decor))?;
        Ok(Arc { id: made.id().0 })
    }

    /// Fixes or frees one parameter (`"x"`, `"y"` or `"r"`) of an entity.
    fn set_fixed(
        &mut self,
        py: Python<'_>,
        id: u32,
        coordinate: &str,
        fixed: bool,
    ) -> PyResult<()> {
        let c = coordinate_from_name(coordinate)?;
        refuse(py, self.inner.set_fixed(EntityId(id), c, fixed))
    }

    /// Whether a parameter is fixed.
    fn is_fixed(&self, id: u32, coordinate: &str) -> PyResult<bool> {
        Ok(self
            .inner
            .is_fixed(EntityId(id), coordinate_from_name(coordinate)?))
    }

    /// Flags an entity as construction, or clears the flag.
    fn set_construction(&mut self, py: Python<'_>, id: u32, construction: bool) -> PyResult<()> {
        refuse(py, self.inner.set_construction(EntityId(id), construction))
    }

    /// Whether an entity is flagged construction.
    fn is_construction(&self, id: u32) -> bool {
        self.inner.is_construction(EntityId(id))
    }

    /// Tags an entity.
    fn tag(&mut self, py: Python<'_>, id: u32, tag: &str) -> PyResult<()> {
        refuse(py, self.inner.tag(EntityId(id), tag))
    }

    /// Attaches opaque bytes to an entity.
    fn payload(&mut self, py: Python<'_>, id: u32, payload: Vec<u8>) -> PyResult<()> {
        refuse(py, self.inner.payload(EntityId(id), Payload(payload)))
    }

    /// Removes an entity nothing uses; `InUse` names what does.
    fn remove(&mut self, py: Python<'_>, id: u32) -> PyResult<()> {
        refuse(py, self.inner.remove(EntityId(id)))
    }

    /// Removes an entity and, in cascade, whatever uses it.
    fn remove_with_constraints(&mut self, py: Python<'_>, id: u32) -> PyResult<Removed> {
        let gone = refuse(py, self.inner.remove_with_constraints(EntityId(id)))?;
        Ok(Removed {
            entities: gone.entities.iter().map(|e| e.0).collect(),
            constraints: gone.constraints.iter().map(|c| c.0).collect(),
        })
    }

    // ---- constraints ----

    /// Removes a constraint.
    fn remove_constraint(&mut self, py: Python<'_>, id: u32) -> PyResult<()> {
        refuse(py, self.inner.remove_constraint(ConstraintId(id)))
    }

    /// Tags a constraint.
    fn tag_constraint(&mut self, py: Python<'_>, id: u32, tag: &str) -> PyResult<()> {
        refuse(py, self.inner.tag_constraint(ConstraintId(id), tag))
    }

    /// Attaches opaque bytes to a constraint.
    fn payload_constraint(&mut self, py: Python<'_>, id: u32, payload: Vec<u8>) -> PyResult<()> {
        refuse(
            py,
            self.inner
                .payload_constraint(ConstraintId(id), Payload(payload)),
        )
    }

    /// Points `p` and `q` are one.
    #[pyo3(signature = (p, q, *, tag=None, payload=None, id=None))]
    fn coincident(
        &mut self,
        py: Python<'_>,
        p: &Point,
        q: &Point,
        tag: Option<String>,
        payload: Option<Vec<u8>>,
        id: Option<u32>,
    ) -> PyResult<u32> {
        let made = (|| {
            let r = Resolve(&self.inner);
            let (p, q) = (r.point(p)?, r.point(q)?);
            self.inner.coincident(p, q)
        })();
        self.finish(py, made, tag, payload, id)
    }

    /// Point `p` lies on the infinite line.
    #[pyo3(signature = (p, line, *, tag=None, payload=None, id=None))]
    fn point_on_line(
        &mut self,
        py: Python<'_>,
        p: &Point,
        line: &Line,
        tag: Option<String>,
        payload: Option<Vec<u8>>,
        id: Option<u32>,
    ) -> PyResult<u32> {
        let made = (|| {
            let r = Resolve(&self.inner);
            let (p, l) = (r.point(p)?, r.line(line)?);
            self.inner.point_on_line(p, l)
        })();
        self.finish(py, made, tag, payload, id)
    }

    /// Point `p` lies on a circle, or on an arc's whole circle.
    #[pyo3(signature = (p, on, *, tag=None, payload=None, id=None))]
    fn point_on_circle(
        &mut self,
        py: Python<'_>,
        p: &Point,
        on: RoundArg<'_>,
        tag: Option<String>,
        payload: Option<Vec<u8>>,
        id: Option<u32>,
    ) -> PyResult<u32> {
        let made = (|| {
            let r = Resolve(&self.inner);
            let (p, o) = (r.point(p)?, r.round(&on)?);
            self.inner.point_on_circle(p, o)
        })();
        self.finish(py, made, tag, payload, id)
    }

    /// A line, or the line through two points, is horizontal.
    #[pyo3(signature = (a, b=None, *, tag=None, payload=None, id=None))]
    fn horizontal(
        &mut self,
        py: Python<'_>,
        a: SpanArg<'_>,
        b: Option<&Point>,
        tag: Option<String>,
        payload: Option<Vec<u8>>,
        id: Option<u32>,
    ) -> PyResult<u32> {
        let span = Resolve(&self.inner).span(&a, b)?;
        let made = span.and_then(|s| self.inner.horizontal(s));
        self.finish(py, made, tag, payload, id)
    }

    /// A line, or the line through two points, is vertical.
    #[pyo3(signature = (a, b=None, *, tag=None, payload=None, id=None))]
    fn vertical(
        &mut self,
        py: Python<'_>,
        a: SpanArg<'_>,
        b: Option<&Point>,
        tag: Option<String>,
        payload: Option<Vec<u8>>,
        id: Option<u32>,
    ) -> PyResult<u32> {
        let span = Resolve(&self.inner).span(&a, b)?;
        let made = span.and_then(|s| self.inner.vertical(s));
        self.finish(py, made, tag, payload, id)
    }

    /// Lines `a` and `b` are parallel.
    #[pyo3(signature = (a, b, *, tag=None, payload=None, id=None))]
    fn parallel(
        &mut self,
        py: Python<'_>,
        a: &Line,
        b: &Line,
        tag: Option<String>,
        payload: Option<Vec<u8>>,
        id: Option<u32>,
    ) -> PyResult<u32> {
        let made = (|| {
            let r = Resolve(&self.inner);
            let (a, b) = (r.line(a)?, r.line(b)?);
            self.inner.parallel(a, b)
        })();
        self.finish(py, made, tag, payload, id)
    }

    /// Lines `a` and `b` are perpendicular.
    #[pyo3(signature = (a, b, *, tag=None, payload=None, id=None))]
    fn perpendicular(
        &mut self,
        py: Python<'_>,
        a: &Line,
        b: &Line,
        tag: Option<String>,
        payload: Option<Vec<u8>>,
        id: Option<u32>,
    ) -> PyResult<u32> {
        let made = (|| {
            let r = Resolve(&self.inner);
            let (a, b) = (r.line(a)?, r.line(b)?);
            self.inner.perpendicular(a, b)
        })();
        self.finish(py, made, tag, payload, id)
    }

    /// Lines `a` and `b` have one length.
    #[pyo3(signature = (a, b, *, tag=None, payload=None, id=None))]
    fn equal_length(
        &mut self,
        py: Python<'_>,
        a: &Line,
        b: &Line,
        tag: Option<String>,
        payload: Option<Vec<u8>>,
        id: Option<u32>,
    ) -> PyResult<u32> {
        let made = (|| {
            let r = Resolve(&self.inner);
            let (a, b) = (r.line(a)?, r.line(b)?);
            self.inner.equal_length(a, b)
        })();
        self.finish(py, made, tag, payload, id)
    }

    /// Circles or arcs `a` and `b` have one radius.
    #[pyo3(signature = (a, b, *, tag=None, payload=None, id=None))]
    fn equal_radius(
        &mut self,
        py: Python<'_>,
        a: RoundArg<'_>,
        b: RoundArg<'_>,
        tag: Option<String>,
        payload: Option<Vec<u8>>,
        id: Option<u32>,
    ) -> PyResult<u32> {
        let made = (|| {
            let r = Resolve(&self.inner);
            let (a, b) = (r.round(&a)?, r.round(&b)?);
            self.inner.equal_radius(a, b)
        })();
        self.finish(py, made, tag, payload, id)
    }

    /// The line touches the circle or arc. The side of the line the centre
    /// is on is read from the drawing, or given as `sense`.
    #[pyo3(signature = (line, round, *, sense=None, tag=None, payload=None, id=None))]
    #[allow(clippy::too_many_arguments)]
    fn tangent_line_circle(
        &mut self,
        py: Python<'_>,
        line: &Line,
        round: RoundArg<'_>,
        sense: Option<&Bound<'_, PyAny>>,
        tag: Option<String>,
        payload: Option<Vec<u8>>,
        id: Option<u32>,
    ) -> PyResult<u32> {
        let sense = sense.map(sense_from_py).transpose()?;
        let made = (|| {
            let r = Resolve(&self.inner);
            let (l, o) = (r.line(line)?, r.round(&round)?);
            match sense {
                None => self.inner.tangent_line_circle(l, o),
                Some(sense) => self.inner.add_constraint(Constraint::TangentLineCircle {
                    line: l,
                    round: o,
                    sense,
                }),
            }
        })();
        self.finish(py, made, tag, payload, id)
    }

    /// Circles or arcs `a` and `b` touch. How they touch is read from the
    /// drawing, or given: `external=True`, or `sense` for one inside the
    /// other (positive when the first is the larger).
    #[pyo3(signature = (a, b, *, external=false, sense=None, tag=None, payload=None, id=None))]
    #[allow(clippy::too_many_arguments)]
    fn tangent_circles(
        &mut self,
        py: Python<'_>,
        a: RoundArg<'_>,
        b: RoundArg<'_>,
        external: bool,
        sense: Option<&Bound<'_, PyAny>>,
        tag: Option<String>,
        payload: Option<Vec<u8>>,
        id: Option<u32>,
    ) -> PyResult<u32> {
        if external && sense.is_some() {
            return Err(PyTypeError::new_err(
                "external and sense exclude each other",
            ));
        }
        let contact = if external {
            Some(Contact::External)
        } else {
            sense.map(sense_from_py).transpose()?.map(Contact::Internal)
        };
        let made = (|| {
            let r = Resolve(&self.inner);
            let (a, b) = (r.round(&a)?, r.round(&b)?);
            match contact {
                None => self.inner.tangent_circles(a, b),
                Some(contact) => {
                    self.inner
                        .add_constraint(Constraint::TangentCircles { a, b, contact })
                }
            }
        })();
        self.finish(py, made, tag, payload, id)
    }

    /// Point `p` is the middle of `line`.
    #[pyo3(signature = (p, line, *, tag=None, payload=None, id=None))]
    fn midpoint(
        &mut self,
        py: Python<'_>,
        p: &Point,
        line: &Line,
        tag: Option<String>,
        payload: Option<Vec<u8>>,
        id: Option<u32>,
    ) -> PyResult<u32> {
        let made = (|| {
            let r = Resolve(&self.inner);
            let (p, l) = (r.point(p)?, r.line(line)?);
            self.inner.midpoint(p, l)
        })();
        self.finish(py, made, tag, payload, id)
    }

    /// Points `p` and `q` mirror each other in `axis`.
    #[pyo3(signature = (p, q, axis, *, tag=None, payload=None, id=None))]
    #[allow(clippy::too_many_arguments)]
    fn symmetric(
        &mut self,
        py: Python<'_>,
        p: &Point,
        q: &Point,
        axis: &Line,
        tag: Option<String>,
        payload: Option<Vec<u8>>,
        id: Option<u32>,
    ) -> PyResult<u32> {
        let made = (|| {
            let r = Resolve(&self.inner);
            let (p, q, l) = (r.point(p)?, r.point(q)?, r.line(axis)?);
            self.inner.symmetric(p, q, l)
        })();
        self.finish(py, made, tag, payload, id)
    }

    /// Point `p` is at `(x, y)`, as two equations a diagnostic can blame.
    #[pyo3(signature = (p, x, y, *, tag=None, payload=None, id=None))]
    #[allow(clippy::too_many_arguments)]
    fn fix(
        &mut self,
        py: Python<'_>,
        p: &Point,
        x: f64,
        y: f64,
        tag: Option<String>,
        payload: Option<Vec<u8>>,
        id: Option<u32>,
    ) -> PyResult<u32> {
        let made = (|| {
            let p = Resolve(&self.inner).point(p)?;
            self.inner.fix(p, x, y)
        })();
        self.finish(py, made, tag, payload, id)
    }

    /// Points `p` and `q` are `value` apart, `value > 0`.
    #[pyo3(signature = (p, q, value, *, tag=None, payload=None, id=None))]
    #[allow(clippy::too_many_arguments)]
    fn distance(
        &mut self,
        py: Python<'_>,
        p: &Point,
        q: &Point,
        value: f64,
        tag: Option<String>,
        payload: Option<Vec<u8>>,
        id: Option<u32>,
    ) -> PyResult<u32> {
        let made = (|| {
            let r = Resolve(&self.inner);
            let (p, q) = (r.point(p)?, r.point(q)?);
            self.inner.distance(p, q, value)
        })();
        self.finish(py, made, tag, payload, id)
    }

    /// Point `p` is `value` from the line, on the side it was drawn on.
    #[pyo3(signature = (p, line, value, *, tag=None, payload=None, id=None))]
    #[allow(clippy::too_many_arguments)]
    fn point_line_distance(
        &mut self,
        py: Python<'_>,
        p: &Point,
        line: &Line,
        value: f64,
        tag: Option<String>,
        payload: Option<Vec<u8>>,
        id: Option<u32>,
    ) -> PyResult<u32> {
        let made = (|| {
            let r = Resolve(&self.inner);
            let (p, l) = (r.point(p)?, r.line(line)?);
            self.inner.point_line_distance(p, l, value)
        })();
        self.finish(py, made, tag, payload, id)
    }

    /// The circle or arc has radius `value > 0`.
    #[pyo3(signature = (round, value, *, tag=None, payload=None, id=None))]
    fn radius(
        &mut self,
        py: Python<'_>,
        round: RoundArg<'_>,
        value: f64,
        tag: Option<String>,
        payload: Option<Vec<u8>>,
        id: Option<u32>,
    ) -> PyResult<u32> {
        let made = (|| {
            let o = Resolve(&self.inner).round(&round)?;
            self.inner.radius(o, value)
        })();
        self.finish(py, made, tag, payload, id)
    }

    /// The angle from line `a` to line `b` is `value` radians, in `(0, pi)`.
    #[pyo3(signature = (a, b, value, *, tag=None, payload=None, id=None))]
    #[allow(clippy::too_many_arguments)]
    fn angle(
        &mut self,
        py: Python<'_>,
        a: &Line,
        b: &Line,
        value: f64,
        tag: Option<String>,
        payload: Option<Vec<u8>>,
        id: Option<u32>,
    ) -> PyResult<u32> {
        let made = (|| {
            let r = Resolve(&self.inner);
            let (a, b) = (r.line(a)?, r.line(b)?);
            self.inner.angle(a, b, value)
        })();
        self.finish(py, made, tag, payload, id)
    }

    /// The tag on an entity, or `None`.
    fn tag_of(&self, id: u32) -> Option<String> {
        self.inner.tag_of(EntityId(id)).map(str::to_owned)
    }

    /// The payload on an entity, or `None`.
    fn payload_of<'py>(&self, py: Python<'py>, id: u32) -> Option<Bound<'py, PyBytes>> {
        self.inner
            .payload_of(EntityId(id))
            .map(|p| PyBytes::new(py, &p.0))
    }

    /// The tag on a constraint, or `None`.
    fn constraint_tag_of(&self, id: u32) -> Option<String> {
        self.inner
            .constraint_tag_of(laman::model::ConstraintId(id))
            .map(str::to_owned)
    }

    /// The payload on a constraint, or `None`.
    fn constraint_payload_of<'py>(&self, py: Python<'py>, id: u32) -> Option<Bound<'py, PyBytes>> {
        self.inner
            .constraint_payload_of(laman::model::ConstraintId(id))
            .map(|p| PyBytes::new(py, &p.0))
    }
}

/// A point being dragged through a series of targets, one frame per `to`.
///
/// `finish` ends it; any call after that raises `ValueError`, since Rust
/// moves the session and Python cannot.
#[pyclass(module = "laman")]
pub struct DragSession {
    inner: Option<laman::drag::DragSession>,
}

fn finished() -> PyErr {
    PyValueError::new_err("the drag session is finished")
}

#[pymethods]
impl DragSession {
    /// One frame: solves with the point pulled toward `(x, y)`, from the
    /// previous step. Always returns a `Solution`.
    fn to(&mut self, py: Python<'_>, x: f64, y: f64) -> PyResult<Solution> {
        let session = self.inner.as_mut().ok_or_else(finished)?;
        Solution::new(py, session.to([x, y]))
    }

    /// Ends the session and returns its last solution.
    fn finish(&mut self, py: Python<'_>) -> PyResult<Solution> {
        let session = self.inner.take().ok_or_else(finished)?;
        Solution::new(py, session.finish())
    }

    fn __repr__(&self) -> String {
        if self.inner.is_some() {
            "DragSession(open)".to_owned()
        } else {
            "DragSession(finished)".to_owned()
        }
    }
}

impl Sketch {
    /// A constraint builder's tail: the id, tag and payload asked for.
    fn finish(
        &mut self,
        py: Python<'_>,
        made: Result<ConstraintId, ModelError>,
        tag: Option<String>,
        payload: Option<Vec<u8>>,
        id: Option<u32>,
    ) -> PyResult<u32> {
        let minted = refuse(py, made)?;
        let at = decorate_constraint(
            &mut self.inner,
            minted,
            id,
            tag.as_deref(),
            payload.as_deref(),
        );
        Ok(refuse(py, at)?.0)
    }
}
