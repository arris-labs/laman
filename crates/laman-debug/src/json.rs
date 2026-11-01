//! A sketch's serde form as JSON text (`docs/DATA-MODEL.md` §The serde
//! form). Floats round-trip exactly, so a fixture's digits are the solver's
//! input bits.

use laman::model::{ModelError, Sketch};
use serde_json::Value;

/// Why a sketch could not be loaded from text.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// The text is not JSON, or does not have the shape of a sketch file.
    #[error("not a sketch file: {0}")]
    Json(#[from] serde_json::Error),
    /// The file is well-formed JSON of a sketch but the sketch is refused;
    /// the error names the id.
    #[error("{0}")]
    Model(#[from] ModelError),
}

/// Loads a sketch from JSON text.
///
/// The `version` is read before the rest, so a file from a newer version
/// is refused with [`ModelError::FormatVersion`] even where its shape has
/// changed.
///
/// ```
/// use laman_debug::json::sketch_from_str;
/// let sk = sketch_from_str(
///     r#"{"format":"laman.sketch","version":1,"entities":[{"id":0,"kind":"point","x":1.0,"y":2.0}]}"#,
/// )?;
/// assert_eq!(sk.entities().count(), 1);
/// # Ok::<(), laman_debug::json::LoadError>(())
/// ```
pub fn sketch_from_str(text: &str) -> Result<Sketch, LoadError> {
    let value: Value = serde_json::from_str(text)?;
    if let Some(found) = value.get("version").and_then(Value::as_u64) {
        let probe = Sketch::from_form(serde_json::from_value(serde_json::json!({
            "format": value.get("format").cloned().unwrap_or(Value::Null),
            "version": found,
        }))?);
        if let Err(e @ ModelError::FormatVersion { .. }) = probe {
            return Err(e.into());
        }
    }
    Ok(Sketch::from_form(serde_json::from_value(value)?)?)
}

/// Writes a sketch as pretty JSON text, one trailing newline.
///
/// ```
/// use laman::model::Sketch;
/// use laman_debug::json::{sketch_from_str, sketch_to_string};
/// let mut sk = Sketch::new();
/// sk.point(1.0, 2.0)?;
/// let text = sketch_to_string(&sk);
/// assert_eq!(sketch_to_string(&sketch_from_str(&text)?), text);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn sketch_to_string(sketch: &Sketch) -> String {
    // A form holds only strings, integers and finite floats, which JSON
    // always writes; the fallback is unreachable and still not a panic.
    let mut text = serde_json::to_string_pretty(&sketch.to_form()).unwrap_or_default();
    text.push('\n');
    text
}
