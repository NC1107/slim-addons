//! Your module's logic. This is the part you write; `lib.rs` is the ABI shim
//! that calls into here. Keep everything pure and unit-tested (see the bottom
//! of this file) - the wasm boundary is just `lib.rs`.
//!
//! [`apply`] receives a command name (one of your manifest's `command`
//! extension points) and its `input` string, and returns either the output to
//! show, or an error message. Return a plain string for text output, or a
//! `scene/1` JSON string to have slim paint it (see the `scene` command below).

use serde::Deserialize;

/// Dispatches a request to the right command. Add an arm per `command`
/// extension point you declare in manifest.json.
pub fn apply(command: &str, input: &str) -> Result<String, String> {
    match command {
        "echo" => Ok(echo(input)),
        "scene" => Ok(scene(input)),
        other => Err(format!("unknown command: {other}")),
    }
}

/// The simplest possible command: text in, text out.
fn echo(input: &str) -> String {
    if input.trim().is_empty() {
        "Type something and run it again.".to_string()
    } else {
        input.to_string()
    }
}

/// An interactive scene: a counter you bump with a control or reset by tapping
/// the circle. Shows the whole interactive loop - initial frame from empty
/// input, then `{action, state}` on each press or tap - in a few lines.
///
/// slim stores and hands back the opaque `state` between frames, so a stateless
/// module remembers the count without any host storage. Here the state is just
/// the number as text; a real module packs whatever it needs.
fn scene(input: &str) -> String {
    let count = next_count(input);
    // width/height are logical units; the painter scales them to fit.
    format!(
        r#"{{"$slim":"scene/1","width":100,"height":60,"background":"surface","ops":[
            {{"op":"text","x":50,"y":18,"s":"count: {count}","fill":"text","align":"center","size":10}},
            {{"op":"circle","cx":50,"cy":42,"r":10,"fill":"accent","tap":"reset"}},
            {{"op":"text","x":50,"y":45,"s":"tap","fill":"surface","align":"center","size":6}}
        ],"status":"press bump, or tap the dot to reset","controls":["bump"],"state":"{count}","live":true}}"#
    )
}

/// The interactive input is either empty (the initial launch) or a JSON
/// `{action, state}`. Work out the next counter value from it.
fn next_count(input: &str) -> i64 {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return 0;
    }
    let action: Action = match serde_json::from_str(trimmed) {
        Ok(a) => a,
        // Not an interactive frame - treat a bare number as a seed, else start at 0.
        Err(_) => return trimmed.parse().unwrap_or(0),
    };
    let current: i64 = action.state.trim().parse().unwrap_or(0);
    match action.action.as_str() {
        "bump" => current + 1,
        "reset" => 0, // the circle's tap value
        _ => current,
    }
}

#[derive(Deserialize)]
struct Action {
    action: String,
    #[serde(default)]
    state: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echo_returns_its_input() {
        assert_eq!(apply("echo", "hi").unwrap(), "hi");
    }

    #[test]
    fn an_unknown_command_is_an_error_not_a_panic() {
        assert!(apply("nope", "").is_err());
    }

    #[test]
    fn scene_starts_at_zero_and_is_a_scene() {
        let out = apply("scene", "").unwrap();
        assert!(out.contains(r#""$slim":"scene/1""#));
        assert!(out.contains("count: 0"));
    }

    #[test]
    fn bump_increments_and_reset_zeroes_from_the_carried_state() {
        assert_eq!(next_count(r#"{"action":"bump","state":"4"}"#), 5);
        assert_eq!(next_count(r#"{"action":"reset","state":"9"}"#), 0);
        assert_eq!(next_count(r#"{"action":"unknown","state":"7"}"#), 7);
    }
}
