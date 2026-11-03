//! A C1 fixture directory: `sketch.json` through laman's serde form, and the
//! numbers of `expected.json` the comparison reads.

use std::path::Path;

use laman::model::{Sketch, SketchForm};
use serde_json::Value;

pub struct Fixture {
    /// The directory's name.
    pub name: String,
    #[allow(dead_code)] // translated from step 3 on
    pub sketch: Sketch,
    #[allow(dead_code)] // read from step 3 on, by the residual check
    /// `expected.json` as written; `compare.py` and the residual check read
    /// it, the adapters never solve towards it.
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
