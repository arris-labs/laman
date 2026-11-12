//! The fixture loader: a fixture directory as its sketch, its expected
//! answers and, for a drag, the drag (`docs/ROADMAP.md` §Fixtures).
//!
//! A fixture directory holds `sketch.json` (the serde form, whose
//! coordinates are the guesses), `expected.json`, and `drag.json` for a
//! drag. The two expectation files are JSON with unknown keys refused, so a
//! misspelt key is an error naming the file, never a silently ignored field.
//!
//! `expected.json`:
//!
//! ```json
//! {
//!   "status": "FullyConstrained",
//!   "dof": 0,
//!   "coordinates": [{"id": 1, "coordinate": "x", "value": 50.0}],
//!   "redundant": ["right-angle"],
//!   "conflicts": [["base", "right", "apex"]],
//!   "flips": ["slot-arc"],
//!   "tolerance": 1e-9,
//!   "digest": "aec3…",
//!   "profiles": {
//!     "profiles": [{
//!       "outer": {"edges": [{"entity": 4, "reversed": false}],
//!                 "signed_area": {"value": 7200.0}, "joins": []},
//!       "holes": [], "net_area": {"value": 7200.0, "tolerance": 1e-9}
//!     }],
//!     "open_chains": [[7, 8]], "dangling": [9], "degenerate": [],
//!     "branched": [{"edges": [4, 5, 6], "joins": [1]}],
//!     "unjoined": [[2, 9]]
//!   },
//!   "intersections": {
//!     "pairs": [{"first": 4, "second": 5, "meetings": [
//!       {"kind": "crossing", "at": [4.0, 2.0], "on_first": 0.4, "on_second": 0.5},
//!       {"kind": "overlap", "at": [6.0, 0.0], "on_first": 0.6, "on_second": 0.0,
//!        "to": [10.0, 0.0], "to_first": 1.0, "to_second": 0.4}
//!     ]}]
//!   }
//! }
//! ```
//!
//! `status`, `redundant`, `conflicts`, `flips` and `digest` are read as
//! written; the solver compares them from M2 on. `flips` (default none) are
//! the tags of the entities and constraints that change branch. `tolerance` defaults to
//! `laman::model::tolerance::LENGTH`. Every coordinate names a parameter the
//! sketch has, or the file is refused with its id.
//!
//! `profiles` (optional) is what `laman::profile::profiles` must return
//! (ADR-0012): every list of `Profiles`, by the caller's ids and in the
//! order the result documents; each area is `{"value", "tolerance"}`, the
//! tolerance relative to `max(1, |value|)` and defaulting to
//! [`AREA_TOLERANCE`]. Every entity and constraint id named must exist in the
//! sketch, or the file is refused with it. [`ExpectedProfiles::mismatches`]
//! compares it with a result.
//!
//! `intersections` (optional) is what `laman::intersect::intersections` must
//! return (ADR-0016): the pairs that meet, by the caller's ids, in the order
//! the result documents, each meeting's kind (`crossing`, `touch` or
//! `overlap`), point and parameters, and for an overlap its other end `to`,
//! `to_first` and `to_second` (given for no other kind). A pair left out
//! must not meet. Points are compared to `tolerance` (default
//! [`INTERSECTION_TOLERANCE`]) relative to `max(1, |coordinate|)`, parameters
//! to `tolerance` absolutely. Every entity id named must exist in the sketch,
//! or the file is refused with it. [`ExpectedIntersections::mismatches`]
//! compares it with a result.
//!
//! `drag.json` names the dragged point, its targets in order (`[x, y]`), and
//! the expected coordinates after chosen steps (1-based, into `targets`),
//! and for each the tags it names as flipped (`flips`, default none) and how
//! many times it is bisected (`halvings`, default none: solved at its own
//! target).

use crate::json::{LoadError, sketch_from_str};
use laman::model::intersection::{Intersections, MeetingKind};
use laman::model::profile::{Branched, Profiles, Traversal, Unjoined};
use laman::model::{
    ConstraintId, Coordinate, Coordinates, EntityId, EntityKind, Sketch, tolerance,
};
use serde::Deserialize;
use std::io;
use std::path::{Path, PathBuf};

