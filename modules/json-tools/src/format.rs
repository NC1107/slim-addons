//! Pretty-prints a JSON string with 2-space indentation, preserving key order.

use serde_json::Value;

/// Parses `input` as JSON and re-serializes it with 2-space indentation.
/// On a parse failure, returns the underlying `serde_json` error text, which
/// already carries a `line X column Y` location.
pub fn pretty_print(input: &str) -> Result<String, String> {
    let value: Value = serde_json::from_str(input).map_err(|err| err.to_string())?;
    serde_json::to_string_pretty(&value).map_err(|err| err.to_string())
}
