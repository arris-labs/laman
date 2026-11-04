//! Ordered structural compilation and contact provenance (ADR-0002/0003).
// Internal until the solve API lands in M2 step 7.
#![allow(dead_code)]

use crate::model::tolerance::{self, Component, Source, Tolerances};
use crate::model::{
    Constraint, ConstraintId, Coordinate, Coordinates, Entity, EntityId, Point, Sketch,
};
use crate::residual::{self, Param, Unit};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Membership {
    Explicit(ConstraintId),
    ArcStart(EntityId),
    ArcEnd(EntityId),
}

#[derive(Clone, Debug)]
pub(crate) struct Contact {
    pub point: Point,
    pub centre: Point,
    pub a: Point,
    pub b: Point,
    pub membership: Membership,
    pub sense: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct Equation {
    pub source: Source,
    pub index: usize,
    pub contact: Option<Contact>,
}

/// A goal equation, `value(param) − target = 0`: a drag target's
/// coordinate, of the `Goal` priority (ARCHITECTURE.md §Solve, Priorities).
/// Required equations are solved first; a goal only in the null space they
/// leave, so it can never push one off. It reads one free parameter and
/// joins that parameter's component; it is compiled after every required
/// equation and kept out of [`System::equations`], so no diagnostic (DoF,
/// redundancy, conflicts) ever reads one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Goal {
    pub param: Param,
    pub target: f64,
}

#[derive(Debug)]
pub(crate) struct System<'a> {
    pub sketch: &'a Sketch,
    pub components: Vec<Component>,
    /// Every required equation, in source order.
    pub equations: Vec<Equation>,
    /// Every goal equation, after the required ones, in the caller's order;
    /// a goal on a fixed or unknown parameter is dropped.
    pub goals: Vec<Goal>,
    /// The components' decompositions when a caller kept them
    /// (`DragSession`); `None` cuts them as asked.
    pub decompositions: Option<&'a crate::stage::Decompositions>,
    /// Every equation's index in [`Self::equations`] by its source, so
    /// reading a part's rows does not scan the others.
    by_source: BTreeMap<Source, Vec<usize>>,
    /// The components cut once, on first ask, when no kept cut is given:
    /// the solve, its acceptance of a staged result and the analysis then
    /// share one.
    pub cut: std::cell::OnceCell<crate::stage::Decompositions>,
}

/// One equation read sparsely ([`System::rows`]).
#[derive(Debug)]
pub(crate) struct Row {
    /// Its index in [`System::equations`].
    pub index: usize,
    /// Its residual, normalized.
    pub value: f64,
    /// Its gradient by parameter, unnormalized.
    pub terms: BTreeMap<Param, f64>,
    /// What a term is multiplied by to be a normalized Jacobian entry.
    pub factor: f64,
}

#[derive(Debug)]
pub(crate) struct Evaluation {
    pub values: Vec<f64>,
    pub jacobian: Vec<Vec<f64>>,
    pub satisfied: bool,
    pub valid: bool,
}

impl<'a> System<'a> {
    pub fn new(sketch: &'a Sketch) -> Self {
        Self::under(sketch, &Tolerances::new(sketch))
    }

