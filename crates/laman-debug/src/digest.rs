//! The digest: SHA-256 over a run's bits (`docs/ARCHITECTURE.md`
//! §Determinism). Two targets that agree on the digest agree on every bit
//! it covers.
//!
//! The byte stream is fixed and written by [`Builder`] alone:
//!
//! - a `u32` is 4 bytes little-endian, a `u64` is 8;
//! - a float is the 8 little-endian bytes of its IEEE bits, so `-0.0` and
//!   `0.0` differ;
//! - text and other bytes are a `u64` length, then the bytes, so
//!   `("ab", "c")` and `("a", "bc")` differ;
//! - an optional text is a flag byte (0 or 1), then the text when present.
//!
//! A run is its sketch (as the serde form's text, whose floats print to the
//! shortest text that reads back to the same bits) and the checker's report
//! at the checked coordinates ([`of_run`]); a solved run adds the
//! `Solution`: coordinates, status and every diagnostic ([`of_solution`]).
//! A profile run adds the coordinates it was extracted at (or the solved
//! run's solution), the extracted [`Profiles`] and the profile check's report
//! ([`of_profiles`], [`of_solved_profiles`]); [`of_run`] and [`of_solution`]
//! are untouched by them, so no digest of a sketch without profiles moves.

use crate::fixture::coordinate_name;
use crate::json::sketch_to_string;
use laman::model::profile::{Loop, Profiles};
use laman::model::{Coordinates, Sketch};
use laman::solution::{ConstraintRef, EntityRef, FlipSubject, Origin, Solution};
use laman_check::{Degeneracy, ProfileReport, ProfileViolation, Report, Source, Subject};
use sha2::{Digest as _, Sha256};
use std::fmt;

/// A SHA-256 digest of a run.
///
/// ```
/// use laman_debug::digest::Builder;
/// let digest = Builder::new().finish();
/// assert_eq!(digest.to_string().len(), 64);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Digest([u8; 32]);