/// The sketch file of a fixture directory.
pub const SKETCH_FILE: &str = "sketch.json";
/// The expectations file of a fixture directory.
pub const EXPECTED_FILE: &str = "expected.json";
/// The drag file of a fixture directory, present only for a drag.
pub const DRAG_FILE: &str = "drag.json";
/// How close an area must be when its expectation names no tolerance,
/// relative to `max(1, |value|)`. Areas are sums of products of lengths, so
/// they carry the relative rounding of those lengths, not an absolute one;
/// 1e-9 is the extraction's own bound at the corpus's scales
/// (`plans/m9-profiles`).
pub const AREA_TOLERANCE: f64 = 1e-9;

/// How close a point and a parameter must be when an `intersections`
/// expectation names no tolerance: the points relative to `max(1, |value|)`
/// and the parameters absolutely. The closed forms are exact to a few ulps
/// and the default length tolerance is 1e-9 (ADR-0004), so a meeting that
/// differs by more is a different meeting (`plans/m12-intersections`).
pub const INTERSECTION_TOLERANCE: f64 = 1e-9;

/// Why a fixture could not be loaded. Each variant names the file, and where
/// it names an entity, its id.
#[derive(Debug, thiserror::Error)]
pub enum FixtureError {
    /// A file could not be read.
    #[error("cannot read {}: {source}", path.display())]
    Read {
        /// The file.
        path: PathBuf,
        /// Why.
        #[source]
        source: io::Error,
    },
    /// A file is not JSON of its expected shape.
    #[error("{file}: {source}")]
    Json {
        /// The file's name within the fixture.
        file: &'static str,
        /// Why.
        #[source]
        source: serde_json::Error,
    },
    /// The sketch file is not a sketch the model accepts.
    #[error("{file}: {source}")]
    Sketch {
        /// The file's name within the fixture.
        file: &'static str,
        /// Why.
        #[source]
        source: LoadError,
    },
    /// A coordinate names an entity, or a parameter of an entity, the sketch
    /// does not have.
    #[error("{file}: entity {id} has no {coordinate:?} parameter in the sketch")]
    UnknownCoordinate {
        /// The file's name within the fixture.
        file: &'static str,
        /// The entity id as written.
        id: EntityId,
        /// The parameter as written.
        coordinate: Coordinate,
    },
    /// A `redundant` or `conflicts` entry is a tag no constraint of the
    /// sketch carries.
    #[error("{file}: tag {tag:?} names no constraint of the sketch")]
    UnknownTag {
        /// The file's name within the fixture.
        file: &'static str,
        /// The tag as written.
        tag: String,
    },
    /// The tolerance is not a positive finite number.
    #[error("expected.json: tolerance {value} is not a positive finite number")]
    Tolerance {
        /// The value as written.
        value: f64,
    },
    /// The `profiles` section names an entity the sketch does not have.
    #[error("expected.json: profiles name entity {id}, which the sketch does not have")]
    UnknownEntity {
        /// The id as written.
        id: EntityId,
    },
    /// The `profiles` section names a constraint the sketch does not have.
    #[error("expected.json: profiles name constraint {id}, which the sketch does not have")]
    UnknownConstraint {
        /// The id as written.
        id: ConstraintId,
    },
    /// An area tolerance is not a positive finite number, or a value is not
    /// finite.
    #[error("expected.json: profiles area {value} with tolerance {tolerance} is not usable")]
    Area {
        /// The value as written.
        value: f64,
        /// The tolerance as written or defaulted.
        tolerance: f64,
    },
    /// The `intersections` section names an entity the sketch does not have.
    #[error("expected.json: intersections name entity {id}, which the sketch does not have")]
    UnknownIntersectionEntity {
        /// The id as written.
        id: EntityId,
    },
    /// An `intersections` meeting is not usable: an unknown kind, an overlap
    /// without its other end, another kind with one, a value that is not
    /// finite, or a tolerance that is not positive and finite.
    #[error("expected.json: intersections: {what}")]
    Intersections {
        /// What is wrong.
        what: String,
    },
    /// The dragged entity is not a point of the sketch.
    #[error("drag.json: entity {id} is not a point of the sketch")]
    NotAPoint {
        /// The id as written.
        id: EntityId,
    },
    /// A step names a target the drag does not have.
    #[error("drag.json: step {step} is outside the {targets} targets")]
    StepOutOfRange {
        /// The step as written.
        step: usize,
        /// How many targets there are.
        targets: usize,
    },
}

