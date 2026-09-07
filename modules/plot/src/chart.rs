//! Turns a list of numbers into a bar-chart scene. A worked example of a
//! module whose output is a `scene/1` drawing rather than text - slim paints
//! whatever the command returns, so "render a chart" is just "return a scene".

use serde_json::{json, Value};

/// The most bars we draw; past this a chart is unreadable at message size, so
/// extra values are dropped rather than crammed in.
const MAX_BARS: usize = 24;

pub fn apply(command: &str, input: &str) -> Result<String, String> {
    match command {
        "bars" => bars(input),
        other => Err(format!("unknown command: {other}")),
    }
}

fn bars(input: &str) -> Result<String, String> {
    let data = parse(input);
    if data.is_empty() {
        return Err(
            "give me some numbers - `3, 7, 2, 8` or `mon: 3, tue: 7` (one per line or comma-separated)"
                .to_string(),
        );
    }
    Ok(render(&data))
}

/// Parses labelled or bare numbers, split on newlines and commas. Each item is
/// `label: value`, `label = value`, `label value`, or a bare number (labelled
/// by its position). Non-numeric items are skipped rather than failing the lot.
fn parse(input: &str) -> Vec<(String, f64)> {
    let mut out = Vec::new();
    for raw in input.split(['\n', ',']) {
        let item = raw.trim();
        if item.is_empty() {
            continue;
        }
        let (label, value) = split_item(item);
        if let Some(v) = value {
            let label = label.unwrap_or_else(|| (out.len() + 1).to_string());
            out.push((label, v));
            if out.len() >= MAX_BARS {
                break;
            }
        }
    }
    out
}

/// Splits one item into an optional label and its value. A bare number has no
/// label; `label: value` / `label = value` / `label value` do.
fn split_item(item: &str) -> (Option<String>, Option<f64>) {
    if let Ok(v) = item.parse::<f64>() {
        return (None, Some(v));
    }
    for sep in [':', '='] {
        if let Some((label, rest)) = item.rsplit_once(sep) {
            if let Ok(v) = rest.trim().parse::<f64>() {
                return (Some(label.trim().to_string()), Some(v));
            }
        }
    }
    if let Some((label, rest)) = item.rsplit_once(char::is_whitespace) {
        if let Ok(v) = rest.trim().parse::<f64>() {
            return (Some(label.trim().to_string()), Some(v));
        }
    }
    (None, None)
}

fn render(data: &[(String, f64)]) -> String {
    let n = data.len() as f64;
    let width = (n * 16.0 + 8.0).max(100.0);
    let height = 64.0;
    let axis_y = height - 12.0;
    let top = 10.0;
    let span = axis_y - top;
    // Scale to the largest bar; a chart of all-equal values still fills the height. A max of 0 (all zero) avoids a divide-by-zero and draws flat bars.
    let max = data.iter().map(|(_, v)| v.abs()).fold(0.0_f64, f64::max);
    let slot = (width - 8.0) / n;
    let bar_w = slot * 0.66;

    let mut ops: Vec<Value> = vec![json!({
        "op": "line", "x1": 4.0, "y1": axis_y, "x2": width - 4.0, "y2": axis_y,
        "stroke": "border", "sw": 0.5
    })];
    for (i, (label, value)) in data.iter().enumerate() {
        let x = 4.0 + i as f64 * slot + (slot - bar_w) / 2.0;
        let h = if max > 0.0 { value.abs() / max * span } else { 0.0 };
        let y = axis_y - h;
        ops.push(json!({
            "op": "rect", "x": x, "y": y, "w": bar_w, "h": h,
            "fill": "accent", "r": 1.0
        }));
        ops.push(json!({
            "op": "text", "x": x + bar_w / 2.0, "y": y - 1.5, "s": trim_num(*value),
            "fill": "text", "align": "center", "size": 4.0
        }));
        ops.push(json!({
            "op": "text", "x": x + bar_w / 2.0, "y": axis_y + 5.0, "s": label,
            "fill": "muted", "align": "center", "size": 4.0
        }));
    }

    let scene = json!({
        "$slim": "scene/1",
        "width": width,
        "height": height,
        "background": "surface",
        "ops": ops,
        "status": format!("{} value{}", data.len(), if data.len() == 1 { "" } else { "s" }),
        "live": false,
    });
    scene.to_string()
}

/// A compact number label: integers without a trailing `.0`, others to one dp.
fn trim_num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bare_numbers_labelled_by_position() {
        assert_eq!(
            parse("3, 7, 2"),
            vec![("1".into(), 3.0), ("2".into(), 7.0), ("3".into(), 2.0)]
        );
    }

    #[test]
    fn parses_labelled_values_across_separators() {
        assert_eq!(
            parse("mon: 3\ntue = 7\nwed 2"),
            vec![("mon".into(), 3.0), ("tue".into(), 7.0), ("wed".into(), 2.0)]
        );
    }

    #[test]
    fn skips_junk_and_caps_the_count() {
        // Junk is dropped; a bare number is labelled by its position among the kept values, so the lone 5 is "1".
        assert_eq!(parse("x, 5, nope"), vec![("1".into(), 5.0)]);
        assert_eq!(parse(&"1,".repeat(40)).len(), MAX_BARS);
    }

    #[test]
    fn empty_input_is_an_error_not_a_panic() {
        assert!(apply("bars", "   ").is_err());
    }

    #[test]
    fn bars_returns_a_scene_with_a_bar_per_value() {
        let out = apply("bars", "3, 7, 2").unwrap();
        assert!(out.contains(r#""$slim":"scene/1""#));
        // one axis line + 3 bars, each a rect.
        assert_eq!(out.matches(r#""op":"rect""#).count(), 3);
    }

    #[test]
    fn a_label_with_a_quote_stays_valid_json() {
        // serde_json escapes it; a hand-built string would not.
        let out = apply("bars", "a\"b: 4").unwrap();
        let _: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    }
}
