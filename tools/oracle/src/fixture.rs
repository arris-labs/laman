//! A C1 fixture directory: `sketch.json` through laman's serde form, the
//! numbers of `expected.json` the comparison reads, and what the checker says
//! about them. Shared by the adapters.

use std::collections::BTreeSet;
use std::path::Path;

use laman::model::{ConstraintId, Coordinate, Coordinates, EntityId, Sketch, SketchForm};
use laman_check::Source;
use serde_json::Value;

pub struct Fixture {
    /// The directory's name.
    pub name: String,
    pub sketch: Sketch,
    /// `expected.json` as written.
    pub expected: Value,
}

/// Load `dir/sketch.json` and `dir/expected.json`. A message, never a panic,
/// for a file that is missing or malformed.
pub fn load(dir: &Path) -> Result<Fixture, String> {
    let read = |file: &str| {
        let path = dir.join(file);
        std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
    };
    let form: SketchForm = serde_json::from_str(&read("sketch.json")?)
        .map_err(|e| format!("{}/sketch.json: {e}", dir.display()))?;
    let sketch =
        Sketch::from_form(form).map_err(|e| format!("{}/sketch.json: {e:?}", dir.display()))?;
    let expected = serde_json::from_str(&read("expected.json")?)
        .map_err(|e| format!("{}/expected.json: {e}", dir.display()))?;
    let name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("{}: no directory name", dir.display()))?
        .to_owned();
    Ok(Fixture {
        name,
        sketch,
        expected,
    })
}

impl Fixture {
    /// The sketch's own values with the expected coordinates laid over them:
    /// every parameter of the sketch, fixed ones at their fixed value.
    pub fn expected_coordinates(&self) -> Result<Coordinates, String> {
        let mut out = self.sketch.coordinates();
        let list = self.expected["coordinates"]
            .as_array()
            .ok_or("expected.json has no coordinates")?;
        for c in list {
            let id = c["id"]
                .as_u64()
                .and_then(|i| u32::try_from(i).ok())
                .ok_or("a coordinate without an id")?;
            let coordinate = match c["coordinate"].as_str() {
                Some("x") => Coordinate::X,
                Some("y") => Coordinate::Y,
                Some("r") => Coordinate::R,
                other => return Err(format!("unknown coordinate {other:?}")),
            };
            let value = c["value"].as_f64().ok_or("a coordinate without a value")?;
            let key = (EntityId(id), coordinate);
            if !out.contains_key(&key) {
                return Err(format!(
                    "expected.json names {id} {coordinate:?}, not in the sketch"
                ));
            }
            out.insert(key, value);
        }
        Ok(out)
    }

    /// The constraints the checker finds violated at the expected
    /// coordinates: none for a consistent fixture, the conflicting
    /// constraint of a conflicting twin. A solver's residuals must agree
    /// with this set (the translation proof).
    pub fn violated_at_expected(&self) -> Result<BTreeSet<ConstraintId>, String> {
        let report = laman_check::check(&self.sketch, &self.expected_coordinates()?);
        let mut out = BTreeSet::new();
        for v in &report.violations {
            match v.id {
                Source::Constraint(id) => {
                    out.insert(id);
                }
                Source::Arc(id) => return Err(format!("an arc's own equation fails at {id}")),
            }
        }
        Ok(out)
    }

    /// A constraint by laman's tag, or `#id` when it has none.
    pub fn constraint_name(&self, id: ConstraintId) -> String {
        match self.sketch.constraint_tag_of(id) {
            Some(tag) => tag.to_owned(),
            None => format!("#{}", id.0),
        }
    }

    /// An arc's own equation (start and end the same distance from the
    /// centre), named `arc:` plus the arc's tag or `#id`.
    pub fn arc_name(&self, id: EntityId) -> String {
        match self.sketch.tag_of(id) {
            Some(tag) => format!("arc:{tag}"),
            None => format!("arc:#{}", id.0),
        }
    }
}