/// A fixture: the sketch, what is expected of it, and its drag if any.
///
/// ```
/// use laman_debug::fixture::Fixture;
/// let fixture = Fixture::parse(
///     r#"{"format":"laman.sketch","version":1,"entities":[{"id":0,"kind":"point","x":1.0,"y":2.0}]}"#,
///     r#"{"status":"UnderConstrained","dof":2,"coordinates":[{"id":0,"coordinate":"x","value":1.0}]}"#,
///     None,
/// )?;
/// assert_eq!(fixture.expected.dof, 2);
/// # Ok::<(), laman_debug::fixture::FixtureError>(())
/// ```
#[derive(Debug)]
pub struct Fixture {
    /// The sketch, its coordinates being the solve's guesses.
    pub sketch: Sketch,
    /// What the solve must return.
    pub expected: Expected,
    /// The drag, for a drag fixture.
    pub drag: Option<Drag>,
}

/// The expected answers of a fixture.
#[derive(Debug, Clone, PartialEq)]
pub struct Expected {
    /// The status name, as the solver reports it.
    pub status: String,
    /// The degrees of freedom.
    pub dof: u32,
    /// The expected coordinates of the free parameters. Parameters not
    /// listed keep the sketch's values.
    pub coordinates: Coordinates,
    /// How close a solved coordinate must be, in the sketch's units.
    pub tolerance: f64,
    /// The tags of the redundant constraints.
    pub redundant: Vec<String>,
    /// Each conflict set, as tags.
    pub conflicts: Vec<Vec<String>>,
    /// The tags of the entities and constraints that change branch (empty
    /// when the solve keeps every one).
    pub flips: Vec<String>,
    /// The blessed digest, as lower-case hex, when there is one.
    pub digest: Option<String>,
    /// What profile extraction must return, when the fixture says.
    pub profiles: Option<ExpectedProfiles>,
    /// What intersection measuring must return, when the fixture says.
    pub intersections: Option<ExpectedIntersections>,
}

/// An expected meeting: its kind, point and parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpectedMeeting {
    /// The kind, with an overlap's other end.
    pub kind: MeetingKind,
    /// The point.
    pub at: [f64; 2],
    /// The parameter on the first curve.
    pub on_first: f64,
    /// The parameter on the second curve.
    pub on_second: f64,
}

/// An expected pair and its meetings, in order.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpectedPair {
    /// The smaller id.
    pub first: EntityId,
    /// The larger id.
    pub second: EntityId,
    /// The meetings.
    pub meetings: Vec<ExpectedMeeting>,
}

/// What `laman::intersect::intersections` must return for a fixture.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpectedIntersections {
    /// The pairs, in order; a pair that is not here must not meet.
    pub pairs: Vec<ExpectedPair>,
    /// How close a point (relative to `max(1, |coordinate|)`) and a
    /// parameter (absolutely) must be.
    pub tolerance: f64,
}