    /// The system of `sketch` whose components are `policy`'s, which must be
    /// [`Tolerances::new`] of `sketch` (the frame shares one, ARCHITECTURE.md
    /// §Frame budget): what [`Self::new`] makes, without measuring again.
    pub fn under(sketch: &'a Sketch, policy: &Tolerances) -> Self {
        let mut equations = Vec::new();
        for (id, e) in sketch.entities() {
            if matches!(e, Entity::Arc { .. }) {
                equations.push(Equation {
                    source: Source::Arc(id),
                    index: 0,
                    contact: None,
                });
            }
        }
        // Each point's first explicit membership of each round, read once.
        let mut on_round: BTreeMap<(EntityId, EntityId), ConstraintId> = BTreeMap::new();
        for (id, c) in sketch.constraints() {
            if let Constraint::PointOnCircle { p, on } = c {
                on_round.entry((p.id(), on.id())).or_insert(id);
            }
        }
        for (id, c) in sketch.constraints() {
            let count = match c {
                Constraint::Coincident { .. }
                | Constraint::Midpoint { .. }
                | Constraint::Symmetric { .. }
                | Constraint::Fix { .. } => 2,
                Constraint::PointOnLine { .. }
                | Constraint::PointOnCircle { .. }
                | Constraint::Horizontal(_)
                | Constraint::Vertical(_)
                | Constraint::Parallel { .. }
                | Constraint::Perpendicular { .. }
                | Constraint::EqualLength { .. }
                | Constraint::EqualRadius { .. }
                | Constraint::TangentLineCircle { .. }
                | Constraint::TangentCircles { .. }
                | Constraint::Distance { .. }
                | Constraint::PointLineDistance { .. }
                | Constraint::Radius { .. }
                | Constraint::Angle { .. } => 1,
            };
            for index in 0..count {
                equations.push(Equation {
                    source: Source::Constraint(id),
                    index,
                    contact: recognize(sketch, c, &on_round),
                });
            }
        }
        let mut by_source: BTreeMap<Source, Vec<usize>> = BTreeMap::new();
        for (index, eq) in equations.iter().enumerate() {
            by_source.entry(eq.source).or_default().push(index);
        }
        Self {
            sketch,
            components: policy.components.clone(),
            equations,
            goals: vec![],
            decompositions: None,
            by_source,
            cut: std::cell::OnceCell::new(),
        }
    }

    /// The system of `sketch` with `goals` compiled after its required
    /// equations. A goal whose parameter is fixed or not in the sketch reads
    /// no free parameter and is dropped: a point with one fixed coordinate is
    /// pursued along the other.
    pub fn with_goals(sketch: &'a Sketch, goals: &[Goal]) -> Self {
        Self::with_goals_under(sketch, goals, &Tolerances::new(sketch))
    }

    /// [`Self::with_goals`] under `policy`, as [`Self::under`] takes it.
    pub fn with_goals_under(sketch: &'a Sketch, goals: &[Goal], policy: &Tolerances) -> Self {
        let mut system = Self::under(sketch, policy);
        system.goals = goals
            .iter()
            .filter(|g| {
                system
                    .components
                    .iter()
                    .any(|c| c.params.contains(&g.param))
            })
            .copied()
            .collect();
        system
    }

    /// The goals that read `component`'s parameters, in compiled order.
    pub fn goals_of(&self, component: &Component) -> Vec<Goal> {
        self.goals
            .iter()
            .filter(|g| component.params.contains(&g.param))
            .copied()
            .collect()
    }

    /// The indices in [`Self::equations`] of `component`'s equations,
    /// ascending: compiled order.
    pub fn rows_of(&self, component: &Component) -> Vec<usize> {
        let mut rows: Vec<usize> = component
            .sources
            .iter()
            .filter_map(|s| self.by_source.get(s))
            .flatten()
            .copied()
            .collect();
        rows.sort_unstable();
        rows.dedup();
        rows
    }

    /// Length acceptance at `coordinates`: the frozen input tolerance, tightened
    /// to the one the checker measures there (ADR-0004). It never loosens, so a
    /// solve cannot shrink or grow its way past the checker.
    pub fn acceptance(&self, component: &Component, coordinates: &Coordinates) -> f64 {
        let here = tolerance::scale_at(component, coordinates);
        tolerance::length(component.scale).min(tolerance::length(here))
    }

    pub fn evaluate(&self, component: &Component, coordinates: &Coordinates) -> Evaluation {
        self.evaluate_to(component, coordinates, 1.0)
    }

