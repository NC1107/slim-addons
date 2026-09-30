//! The palette app: a seed colour and a scheme, drawn as swatches you can tap.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::color::{normalize_hex, palette, SCHEMES};

const DEFAULT_SEED: &str = "3a86ff";

pub fn apply(command: &str, input: &str) -> Result<String, String> {
    match command {
        "palette" => Ok(draw(input)),
        other => Err(format!("unknown command: {other}")),
    }
}

#[derive(Deserialize)]
struct Frame {
    action: String,
    #[serde(default)]
    state: String,
}

struct State {
    seed: String,
    scheme: String,
}

impl State {
    fn parse(text: &str) -> State {
        let (seed, scheme) = text.split_once('|').unwrap_or((text, ""));
        State {
            seed: normalize_hex(seed).unwrap_or_else(|| DEFAULT_SEED.to_string()),
            scheme: if SCHEMES.contains(&scheme) { scheme } else { SCHEMES[0] }.to_string(),
        }
    }

    fn encode(&self) -> String {
        format!("{}|{}", self.seed, self.scheme)
    }
}

/// Empty input is the launch frame, plain text is a seed colour, JSON is an interaction.
fn draw(input: &str) -> String {
    let text = input.trim();
    let (state, note) = match serde_json::from_str::<Frame>(text) {
        Ok(frame) => next(State::parse(&frame.state), &frame.action),
        Err(_) if text.is_empty() => (State::parse(""), None),
        Err(_) => match normalize_hex(text) {
            Some(seed) => (State { seed, scheme: SCHEMES[0].to_string() }, None),
            None => (State::parse(""), Some(format!("{text:?} is not a hex colour"))),
        },
    };
    scene(&state, note).to_string()
}

fn next(mut state: State, action: &str) -> (State, Option<String>) {
    if let Some(n) = action.strip_prefix("pick").and_then(|n| n.parse::<usize>().ok()) {
        if let Some(hex) = palette(&state.seed, &state.scheme).get(n) {
            state.seed = hex.clone();
        }
    } else if let Some(text) = action.strip_prefix("seed:") {
        match normalize_hex(text) {
            Some(seed) => state.seed = seed,
            None => return (state, Some(format!("{text:?} is not a hex colour"))),
        }
    } else if SCHEMES.contains(&action) {
        state.scheme = action.to_string();
    }
    (state, None)
}