impl ExpectedIntersections {
    /// What differs between these expectations and a result, one line per
    /// difference naming the pair and the meeting; empty when they agree.
    ///
    /// ```
    /// use laman::model::intersection::Intersections;
    /// use laman_debug::fixture::{ExpectedIntersections, INTERSECTION_TOLERANCE};
    /// let none = ExpectedIntersections { pairs: vec![], tolerance: INTERSECTION_TOLERANCE };
    /// assert!(none.mismatches(&Intersections::default()).is_empty());
    /// ```
    pub fn mismatches(&self, found: &Intersections) -> Vec<String> {
        let mut out = Vec::new();
        let tol = self.tolerance;
        let point = |a: [f64; 2], b: [f64; 2]| {
            a.iter()
                .zip(&b)
                .all(|(x, y)| (x - y).abs() <= tol * y.abs().max(1.0))
        };
        let param = |a: f64, b: f64| (a - b).abs() <= tol;
        let ids = |pairs: &mut dyn Iterator<Item = (EntityId, EntityId)>| {
            pairs.map(|(a, b)| format!("{a}-{b}")).collect::<Vec<_>>()
        };
        let want = ids(&mut self.pairs.iter().map(|p| (p.first, p.second)));
        let got = ids(&mut found.pairs.iter().map(|p| (p.first, p.second)));
        if want != got {
            out.push(format!("pairs: expected {want:?}, found {got:?}"));
            return out;
        }
        for (w, g) in self.pairs.iter().zip(&found.pairs) {
            let name = format!("pair {}-{}", w.first, w.second);
            if w.meetings.len() != g.meetings.len() {
                out.push(format!(
                    "{name}: expected {} meetings, found {}",
                    w.meetings.len(),
                    g.meetings.len()
                ));
                continue;
            }
            for (k, (m, f)) in w.meetings.iter().zip(&g.meetings).enumerate() {
                let same_kind = match (m.kind, f.kind) {
                    (MeetingKind::Crossing, MeetingKind::Crossing)
                    | (MeetingKind::Touch, MeetingKind::Touch) => true,
                    (
                        MeetingKind::Overlap {
                            to,
                            to_first,
                            to_second,
                        },
                        MeetingKind::Overlap {
                            to: gt,
                            to_first: gf,
                            to_second: gs,
                        },
                    ) => point(gt, to) && param(gf, to_first) && param(gs, to_second),
                    (
                        MeetingKind::Crossing | MeetingKind::Touch | MeetingKind::Overlap { .. },
                        MeetingKind::Crossing | MeetingKind::Touch | MeetingKind::Overlap { .. },
                    ) => false,
                };
                if !(same_kind
                    && point(f.at, m.at)
                    && param(f.on_first, m.on_first)
                    && param(f.on_second, m.on_second))
                {
                    out.push(format!("{name} meeting {k}: expected {m:?}, found {f:?}"));
                }
            }
        }
        out
    }
}

/// An area and how close a result must be to it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExpectedArea {
    /// The area, in the sketch's units squared.
    pub value: f64,
    /// The tolerance, relative to `max(1, |value|)`.
    pub tolerance: f64,
}

impl ExpectedArea {
    fn holds(&self, found: f64) -> bool {
        (found - self.value).abs() <= self.tolerance * self.value.abs().max(1.0)
    }
}

/// An expected loop: its directed edges in order, its area and its joins.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpectedLoop {
    /// The edges, as `laman::model::profile::Loop::edges` lists them.
    pub edges: Vec<Traversal>,
    /// The signed area.
    pub signed_area: ExpectedArea,
    /// The joining `coincident` constraints, by id.
    pub joins: Vec<ConstraintId>,
}

/// An expected profile.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpectedProfile {
    /// The outer loop.
    pub outer: ExpectedLoop,
    /// The holes, in order.
    pub holes: Vec<ExpectedLoop>,
    /// The net area.
    pub net_area: ExpectedArea,
}

/// What `laman::profile::profiles` must return for a fixture.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpectedProfiles {
    /// The profiles, in order.
    pub profiles: Vec<ExpectedProfile>,
    /// The open chains, as `Profiles::open_chains`.
    pub open_chains: Vec<Vec<EntityId>>,
    /// The dangling edges.
    pub dangling: Vec<EntityId>,
    /// The branched cores.
    pub branched: Vec<Branched>,
    /// The self-joined edges.
    pub degenerate: Vec<EntityId>,
    /// The unjoined coincidences.
    pub unjoined: Vec<Unjoined>,
}