    pub fn evaluate_to(
        &self,
        component: &Component,
        coordinates: &Coordinates,
        target: f64,
    ) -> Evaluation {
        self.evaluate_indexed(component, coordinates, target).0
    }

    /// [`Self::evaluate_to`] with, for each row it returns, the index of its
    /// equation in [`Self::equations`]: an equation that cannot be read at
    /// `coordinates` gives no row.
    pub fn evaluate_indexed(
        &self,
        component: &Component,
        coordinates: &Coordinates,
        target: f64,
    ) -> (Evaluation, Vec<usize>) {
        self.evaluate_within(component, component, coordinates, target)
    }

    /// [`Self::evaluate_to`] of `part`, a part of `whole` (plans/m6-stages):
    /// `part`'s equations over `part`'s parameters, every other parameter a
    /// constant, satisfied by `whole`'s acceptance, so a part meets the
    /// tolerance its component does.
    pub fn evaluate_part(
        &self,
        part: &Component,
        whole: &Component,
        coordinates: &Coordinates,
        target: f64,
    ) -> Evaluation {
        self.evaluate_within(part, whole, coordinates, target).0
    }

    fn evaluate_within(
        &self,
        component: &Component,
        whole: &Component,
        coordinates: &Coordinates,
        target: f64,
    ) -> (Evaluation, Vec<usize>) {
        let (rows, satisfied, valid) = self.rows(component, whole, coordinates, target);
        let mut out = Evaluation {
            values: vec![],
            jacobian: vec![],
            satisfied,
            valid,
        };
        let mut indices = Vec::new();
        // The columns by parameter, sorted: a part's parameters are already
        // in order, which the sort detects, and a lookup allocates nothing.
        let mut column: Vec<(Param, usize)> = component
            .params
            .iter()
            .enumerate()
            .map(|(j, p)| (*p, j))
            .collect();
        column.sort_by_key(|(p, _)| *p);
        for row in rows {
            // A column the row does not read is `0 × factor`: +0, the factor
            // being a positive finite scale.
            let mut dense = vec![0.0; component.params.len()];
            for (p, term) in &row.terms {
                if let Ok(at) = column.binary_search_by_key(p, |(q, _)| *q) {
                    if let Some(x) = column.get(at).and_then(|(_, j)| dense.get_mut(*j)) {
                        *x = term * row.factor;
                    }
                }
            }
            out.valid &= row.value.is_finite() && dense.iter().all(|x| x.is_finite());
            out.values.push(row.value);
            out.jacobian.push(dense);
            indices.push(row.index);
        }
        out.satisfied &= out.valid;
        (out, indices)
    }

    /// `component`'s equations at `coordinates` as sparse rows over every
    /// parameter they read, normalized as [`Self::evaluate_part`]'s: a row's
    /// Jacobian entry for a parameter is its term times the row's factor.
    /// Also whether every row meets `whole`'s acceptance times `target`, and
    /// whether every row could be read and is not degenerate; finiteness is
    /// the caller's to check.
    pub fn rows(
        &self,
        component: &Component,
        whole: &Component,
        coordinates: &Coordinates,
        target: f64,
    ) -> (Vec<Row>, bool, bool) {
        let scale = component.scale.max(tolerance::LENGTH_FLOOR);
        let acceptance = self.acceptance(whole, coordinates);
        let (mut satisfied, mut valid) = (true, true);
        let mut rows = Vec::new();
        for index in self.rows_of(component) {
            let Some(eq) = self.equations.get(index) else {
                continue;
            };
            let original = match eq.source {
                Source::Arc(id) => residual::intrinsic(self.sketch, coordinates, id),
                Source::Constraint(id) => residual::equations(self.sketch, coordinates, id)
                    .and_then(|e| e.as_slice().get(eq.index).copied()),
            };
            let Some(original) = original else {
                valid = false;
                satisfied = false;
                continue;
            };
            let divisor = match original.unit() {
                Unit::Length => scale,
                Unit::Ratio => 1.0,
            };
            let threshold = match original.unit() {
                Unit::Length => acceptance,
                Unit::Ratio => tolerance::ANGLE,
            };
            satisfied &= !original.is_degenerate() && original.value().abs() <= target * threshold;
            valid &= !original.is_degenerate();
            let (value, terms, domain) = if let Some(contact) = &eq.contact {
                contact.evaluate(coordinates, tolerance::length(component.scale))
            } else {
                (
                    original.value(),
                    original.terms().iter().copied().collect(),
                    true,
                )
            };
            valid &= domain;
            satisfied &= domain && value.abs() <= target * threshold;
            rows.push(Row {
                index,
                value: value / divisor,
                terms,
                factor: scale / divisor,
            });
        }
        (rows, satisfied && valid, valid)
    }
}