impl Digest {
    /// The 32 bytes of the digest.
    ///
    /// ```
    /// use laman_debug::digest::Builder;
    /// assert_eq!(Builder::new().finish().as_bytes().len(), 32);
    /// ```
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for Digest {
    /// Lower-case hexadecimal, 64 characters: the form `expected.json` holds.
    ///
    /// ```
    /// use laman_debug::digest::Builder;
    /// assert!(Builder::new().finish().to_string().chars().all(|c| c.is_ascii_hexdigit()));
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Feeds values into a digest in the fixed byte layout of this module.
///
/// ```
/// use laman_debug::digest::Builder;
/// let mut a = Builder::new();
/// a.text("ab");
/// a.text("c");
/// let mut b = Builder::new();
/// b.text("a");
/// b.text("bc");
/// assert_ne!(a.finish(), b.finish());
/// ```
#[derive(Default)]
pub struct Builder {
    hash: Sha256,
}

impl Builder {
    /// An empty builder.
    ///
    /// ```
    /// use laman_debug::digest::Builder;
    /// let _ = Builder::new().finish();
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds a `u32` as 4 little-endian bytes.
    ///
    /// ```
    /// use laman_debug::digest::Builder;
    /// let mut b = Builder::new();
    /// b.u32(1);
    /// let _ = b.finish();
    /// ```
    pub fn u32(&mut self, value: u32) {
        self.hash.update(value.to_le_bytes());
    }

    /// Feeds a `u64` as 8 little-endian bytes.
    ///
    /// ```
    /// use laman_debug::digest::Builder;
    /// let mut b = Builder::new();
    /// b.u64(1);
    /// let _ = b.finish();
    /// ```
    pub fn u64(&mut self, value: u64) {
        self.hash.update(value.to_le_bytes());
    }

    /// Feeds an `f64` by its IEEE bits, so `-0.0` and `0.0` differ.
    ///
    /// ```
    /// use laman_debug::digest::Builder;
    /// let (mut a, mut b) = (Builder::new(), Builder::new());
    /// a.float(-0.0);
    /// b.float(0.0);
    /// assert_ne!(a.finish(), b.finish());
    /// ```
    pub fn float(&mut self, value: f64) {
        self.u64(value.to_bits());
    }

    /// Feeds text: its length as a `u64`, then its UTF-8 bytes.
    ///
    /// ```
    /// use laman_debug::digest::Builder;
    /// let mut b = Builder::new();
    /// b.text("sss-triangle");
    /// let _ = b.finish();
    /// ```
    pub fn text(&mut self, value: &str) {
        self.u64(len_of(value.len()));
        self.hash.update(value.as_bytes());
    }

    /// Feeds an optional text: a flag byte, then the text when present.
    ///
    /// ```
    /// use laman_debug::digest::Builder;
    /// let mut b = Builder::new();
    /// b.optional_text(None);
    /// b.optional_text(Some("apex"));
    /// let _ = b.finish();
    /// ```
    pub fn optional_text(&mut self, value: Option<&str>) {
        match value {
            None => self.hash.update([0_u8]),
            Some(text) => {
                self.hash.update([1_u8]);
                self.text(text);
            }
        }
    }

    /// The digest of everything fed so far.
    ///
    /// ```
    /// use laman_debug::digest::Builder;
    /// let _ = Builder::new().finish();
    /// ```
    pub fn finish(self) -> Digest {
        Digest(self.hash.finalize().into())
    }
}

/// The digest of a run: the loaded sketch, then the checker's report at the
/// coordinates the run was checked at. Equal sketches and reports give equal
/// digests, and any difference in a covered bit changes it.
///
/// ```
/// use laman::model::Sketch;
/// use laman_check::check;
/// use laman_debug::digest::of_run;
///
/// let mut sk = Sketch::new();
/// let a = sk.point(0.0, 0.0)?;
/// let b = sk.point(3.0, 4.0)?;
/// sk.distance(a, b, 5.0)?;
/// let report = check(&sk, &sk.coordinates());
/// assert_eq!(of_run(&sk, &report), of_run(&sk, &report));
/// # Ok::<(), laman::model::ModelError>(())
/// ```
pub fn of_run(sketch: &Sketch, report: &Report) -> Digest {
    let mut b = Builder::new();
    b.text(&sketch_to_string(sketch));
    feed_report(&mut b, report);
    b.finish()
}

/// The digest of a solved run: the sketch, the solution (coordinates in id
/// order, status, every diagnostic in the order the solution lists them)
/// and the checker's report at the solved coordinates. Equal runs give
/// equal digests; any difference in a covered bit changes it.
///
/// ```
/// use laman::model::Sketch;
/// use laman::solution::SolverOptions;
/// use laman_check::check;
/// use laman_debug::digest::of_solution;
///
/// let mut sk = Sketch::new();
/// let a = sk.point_fixed(0.0, 0.0)?;
/// let b = sk.point(3.0, 1.0)?;
/// sk.distance(a, b, 5.0)?;
/// let solution = sk.solve(&SolverOptions::default());
/// let report = check(&sk, &solution.coordinates);
/// assert_eq!(of_solution(&sk, &solution, &report), of_solution(&sk, &solution, &report));
/// # Ok::<(), laman::model::ModelError>(())
/// ```
pub fn of_solution(sketch: &Sketch, solution: &Solution, report: &Report) -> Digest {
    let mut b = Builder::new();
    feed_solution(&mut b, sketch, solution, report);
    b.finish()
}

/// The digest of a profile run at given coordinates: the sketch, the
/// coordinates in id order, what extraction found there and the profile
/// check's report. Any difference in a covered bit changes it.
///
/// ```
/// use laman::model::Sketch;
/// use laman::profile::profiles;
/// use laman_check::check_profiles;
/// use laman_debug::digest::of_profiles;
///
/// let mut sk = Sketch::new();
/// let c = sk.point(0.0, 0.0)?;
/// sk.circle(c, 2.0)?;
/// let values = sk.coordinates();
/// let found = profiles(&sk, &values).unwrap_or_default();
/// let report = check_profiles(&sk, &values, &found);
/// assert_eq!(
///     of_profiles(&sk, &values, &found, &report),
///     of_profiles(&sk, &values, &found, &report)
/// );
/// # Ok::<(), laman::model::ModelError>(())
/// ```
pub fn of_profiles(
    sketch: &Sketch,
    values: &Coordinates,
    found: &Profiles,
    report: &ProfileReport,
) -> Digest {
    let mut b = Builder::new();
    b.text(&sketch_to_string(sketch));
    b.u64(len_of(values.len()));
    for (&(id, c), &value) in values {
        b.u32(id.0);
        b.text(coordinate_name(c));
        b.float(value);
    }
    feed_profiles(&mut b, found, report);
    b.finish()
}

/// The digest of a solved profile run: what [`of_solution`] covers (the
/// sketch, the solution and the checker's `report`), then the profiles
/// extracted at the solution and the profile check's report.
///
/// ```
/// use laman::model::Sketch;
/// use laman::profile::profiles;
/// use laman::solution::SolverOptions;
/// use laman_check::{check, check_profiles};
/// use laman_debug::digest::of_solved_profiles;
///
/// let mut sk = Sketch::new();
/// let c = sk.point_fixed(0.0, 0.0)?;
/// sk.circle(c, 2.0)?;
/// let solution = sk.solve(&SolverOptions::default());
/// let report = check(&sk, &solution.coordinates);
/// let found = profiles(&sk, &solution.coordinates).unwrap_or_default();
/// let profile_report = check_profiles(&sk, &solution.coordinates, &found);
/// let a = of_solved_profiles(&sk, &solution, &report, &found, &profile_report);
/// assert_eq!(a, of_solved_profiles(&sk, &solution, &report, &found, &profile_report));
/// # Ok::<(), laman::model::ModelError>(())
/// ```
pub fn of_solved_profiles(
    sketch: &Sketch,
    solution: &Solution,
    report: &Report,
    found: &Profiles,
    profile_report: &ProfileReport,
) -> Digest {
    let mut b = Builder::new();
    feed_solution(&mut b, sketch, solution, report);
    feed_profiles(&mut b, found, profile_report);
    b.finish()
}

fn feed_profiles(b: &mut Builder, found: &Profiles, report: &ProfileReport) {
    b.u64(len_of(found.profiles.len()));
    for p in &found.profiles {
        feed_loop(b, &p.outer);
        b.u64(len_of(p.holes.len()));
        for hole in &p.holes {
            feed_loop(b, hole);
        }
        b.float(p.net_area);
    }
    b.u64(len_of(found.open_chains.len()));
    for chain in &found.open_chains {
        b.u64(len_of(chain.len()));
        for e in chain {
            b.u32(e.0);
        }
    }
    b.u64(len_of(found.dangling.len()));
    for e in &found.dangling {
        b.u32(e.0);
    }
    b.u64(len_of(found.branched.len()));
    for core in &found.branched {
        b.u64(len_of(core.edges.len()));
        for e in &core.edges {
            b.u32(e.0);
        }
        b.u64(len_of(core.joins.len()));
        for j in &core.joins {
            b.u32(j.0);
        }
    }
    b.u64(len_of(found.degenerate.len()));
    for e in &found.degenerate {
        b.u32(e.0);
    }
    b.u64(len_of(found.unjoined.len()));
    for u in &found.unjoined {
        b.u32(u.first.0);
        b.u32(u.second.0);
    }

    b.u64(len_of(report.violations.len()));
    for v in &report.violations {
        // A loop's first edge: a flag byte, then the id when it has one.
        let first = |b: &mut Builder, first: &Option<laman::model::EntityId>| match first {
            None => b.optional_text(None),
            Some(id) => b.optional_text(Some(&id.0.to_string())),
        };
        match v {
            ProfileViolation::Area {
                first: f,
                measured,
                reported,
            } => {
                b.text("area");
                first(b, f);
                b.float(*measured);
                b.float(*reported);
            }
            ProfileViolation::Orientation { first: f } => {
                b.text("orientation");
                first(b, f);
            }
            ProfileViolation::NotChained { first: f } => {
                b.text("not_chained");
                first(b, f);
            }
            ProfileViolation::OpenJoin { first: f } => {
                b.text("open_join");
                first(b, f);
            }
            ProfileViolation::ArcOffRadius { first: f } => {
                b.text("arc_off_radius");
                first(b, f);
            }
            ProfileViolation::NotAnEdge { entity } => {
                b.text("not_an_edge");
                b.u32(entity.0);
            }
            ProfileViolation::Repeated { entity } => {
                b.text("repeated");
                b.u32(entity.0);
            }
            ProfileViolation::Uncovered { entity } => {
                b.text("uncovered");
                b.u32(entity.0);
            }
            ProfileViolation::Containment { first: f } => {
                b.text("containment");
                first(b, f);
            }
        }
    }
    b.u64(len_of(report.missing.len()));
    for m in &report.missing {
        b.u32(m.id.0);
        match m.coordinate {
            None => b.optional_text(None),
            Some(c) => b.optional_text(Some(coordinate_name(c))),
        }
    }
}

fn feed_loop(b: &mut Builder, l: &Loop) {
    b.u64(len_of(l.edges.len()));
    for t in &l.edges {
        b.u32(t.entity.0);
        b.u32(u32::from(t.reversed));
    }
    b.float(l.signed_area);
    b.u64(len_of(l.joins.len()));
    for j in &l.joins {
        b.u32(j.0);
    }
}

/// The digest of one drag step: the step's index (1-based) and the bits of
/// its target, then what [`of_solution`] covers, over the step's solution.
///
/// ```
/// use laman::model::Sketch;
/// use laman::solution::SolverOptions;
/// use laman_check::check;
/// use laman_debug::digest::of_drag_step;
///
/// let mut sk = Sketch::new();
/// let a = sk.point_fixed(0.0, 0.0)?;
/// let b = sk.point(3.0, 4.0)?;
/// sk.distance(a, b, 5.0)?;
/// let mut drag = sk.drag(b.id(), &SolverOptions::default())?;
/// let step = drag.to([0.0, 9.0]);
/// let report = check(&sk, &step.coordinates);
/// let digest = of_drag_step(&sk, 1, [0.0, 9.0], &step, &report);
/// assert_ne!(digest, of_drag_step(&sk, 2, [0.0, 9.0], &step, &report));
/// # Ok::<(), laman::model::ModelError>(())
/// ```
pub fn of_drag_step(
    sketch: &Sketch,
    index: usize,
    target: [f64; 2],
    solution: &Solution,
    report: &Report,
) -> Digest {
    let mut b = Builder::new();
    b.u64(len_of(index));
    b.float(target[0]);
    b.float(target[1]);
    feed_solution(&mut b, sketch, solution, report);
    b.finish()
}

fn feed_solution(b: &mut Builder, sketch: &Sketch, solution: &Solution, report: &Report) {
    b.text(&sketch_to_string(sketch));
    b.text(&solution.status.to_string());
    b.u64(len_of(solution.coordinates.len()));
    for (&(id, c), &value) in &solution.coordinates {
        b.u32(id.0);
        b.text(coordinate_name(c));
        b.float(value);
    }
    let d = &solution.diagnostics;
    b.u64(len_of(d.dof));
    b.u64(len_of(d.entities.len()));
    for e in &d.entities {
        entity_ref(b, &e.entity);
        b.u64(len_of(e.dof));
    }
    b.u64(len_of(d.free_directions.len()));
    for direction in &d.free_directions {
        b.u64(len_of(direction.len()));
        for m in direction {
            b.u32(m.entity.0);
            b.text(coordinate_name(m.coordinate));
            b.float(m.amount);
        }
    }
    b.u64(len_of(d.redundant.len()));
    for c in &d.redundant {
        constraint_ref(b, c);
    }
    b.u64(len_of(d.conflicts.len()));
    for set in &d.conflicts {
        b.u64(len_of(set.len()));
        for c in set {
            constraint_ref(b, c);
        }
    }
    b.u64(len_of(d.degenerate.len()));
    for g in &d.degenerate {
        entity_ref(b, &g.entity);
        b.text(&format!("{:?}", g.collapse));
        b.float(g.size);
        origins(b, &g.equations);
    }
    origins(b, &d.ill_conditioned);
    b.u64(len_of(d.flips.len()));
    for f in &d.flips {
        match &f.subject {
            FlipSubject::Entity(e) => {
                b.u32(0);
                entity_ref(b, e);
            }
            FlipSubject::Constraint(c) => {
                b.u32(1);
                constraint_ref(b, c);
            }
        }
        b.float(f.was);
        b.float(f.now);
    }
    // Only a bisected frame has one, so every other digest is unchanged.
    if let Some(bisection) = &d.bisection {
        b.text("bisection");
        b.float(bisection.reached[0]);
        b.float(bisection.reached[1]);
        b.u64(len_of(bisection.halvings));
    }
    feed_report(b, report);
}

fn entity_ref(b: &mut Builder, e: &EntityRef) {
    b.u32(e.id.0);
    b.text(&e.kind.to_string());
    b.optional_text(e.tag.as_deref());
}

fn constraint_ref(b: &mut Builder, c: &ConstraintRef) {
    b.u32(c.id.0);
    b.text(&c.kind.to_string());
    b.optional_text(c.tag.as_deref());
}

fn origins(b: &mut Builder, list: &[Origin]) {
    b.u64(len_of(list.len()));
    for o in list {
        match o {
            Origin::Constraint(c) => {
                b.u32(0);
                constraint_ref(b, c);
            }
            Origin::Arc(e) => {
                b.u32(1);
                entity_ref(b, e);
            }
        }
    }
}

fn feed_report(b: &mut Builder, report: &Report) {
    b.u64(len_of(report.violations.len()));
    for v in &report.violations {
        match v.id {
            Source::Constraint(id) => {
                b.u32(0);
                b.u32(id.0);
            }
            Source::Arc(id) => {
                b.u32(1);
                b.u32(id.0);
            }
        }
        b.optional_text(v.tag.as_deref());
        b.text(&subject_name(v.kind));
        b.float(v.measured);
        b.float(v.wanted);
    }

    b.u64(len_of(report.degenerate.len()));
    for d in &report.degenerate {
        b.u32(d.id.0);
        b.optional_text(d.tag.as_deref());
        b.text(&d.kind.to_string());
        b.text(degeneracy_name(d.what));
        b.float(d.size);
    }

    b.u64(len_of(report.missing.len()));
    for m in &report.missing {
        b.u32(m.id.0);
        match m.coordinate {
            None => b.optional_text(None),
            Some(c) => b.optional_text(Some(coordinate_name(c))),
        }
    }
}

fn len_of(n: usize) -> u64 {
    // A length past u64 is unreachable on any target; saturating keeps the
    // function total without a panic.
    u64::try_from(n).unwrap_or(u64::MAX)
}

fn subject_name(subject: Subject) -> String {
    match subject {
        Subject::Constraint(kind) => kind.to_string(),
        Subject::ArcIntrinsic => "arc_intrinsic".to_owned(),
    }
}

fn degeneracy_name(what: Degeneracy) -> &'static str {
    match what {
        Degeneracy::ZeroLengthLine => "zero_length_line",
        Degeneracy::ZeroRadius => "zero_radius",
        Degeneracy::ZeroRadiusArc => "zero_radius_arc",
        Degeneracy::EmptyArc => "empty_arc",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The layout above, computed outside this crate: the bytes are
    /// `u32 7`, `float 1.5`, `text "ab"`, `optional_text Some("")`, hashed
    /// with `sha256sum`. A change to the layout moves this digest.
    #[test]
    fn pinned_test_vector() {
        let mut b = Builder::new();
        b.u32(7);
        b.float(1.5);
        b.text("ab");
        b.optional_text(Some(""));
        assert_eq!(
            b.finish().to_string(),
            "aec34e774dafd4cb7aaeab66536174ab778d6baaa88b7b0eb4432ef01a7caf88"
        );
    }
}