impl ExpectedProfiles {
    /// What differs between these expectations and a result, one line per
    /// difference naming the profile and the field; empty when they agree.
    ///
    /// ```
    /// use laman::model::profile::Profiles;
    /// use laman_debug::fixture::ExpectedProfiles;
    /// let none = ExpectedProfiles {
    ///     profiles: vec![],
    ///     open_chains: vec![],
    ///     dangling: vec![],
    ///     branched: vec![],
    ///     degenerate: vec![],
    ///     unjoined: vec![],
    /// };
    /// assert!(none.mismatches(&Profiles::default()).is_empty());
    /// ```
    pub fn mismatches(&self, found: &Profiles) -> Vec<String> {
        let mut out = Vec::new();
        if self.profiles.len() != found.profiles.len() {
            out.push(format!(
                "profiles: expected {}, found {}",
                self.profiles.len(),
                found.profiles.len()
            ));
        }
        for (k, (want, got)) in self.profiles.iter().zip(&found.profiles).enumerate() {
            let name = format!("profile {k}");
            loop_mismatches(&mut out, &format!("{name} outer"), &want.outer, &got.outer);
            if want.holes.len() != got.holes.len() {
                out.push(format!(
                    "{name}: expected {} holes, found {}",
                    want.holes.len(),
                    got.holes.len()
                ));
            }
            for (h, (w, g)) in want.holes.iter().zip(&got.holes).enumerate() {
                loop_mismatches(&mut out, &format!("{name} hole {h}"), w, g);
            }
            if !want.net_area.holds(got.net_area) {
                out.push(format!(
                    "{name}: net area expected {}, found {}",
                    want.net_area.value, got.net_area
                ));
            }
        }
        let mut list = |what: &str, same: bool, want: String, got: String| {
            if !same {
                out.push(format!("{what}: expected {want}, found {got}"));
            }
        };
        list(
            "open_chains",
            self.open_chains == found.open_chains,
            format!("{:?}", self.open_chains),
            format!("{:?}", found.open_chains),
        );
        list(
            "dangling",
            self.dangling == found.dangling,
            format!("{:?}", self.dangling),
            format!("{:?}", found.dangling),
        );
        list(
            "branched",
            self.branched == found.branched,
            format!("{:?}", self.branched),
            format!("{:?}", found.branched),
        );
        list(
            "degenerate",
            self.degenerate == found.degenerate,
            format!("{:?}", self.degenerate),
            format!("{:?}", found.degenerate),
        );
        list(
            "unjoined",
            self.unjoined == found.unjoined,
            format!("{:?}", self.unjoined),
            format!("{:?}", found.unjoined),
        );
        out
    }
}

fn loop_mismatches(
    out: &mut Vec<String>,
    name: &str,
    want: &ExpectedLoop,
    got: &laman::model::profile::Loop,
) {
    if want.edges != got.edges {
        out.push(format!(
            "{name}: edges expected {:?}, found {:?}",
            want.edges, got.edges
        ));
    }
    if !want.signed_area.holds(got.signed_area) {
        out.push(format!(
            "{name}: area expected {}, found {}",
            want.signed_area.value, got.signed_area
        ));
    }
    if want.joins != got.joins {
        out.push(format!(
            "{name}: joins expected {:?}, found {:?}",
            want.joins, got.joins
        ));
    }
}

/// A drag: one point pulled through targets in order.
#[derive(Debug, Clone, PartialEq)]
pub struct Drag {
    /// The point being dragged.
    pub dragged: EntityId,
    /// The targets, `[x, y]`, in order.
    pub targets: Vec<[f64; 2]>,
    /// The expected coordinates after chosen steps.
    pub after: Vec<DragStep>,
}

/// The expected coordinates after one step of a drag.
#[derive(Debug, Clone, PartialEq)]
pub struct DragStep {
    /// The step, 1-based into the targets.
    pub step: usize,
    /// The expected coordinates after the step.
    pub coordinates: Coordinates,
    /// The tags of the entities and constraints the step names as flipped
    /// (empty: none).
    pub flips: Vec<String>,
    /// How many times the step is bisected (at least 1), or `None` for a
    /// step solved at its own target.
    pub halvings: Option<usize>,
}

impl Fixture {
    /// Loads the fixture in `dir`: `sketch.json` and `expected.json`, and
    /// `drag.json` when it is there.
    ///
    /// ```no_run
    /// use laman_debug::fixture::Fixture;
    /// let fixture = Fixture::load(std::path::Path::new("tests/fixtures/c1/sss-triangle"))?;
    /// # Ok::<(), laman_debug::fixture::FixtureError>(())
    /// ```
    pub fn load(dir: &Path) -> Result<Fixture, FixtureError> {
        let sketch = read(&dir.join(SKETCH_FILE))?;
        let expected = read(&dir.join(EXPECTED_FILE))?;
        let drag = read_optional(&dir.join(DRAG_FILE))?;
        Fixture::parse(&sketch, &expected, drag.as_deref())
    }