/// A tangency's contact, its point found among the line's ends: an arc's
/// own end, else a point with an explicit membership of the round, the
/// first in constraint order (`on_round`).
fn recognize(
    sk: &Sketch,
    c: &Constraint,
    on_round: &BTreeMap<(EntityId, EntityId), ConstraintId>,
) -> Option<Contact> {
    let Constraint::TangentLineCircle { line, round, sense } = *c else {
        return None;
    };
    let Entity::Line { a, b } = *sk.entity(line.id())? else {
        return None;
    };
    let centre = match *sk.entity(round.id())? {
        Entity::Circle { c, .. } | Entity::Arc { c, .. } => c,
        Entity::Point { .. } | Entity::Line { .. } => return None,
    };
    let mut ends = [a, b];
    ends.sort_by_key(|p| p.id());
    for point in ends {
        let implicit = match *sk.entity(round.id())? {
            Entity::Arc { s, .. } if point == s => Some(Membership::ArcStart(round.id())),
            Entity::Arc { e, .. } if point == e => Some(Membership::ArcEnd(round.id())),
            Entity::Point { .. }
            | Entity::Line { .. }
            | Entity::Circle { .. }
            | Entity::Arc { .. } => None,
        };
        let explicit = on_round
            .get(&(point.id(), round.id()))
            .map(|id| Membership::Explicit(*id));
        if let Some(membership) = implicit.or(explicit) {
            return Some(Contact {
                point,
                centre,
                a,
                b,
                membership,
                sense: sense.sign(),
            });
        }
    }
    None
}