fn scene(state: &State, note: Option<String>) -> Value {
    let swatches = palette(&state.seed, &state.scheme);
    let gap = 2.0;
    let width = (92.0 - gap * (swatches.len() as f64 - 1.0)) / swatches.len() as f64;
    let mut ops = vec![json!({"op": "text", "x": 4, "y": 7, "s": format!("{} from #{}", state.scheme, state.seed), "fill": "text", "size": 5, "align": "left"})];
    for (i, hex) in swatches.iter().enumerate() {
        let x = 4.0 + i as f64 * (width + gap);
        ops.push(json!({"op": "rect", "x": x, "y": 12, "w": width, "h": 24, "fill": format!("#{hex}"), "r": 2, "tap": format!("pick{i}")}));
        ops.push(json!({"op": "text", "x": x + width / 2.0, "y": 41, "s": format!("#{hex}"), "fill": "muted", "size": 3.5, "align": "center"}));
    }
    ops.push(json!({"op": "input", "x": 4, "y": 46, "w": 60, "submit": "seed", "placeholder": "a hex colour, like ff8800", "max": 7}));
    let status = note.unwrap_or_else(|| "tap a swatch to make it the new seed".to_string());
    json!({"$slim": "scene/1", "width": 100, "height": 62, "background": "surface", "ops": ops,
        "status": status, "controls": SCHEMES, "state": state.encode(), "live": true})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::{to_hex, to_hsl};

    fn scene_of(input: &str) -> Value {
        serde_json::from_str(&apply("palette", input).unwrap()).unwrap()
    }

    fn swatch_fills(scene: &Value) -> Vec<String> {
        let ops = scene["ops"].as_array().unwrap();
        ops.iter().filter(|o| o["op"] == "rect").map(|o| o["fill"].as_str().unwrap().to_string()).collect()
    }

    #[test]
    fn hex_survives_a_round_trip_through_hsl() {
        for hex in ["3a86ff", "ff0000", "00ff00", "0000ff", "808080", "000000", "ffffff", "e5a00d"] {
            assert_eq!(to_hex(to_hsl(hex)), hex);
        }
    }

    #[test]
    fn hex_input_is_normalised() {
        assert_eq!(normalize_hex("#ABC").as_deref(), Some("aabbcc"));
        assert_eq!(normalize_hex(" FF8800 ").as_deref(), Some("ff8800"));
        assert_eq!(normalize_hex("ff88"), None);
        assert_eq!(normalize_hex("gggggg"), None);
    }

    #[test]
    fn the_launch_frame_is_an_interactive_scene_with_the_default_seed() {
        let s = scene_of("");
        assert_eq!(s["$slim"], "scene/1");
        assert_eq!(s["state"], "3a86ff|analogous");
        assert_eq!(swatch_fills(&s).len(), 5);
        assert_eq!(s["controls"], json!(SCHEMES));
    }

    #[test]
    fn the_complement_of_red_is_cyan_and_a_triad_is_evenly_spaced() {
        assert_eq!(palette("ff0000", "complementary")[1], "00ffff");
        assert_eq!(palette("ff0000", "triad"), ["ff0000", "00ff00", "0000ff"]);
    }

    #[test]
    fn shades_keep_the_hue_and_the_seed_is_the_middle_one() {
        let shades = palette("3a86ff", "shades");
        assert_eq!(shades[2], "3a86ff");
        assert!(shades.windows(2).all(|w| to_hsl(&w[0]).l < to_hsl(&w[1]).l));
    }

    #[test]
    fn a_scheme_button_changes_the_scheme_and_keeps_the_seed() {
        let s = scene_of(r##"{"action":"triad","state":"ff0000|analogous"}"##);
        assert_eq!(s["state"], "ff0000|triad");
        assert_eq!(swatch_fills(&s), ["#ff0000", "#00ff00", "#0000ff"]);
    }

    #[test]
    fn tapping_a_swatch_makes_it_the_seed() {
        let s = scene_of(r#"{"action":"pick1","state":"ff0000|triad"}"#);
        assert_eq!(s["state"], "00ff00|triad");
    }

    #[test]
    fn a_typed_seed_is_accepted_and_a_bad_one_keeps_the_frame() {
        assert_eq!(scene_of(r##"{"action":"seed:#F80","state":"ff0000|triad"}"##)["state"], "ff8800|triad");
        let bad = scene_of(r#"{"action":"seed:nope","state":"ff0000|triad"}"#);
        assert_eq!(bad["state"], "ff0000|triad");
        assert!(bad["status"].as_str().unwrap().contains("not a hex colour"));
    }

    #[test]
    fn plain_text_input_seeds_the_palette_for_the_slash_command() {
        assert_eq!(scene_of("#ff8800")["state"], "ff8800|analogous");
        assert!(scene_of("banana")["status"].as_str().unwrap().contains("not a hex colour"));
    }

    #[test]
    fn a_damaged_state_falls_back_instead_of_failing() {
        assert_eq!(scene_of(r#"{"action":"triad","state":"garbage"}"#)["state"], "3a86ff|triad");
    }

    #[test]
    fn every_fill_is_a_hex_colour_or_a_theme_token() {
        let s = scene_of("");
        let tokens = ["accent", "surface", "sunken", "muted", "text", "border", "danger"];
        for op in s["ops"].as_array().unwrap() {
            if let Some(fill) = op["fill"].as_str() {
                assert!(tokens.contains(&fill) || (fill.len() == 7 && fill.starts_with('#')), "{fill}");
            }
        }
    }
}