    /// Parses a fixture from its files' text. `drag` is `None` for a fixture
    /// that is not a drag.
    ///
    /// ```
    /// use laman_debug::fixture::{Fixture, FixtureError};
    /// let sketch = r#"{"format":"laman.sketch","version":1,"entities":[{"id":0,"kind":"point","x":1.0,"y":2.0}]}"#;
    /// let expected = r#"{"status":"UnderConstrained","dof":2,"coordinates":[{"id":9,"coordinate":"x","value":1.0}]}"#;
    /// assert!(matches!(
    ///     Fixture::parse(sketch, expected, None),
    ///     Err(FixtureError::UnknownCoordinate { id, .. }) if id.0 == 9
    /// ));
    /// ```
    pub fn parse(
        sketch: &str,
        expected: &str,
        drag: Option<&str>,
    ) -> Result<Fixture, FixtureError> {
        let sketch = sketch_from_str(sketch).map_err(|source| FixtureError::Sketch {
            file: SKETCH_FILE,
            source,
        })?;
        let known = sketch.coordinates();
        let expected = parse_expected(&sketch, &known, expected)?;
        check_tags(&sketch, &expected)?;
        let drag = match drag {
            None => None,
            Some(text) => Some(parse_drag(&sketch, &known, text)?),
        };
        Ok(Fixture {
            sketch,
            expected,
            drag,
        })
    }
}