impl Contact {
    fn evaluate(&self, values: &Coordinates, tolerance: f64) -> (f64, BTreeMap<Param, f64>, bool) {
        let xy = |p: Point| {
            [
                values
                    .get(&(p.id(), Coordinate::X))
                    .copied()
                    .unwrap_or(f64::NAN),
                values
                    .get(&(p.id(), Coordinate::Y))
                    .copied()
                    .unwrap_or(f64::NAN),
            ]
        };
        let a = xy(self.a);
        let b = xy(self.b);
        let p = xy(self.point);
        let c = xy(self.centre);
        let u = [b[0] - a[0], b[1] - a[1]];
        let v = [p[0] - c[0], p[1] - c[1]];
        let len = libm::hypot(u[0], u[1]);
        let radius = libm::hypot(v[0], v[1]);
        let t = [u[0] / len, u[1] / len];
        let h = t[0] * v[0] + t[1] * v[1];
        let w = [(v[0] - t[0] * h) / len, (v[1] - t[1] * h) / len];
        let mut terms = BTreeMap::new();
        for (point, g) in [
            (self.a, [-w[0], -w[1]]),
            (self.b, w),
            (self.point, t),
            (self.centre, [-t[0], -t[1]]),
        ] {
            for (coordinate, g) in [Coordinate::X, Coordinate::Y].into_iter().zip(g) {
                *terms.entry((point.id(), coordinate)).or_default() += g;
            }
        }
        (
            h,
            terms,
            len > tolerance && radius > tolerance && self.sense * (t[1] * v[0] - t[0] * v[1]) > 0.0,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_fixed_isolated_and_zero_derivatives() {
        assert!(System::new(&Sketch::new()).components.is_empty());
        let mut sk = Sketch::new();
        let a = sk.point_fixed(0.0, 0.0).unwrap();
        let b = sk.point_fixed(3.0, 4.0).unwrap();
        sk.distance(a, b, 5.0).unwrap();
        let p = sk.point(0.0, 0.0).unwrap();
        let q = sk.point(0.0, 0.0).unwrap();
        sk.distance(p, q, 2.0).unwrap();
        sk.point(20.0, 30.0).unwrap();
        let sys = System::new(&sk);
        assert_eq!(
            sys.components
                .iter()
                .map(|c| c.params.len())
                .collect::<Vec<_>>(),
            [4, 1, 1, 0]
        );
        assert!(
            sys.evaluate(sys.components.last().unwrap(), &sk.coordinates())
                .satisfied
        );
        assert!(!sys.evaluate(&sys.components[0], &sk.coordinates()).valid);
    }
    #[test]
    fn contact_provenance_and_removal() {
        let mut sk = Sketch::new();
        let c = sk.point_fixed(0.0, 1.0).unwrap();
        let p = sk.point(0.0, 0.0).unwrap();
        let q = sk.point(2.0, 0.0).unwrap();
        let l = sk.line(p, q).unwrap();
        let circle = sk.circle(c, 1.0).unwrap();
        let member = sk.point_on_circle(p, circle).unwrap();
        sk.tangent_line_circle(l, circle).unwrap();
        let sys = System::new(&sk);
        assert_eq!(
            sys.equations
                .last()
                .unwrap()
                .contact
                .as_ref()
                .unwrap()
                .membership,
            Membership::Explicit(member)
        );
        assert!(
            sys.components
                .iter()
                .all(|c| sys.evaluate(c, &sk.coordinates()).satisfied)
        );
        sk.remove_constraint(member).unwrap();
        assert!(System::new(&sk).equations.last().unwrap().contact.is_none());
    }
    #[test]
    fn shared_arc_endpoints_and_contact_gradient() {
        for end in [false, true] {
            let mut sk = Sketch::new();
            let c = sk.point(0.0, 1.0).unwrap();
            let p = sk.point(0.0, 0.0).unwrap();
            let other = sk.point(0.0, 2.0).unwrap();
            let q = sk.point(2.0, 0.0).unwrap();
            let arc = if end {
                sk.arc(c, other, p)
            } else {
                sk.arc(c, p, other)
            }
            .unwrap();
            let line = sk.line(p, q).unwrap();
            sk.tangent_line_circle(line, arc).unwrap();
            let sys = System::new(&sk);
            assert_eq!(sys.components.len(), 1);
            let contact = sys.equations.last().unwrap().contact.as_ref().unwrap();
            assert_eq!(
                contact.membership,
                if end {
                    Membership::ArcEnd(arc.id())
                } else {
                    Membership::ArcStart(arc.id())
                }
            );
            let values = sk.coordinates();
            let (_, g, valid) = contact.evaluate(&values, tolerance::LENGTH_FLOOR);
            assert!(valid);
            // Central finite differences, independently perturb every coordinate.
            const STEP: f64 = 1e-5;
            const AGREEMENT: f64 = 1e-9;
            for (key, gradient) in g {
                let mut plus = values.clone();
                let mut minus = values.clone();
                *plus.get_mut(&key).unwrap() += STEP;
                *minus.get_mut(&key).unwrap() -= STEP;
                let fd = (contact.evaluate(&plus, tolerance::LENGTH_FLOOR).0
                    - contact.evaluate(&minus, tolerance::LENGTH_FLOOR).0)
                    / (2.0 * STEP);
                assert!((fd - gradient).abs() < AGREEMENT);
            }
        }
    }
}