/// The name a coordinate has in fixture files and dumps.
///
/// ```
/// use laman::model::Coordinate;
/// use laman_debug::fixture::coordinate_name;
/// assert_eq!(coordinate_name(Coordinate::R), "r");
/// ```
pub fn coordinate_name(c: Coordinate) -> &'static str {
    match c {
        Coordinate::X => "x",
        Coordinate::Y => "y",
        Coordinate::R => "r",
    }
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Name {
    X,
    Y,
    R,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: u32,
    coordinate: Name,
    value: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedFile {
    status: String,
    dof: u32,
    coordinates: Vec<Entry>,
    #[serde(default)]
    redundant: Vec<String>,
    #[serde(default)]
    conflicts: Vec<Vec<String>>,
    #[serde(default)]
    flips: Vec<String>,
    tolerance: Option<f64>,
    digest: Option<String>,
    profiles: Option<ProfilesFile>,
    intersections: Option<IntersectionsFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MeetingFile {
    kind: String,
    at: [f64; 2],
    on_first: f64,
    on_second: f64,
    to: Option<[f64; 2]>,
    to_first: Option<f64>,
    to_second: Option<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PairFile {
    first: u32,
    second: u32,
    meetings: Vec<MeetingFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IntersectionsFile {
    #[serde(default)]
    pairs: Vec<PairFile>,
    tolerance: Option<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AreaFile {
    value: f64,
    tolerance: Option<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TraversalFile {
    entity: u32,
    reversed: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoopFile {
    edges: Vec<TraversalFile>,
    signed_area: AreaFile,
    #[serde(default)]
    joins: Vec<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileFile {
    outer: LoopFile,
    #[serde(default)]
    holes: Vec<LoopFile>,
    net_area: AreaFile,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BranchedFile {
    edges: Vec<u32>,
    joins: Vec<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfilesFile {
    #[serde(default)]
    profiles: Vec<ProfileFile>,
    #[serde(default)]
    open_chains: Vec<Vec<u32>>,
    #[serde(default)]
    dangling: Vec<u32>,
    #[serde(default)]
    branched: Vec<BranchedFile>,
    #[serde(default)]
    degenerate: Vec<u32>,
    #[serde(default)]
    unjoined: Vec<[u32; 2]>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DragFile {
    dragged: u32,
    targets: Vec<[f64; 2]>,
    #[serde(default)]
    after: Vec<StepFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StepFile {
    step: usize,
    coordinates: Vec<Entry>,
    #[serde(default)]
    flips: Vec<String>,
    halvings: Option<usize>,
}

fn read(path: &Path) -> Result<String, FixtureError> {
    std::fs::read_to_string(path).map_err(|source| FixtureError::Read {
        path: path.to_path_buf(),
        source,
    })
}

fn read_optional(path: &Path) -> Result<Option<String>, FixtureError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(FixtureError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn parse_expected(
    sketch: &Sketch,
    known: &Coordinates,
    text: &str,
) -> Result<Expected, FixtureError> {
    let file: ExpectedFile = serde_json::from_str(text).map_err(|source| FixtureError::Json {
        file: EXPECTED_FILE,
        source,
    })?;
    let tolerance = file.tolerance.unwrap_or(tolerance::LENGTH);
    if !(tolerance.is_finite() && tolerance > 0.0) {
        return Err(FixtureError::Tolerance { value: tolerance });
    }
    Ok(Expected {
        status: file.status,
        dof: file.dof,
        coordinates: coordinates(EXPECTED_FILE, known, &file.coordinates)?,
        tolerance,
        redundant: file.redundant,
        conflicts: file.conflicts,
        flips: file.flips,
        digest: file.digest,
        profiles: file
            .profiles
            .map(|p| expected_profiles(sketch, p))
            .transpose()?,
        intersections: file
            .intersections
            .map(|i| expected_intersections(sketch, i))
            .transpose()?,
    })
}

fn expected_intersections(
    sketch: &Sketch,
    file: IntersectionsFile,
) -> Result<ExpectedIntersections, FixtureError> {
    let bad = |what: String| FixtureError::Intersections { what };
    let tolerance = file.tolerance.unwrap_or(INTERSECTION_TOLERANCE);
    if !(tolerance.is_finite() && tolerance > 0.0) {
        return Err(bad(format!("tolerance {tolerance} is not usable")));
    }
    let entity = |id: u32| {
        let id = EntityId(id);
        sketch
            .entity(id)
            .map(|_| id)
            .ok_or(FixtureError::UnknownIntersectionEntity { id })
    };
    let mut pairs = Vec::with_capacity(file.pairs.len());
    for pair in file.pairs {
        let mut meetings = Vec::with_capacity(pair.meetings.len());
        for m in pair.meetings {
            let numbers = [m.at[0], m.at[1], m.on_first, m.on_second];
            let ends = (m.to, m.to_first, m.to_second);
            let finite = numbers.iter().all(|x| x.is_finite())
                && m.to.is_none_or(|p| p.iter().all(|x| x.is_finite()))
                && m.to_first.is_none_or(f64::is_finite)
                && m.to_second.is_none_or(f64::is_finite);
            if !finite {
                return Err(bad(format!(
                    "a {} meeting holds a value that is not finite",
                    m.kind
                )));
            }
            let kind = match (m.kind.as_str(), ends) {
                ("crossing", (None, None, None)) => MeetingKind::Crossing,
                ("touch", (None, None, None)) => MeetingKind::Touch,
                ("overlap", (Some(to), Some(to_first), Some(to_second))) => MeetingKind::Overlap {
                    to,
                    to_first,
                    to_second,
                },
                (kind, _) => {
                    return Err(bad(format!(
                        "a {kind:?} meeting is not a crossing or touch without `to` fields, \
                         or an overlap with all three"
                    )));
                }
            };
            meetings.push(ExpectedMeeting {
                kind,
                at: m.at,
                on_first: m.on_first,
                on_second: m.on_second,
            });
        }
        pairs.push(ExpectedPair {
            first: entity(pair.first)?,
            second: entity(pair.second)?,
            meetings,
        });
    }
    Ok(ExpectedIntersections { pairs, tolerance })
}

fn expected_profiles(
    sketch: &Sketch,
    file: ProfilesFile,
) -> Result<ExpectedProfiles, FixtureError> {
    let entity = |id: u32| {
        let id = EntityId(id);
        sketch
            .entity(id)
            .map(|_| id)
            .ok_or(FixtureError::UnknownEntity { id })
    };
    let entities = |ids: &[u32]| {
        ids.iter()
            .map(|&id| entity(id))
            .collect::<Result<Vec<_>, _>>()
    };
    let constraints = |ids: &[u32]| {
        ids.iter()
            .map(|&raw| {
                let id = ConstraintId(raw);
                sketch
                    .constraints()
                    .any(|(c, _)| c == id)
                    .then_some(id)
                    .ok_or(FixtureError::UnknownConstraint { id })
            })
            .collect::<Result<Vec<_>, _>>()
    };
    let area = |a: AreaFile| {
        let tolerance = a.tolerance.unwrap_or(AREA_TOLERANCE);
        if a.value.is_finite() && tolerance.is_finite() && tolerance > 0.0 {
            Ok(ExpectedArea {
                value: a.value,
                tolerance,
            })
        } else {
            Err(FixtureError::Area {
                value: a.value,
                tolerance,
            })
        }
    };
    let one_loop = |l: LoopFile| -> Result<ExpectedLoop, FixtureError> {
        let mut edges = Vec::with_capacity(l.edges.len());
        for t in &l.edges {
            edges.push(Traversal {
                entity: entity(t.entity)?,
                reversed: t.reversed,
            });
        }
        Ok(ExpectedLoop {
            edges,
            signed_area: area(l.signed_area)?,
            joins: constraints(&l.joins)?,
        })
    };
    let mut profiles = Vec::with_capacity(file.profiles.len());
    for p in file.profiles {
        profiles.push(ExpectedProfile {
            outer: one_loop(p.outer)?,
            holes: p
                .holes
                .into_iter()
                .map(one_loop)
                .collect::<Result<_, _>>()?,
            net_area: area(p.net_area)?,
        });
    }
    let mut branched = Vec::with_capacity(file.branched.len());
    for b in &file.branched {
        branched.push(Branched {
            edges: entities(&b.edges)?,
            joins: entities(&b.joins)?,
        });
    }
    let mut unjoined = Vec::with_capacity(file.unjoined.len());
    for [first, second] in &file.unjoined {
        unjoined.push(Unjoined {
            first: entity(*first)?,
            second: entity(*second)?,
        });
    }
    Ok(ExpectedProfiles {
        profiles,
        open_chains: file
            .open_chains
            .iter()
            .map(|c| entities(c))
            .collect::<Result<_, _>>()?,
        dangling: entities(&file.dangling)?,
        branched,
        degenerate: entities(&file.degenerate)?,
        unjoined,
    })
}

/// Refuses a `redundant` or `conflicts` tag that no constraint carries, and
/// a `flips` tag that no entity or constraint carries.
fn check_tags(sketch: &Sketch, expected: &Expected) -> Result<(), FixtureError> {
    let named = expected
        .redundant
        .iter()
        .chain(expected.conflicts.iter().flatten());
    for tag in named {
        let exists = sketch
            .constraints()
            .any(|(id, _)| sketch.constraint_tag_of(id) == Some(tag.as_str()));
        if !exists {
            return Err(FixtureError::UnknownTag {
                file: EXPECTED_FILE,
                tag: tag.clone(),
            });
        }
    }
    for tag in &expected.flips {
        let exists = sketch
            .constraints()
            .any(|(id, _)| sketch.constraint_tag_of(id) == Some(tag.as_str()))
            || sketch
                .entities()
                .any(|(id, _)| sketch.tag_of(id) == Some(tag.as_str()));
        if !exists {
            return Err(FixtureError::UnknownTag {
                file: EXPECTED_FILE,
                tag: tag.clone(),
            });
        }
    }
    Ok(())
}

fn parse_drag(sketch: &Sketch, known: &Coordinates, text: &str) -> Result<Drag, FixtureError> {
    let file: DragFile = serde_json::from_str(text).map_err(|source| FixtureError::Json {
        file: DRAG_FILE,
        source,
    })?;
    let dragged = EntityId(file.dragged);
    let is_point = sketch
        .entity(dragged)
        .is_some_and(|e| e.kind() == EntityKind::Point);
    if !is_point {
        return Err(FixtureError::NotAPoint { id: dragged });
    }
    let targets = file.targets.len();
    let mut after = Vec::with_capacity(file.after.len());
    for step in file.after {
        if step.step == 0 || step.step > targets {
            return Err(FixtureError::StepOutOfRange {
                step: step.step,
                targets,
            });
        }
        after.push(DragStep {
            step: step.step,
            coordinates: coordinates(DRAG_FILE, known, &step.coordinates)?,
            flips: step.flips,
            halvings: step.halvings,
        });
    }
    Ok(Drag {
        dragged,
        targets: file.targets,
        after,
    })
}

fn coordinates(
    file: &'static str,
    known: &Coordinates,
    entries: &[Entry],
) -> Result<Coordinates, FixtureError> {
    let mut out = Coordinates::new();
    for entry in entries {
        let id = EntityId(entry.id);
        let coordinate = match entry.coordinate {
            Name::X => Coordinate::X,
            Name::Y => Coordinate::Y,
            Name::R => Coordinate::R,
        };
        if !known.contains_key(&(id, coordinate)) {
            return Err(FixtureError::UnknownCoordinate {
                file,
                id,
                coordinate,
            });
        }
        out.insert((id, coordinate), entry.value);
    }
    Ok(out)
}
