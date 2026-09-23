//! Scene Stress: a discovery module. Every scenario here deliberately sits on
//! or past one documented ceiling of the scene contract - it is not trying to
//! be useful, it is trying to show what actually happens at each boundary.
//!
//! `apply` is the one entry point every extension point (command,
//! slash-command, app) calls into with a scenario name as `input`, optionally
//! `name:arg`. Unknown or malformed input always falls back to the menu/help
//! text rather than erroring, because the slash-command surface hands this
//! whatever a person typed - long, unicode, newlines, or nonsense - and none
//! of that should be able to crash a module run.

pub fn apply(command: &str, input: &str) -> Result<String, String> {
    match command {
        "probe" => Ok(probe(input)),
        other => Err(format!("unknown command: {other}")),
    }
}

fn probe(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return menu_scene();
    }
    let (name, arg) = split_scenario(trimmed);
    match name.as_str() {
        "help" | "menu" => menu_scene(),
        "grid-max" => grid_max_scene(),
        "grid-over" => grid_over_scene(),
        "path-over" => path_over_scene(),
        "images-over" => images_over_scene(),
        "image-oversize" => image_oversize_scene(),
        "flood" => flood_scene(arg),
        "state-huge" => state_huge_scene(arg),
        "aspect" => aspect_scene(arg),
        _ => unknown_scenario_text(trimmed),
    }
}

/// `name:arg` split on the first colon; a bare name has no arg. Only ever
/// compares the whole lowercased name against fixed ASCII keywords below, so
/// nothing here needs the input to be well-formed, short, or plausible.
fn split_scenario(trimmed: &str) -> (String, Option<&str>) {
    match trimmed.split_once(':') {
        Some((name, arg)) => (name.trim().to_lowercase(), Some(arg.trim())),
        None => (trimmed.to_lowercase(), None),
    }
}

/// Echoes back a bounded slice of whatever was typed, so a huge or unicode
/// scenario name shows up in the reply without the reply itself being able to
/// grow unbounded. Truncates on a `char` boundary, never a byte one - the
/// composer can hand this a slash-command body of any content.
fn unknown_scenario_text(raw: &str) -> String {
    let shown: String = raw.chars().take(60).collect();
    let marker = if raw.chars().count() > 60 { "..." } else { "" };
    format!("unknown scenario: \"{shown}{marker}\"\n\n{}", SCENARIO_LIST)
}

const SCENARIO_LIST: &str = "scenarios: grid-max, grid-over, path-over, \
images-over, image-oversize, flood[:n], state-huge[:n], aspect:wide|tall";

fn menu_scene() -> String {
    let lines = SCENARIO_LIST.split(", ").collect::<Vec<_>>();
    let ops: Vec<String> = lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let y = 12 + i as i32 * 11;
            format!(r#"{{"op":"text","x":5,"y":{y},"s":"{line}","fill":"text","size":7}}"#)
        })
        .collect();
    format!(
        r#"{{"$slim":"scene/1","width":100,"height":100,"background":"surface","ops":[{}],"status":"scene-contract boundary probes; run /stress <scenario> or /stress <scenario>:<arg>"}}"#,
        ops.join(",")
    )
}

/// 128x128: the exact per-axis ceiling ([`CellsOp.maxPerAxis`] in the client).
/// A checkerboard, not a solid fill, so a stride mismatch anywhere in the
/// pipeline would show up as a broken pattern rather than being invisible.
fn grid_max_scene() -> String {
    const N: usize = 128;
    let data = checkerboard(N, N);
    format!(
        r#"{{"$slim":"scene/1","width":100,"height":100,"ops":[{{"op":"cells","cols":{N},"rows":{N},"data":"{data}","palette":["sunken","accent"]}}],"status":"128x128 checkerboard: the exact grid-axis ceiling"}}"#
    )
}

/// Declares a 200x200 grid - over the 128-per-axis ceiling on both axes - with
/// `data` laid out the way a module author naturally would: row-major at the
/// *declared* 200-wide stride, one lit cell at the start of every row, which
/// draws as a solid vertical line down the left edge if read at that stride.
///
/// The client clamps `cols`/`rows` to 128 but never touches `data`, so the
/// painter reads the same bytes at a *128*-wide stride instead - which
/// scatters the line rather than shrinking it. See this module's PR
/// description for the measured effect.
fn grid_over_scene() -> String {
    const DECLARED: usize = 200;
    let mut data = String::with_capacity(DECLARED * DECLARED);
    for i in 0..DECLARED * DECLARED {
        data.push(if i % DECLARED == 0 { '1' } else { '0' });
    }
    format!(
        r#"{{"$slim":"scene/1","width":100,"height":100,"ops":[{{"op":"cells","cols":{DECLARED},"rows":{DECLARED},"data":"{data}","palette":["sunken","accent"]}}],"status":"declares 200x200 (over the 128 cap) with a vertical line down column 0 of an intended 200-wide row; watch what the 128-wide clamp does to it"}}"#
    )
}

fn checkerboard(cols: usize, rows: usize) -> String {
    let mut data = String::with_capacity(cols * rows);
    for i in 0..cols * rows {
        let col = i % cols;
        let row = i / cols;
        data.push(if (row + col).is_multiple_of(2) { '0' } else { '1' });
    }
    data
}

/// A zigzag of 600 line segments after the initial move - well past
/// [`sceneMaxPathSteps`]'s 512 - so the shape is visibly cut off mid-zigzag
/// rather than merely "a path that used to be bigger".
fn path_over_scene() -> String {
    const SEGMENTS: usize = 600;
    let mut d = String::from("M 2 50");
    for k in 0..SEGMENTS {
        let x = 2.0 + (k as f64) * 0.16;
        let y = if k % 2 == 0 { 10.0 } else { 90.0 };
        d.push_str(&format!(" L {x:.2} {y:.0}"));
    }
    format!(
        r#"{{"$slim":"scene/1","width":100,"height":100,"ops":[{{"op":"path","d":"{d}","stroke":"accent","sw":1}}],"status":"a zigzag of 601 path steps (1 move + 600 lines), 89 past the 512-step cap"}}"#
    )
}

/// A known-valid 1x1 transparent PNG, reused for every image op below: the
/// scenarios here are testing image *count* and *byte* ceilings, not decoding.
const TINY_PNG_B64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

/// Nine images in one scene - one past [`ImageOp.maxPerScene`]'s cap of 8.
fn images_over_scene() -> String {
    const COUNT: usize = 9;
    let ops: Vec<String> = (0..COUNT)
        .map(|i| {
            let x = 2 + i * 11;
            format!(r#"{{"op":"image","x":{x},"y":40,"w":9,"h":9,"b64":"{TINY_PNG_B64}"}}"#)
        })
        .collect();
    format!(
        r#"{{"$slim":"scene/1","width":100,"height":100,"ops":[{}],"status":"9 images, one past the 8-per-scene cap; the last one placed should never draw"}}"#,
        ops.join(",")
    )
}

/// One image whose base64 payload is longer than [`ImageOp.maxEncodedLength`]
/// (88 * 1024). The length check runs before any base64 decode, so this does
/// not need to decode to anything - only to be too long.
fn image_oversize_scene() -> String {
    const OVER: usize = 88 * 1024 + 200;
    let oversized = "A".repeat(OVER);
    format!(
        r#"{{"$slim":"scene/1","width":100,"height":100,"ops":[{{"op":"image","x":10,"y":10,"w":40,"h":40,"b64":"{oversized}"}}],"status":"one image, {OVER} base64 chars, 200 over the 88k cap - should be dropped before it is even decoded"}}"#
    )
}

/// `flood[:n]` - `n` small rect ops (default 1500, capped at 20000 so a
/// module run stays cheap even at the largest count offered). There is no
/// documented ceiling on how many ops a scene may carry; this is the vehicle
/// for finding out what actually happens when the JSON crosses the *server's*
/// 64k shared-output clamp (`MAX_SHARED_OUTPUT_BYTES` in slim-m), which is a
/// different ceiling from anything in the client's scene parser.
fn flood_scene(arg: Option<&str>) -> String {
    const DEFAULT: usize = 1500;
    const MAX: usize = 20_000;
    let count = arg
        .and_then(|a| a.parse::<usize>().ok())
        .unwrap_or(DEFAULT)
        .clamp(1, MAX);
    let per_row = 40usize;
    let ops: Vec<String> = (0..count)
        .map(|i| {
            let col = (i % per_row) as f64;
            let row = (i / per_row) as f64;
            let fill = if i % 2 == 0 { "accent" } else { "sunken" };
            format!(
                r#"{{"op":"rect","x":{:.1},"y":{:.1},"w":2,"h":2,"fill":"{fill}"}}"#,
                col * 2.5,
                row * 2.5
            )
        })
        .collect();
    let ops_json = ops.join(",");
    let approx_bytes = ops_json.len();
    format!(
        r#"{{"$slim":"scene/1","width":100,"height":100,"ops":[{ops_json}],"status":"{count} rect ops, ~{approx_bytes} bytes of ops JSON alone - compare to slim-m's 64k shared-output clamp for apps and code-block runs"}}"#
    )
}

/// `state-huge[:n]` - an interactive scene whose opaque `state` string is `n`
/// characters (default 70000, capped at 100000). `state` has no documented
/// size ceiling of its own in the client parser; this checks what the
/// server's 64k shared-output clamp - which the whole scene string rides
/// through for an app - does to a state that big on its own.
fn state_huge_scene(arg: Option<&str>) -> String {
    const DEFAULT: usize = 70_000;
    const MAX: usize = 100_000;
    let len = arg
        .and_then(|a| a.parse::<usize>().ok())
        .unwrap_or(DEFAULT)
        .min(MAX);
    let pattern = "0123456789";
    let mut state = String::with_capacity(len);
    while state.len() < len {
        state.push_str(pattern);
    }
    state.truncate(len);
    format!(
        r#"{{"$slim":"scene/1","width":100,"height":40,"ops":[{{"op":"text","x":50,"y":20,"align":"center","s":"state is {len} chars","fill":"text","size":9}}],"status":"an interactive scene whose state alone is {len} bytes","controls":["bump"],"state":"{state}","live":true}}"#
    )
}

/// `aspect:wide` (1000:1) or `aspect:tall` (1:1000, the default branch for
/// anything else). Both are inside every per-op ceiling; the only thing being
/// tested is what an extreme `width`/`height` ratio does once the client
/// scales it into an actual box.
fn aspect_scene(arg: Option<&str>) -> String {
    let tall = arg != Some("wide");
    let (width, height, label) = if tall {
        (3, 3000, "1:1000 tall")
    } else {
        (3000, 3, "1000:1 wide")
    };
    let (tx, ty) = (width as f64 / 2.0, height as f64 / 2.0);
    format!(
        r#"{{"$slim":"scene/1","width":{width},"height":{height},"background":"surface","ops":[{{"op":"text","x":{tx},"y":{ty},"align":"center","s":"{label}","fill":"text","size":10}}],"status":"declares a {label} scene; see the README for what a live client measured this as"}}"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene(input: &str) -> serde_json::Value {
        let out = apply("probe", input).unwrap();
        serde_json::from_str(&out).expect("scenario output must be valid JSON")
    }

    #[test]
    fn empty_input_returns_the_menu_as_a_scene() {
        let v = scene("");
        assert_eq!(v[r#"$slim"#], "scene/1");
    }

    #[test]
    fn unknown_command_is_an_error_not_a_panic() {
        assert!(apply("nope", "").is_err());
    }

    #[test]
    fn grid_max_is_exactly_128_by_128_with_matching_data_length() {
        let v = scene("grid-max");
        let op = &v["ops"][0];
        assert_eq!(op["cols"], 128);
        assert_eq!(op["rows"], 128);
        assert_eq!(op["data"].as_str().unwrap().len(), 128 * 128);
    }

    #[test]
    fn grid_over_declares_200_with_data_sized_for_200() {
        let v = scene("grid-over");
        let op = &v["ops"][0];
        assert_eq!(op["cols"], 200);
        assert_eq!(op["rows"], 200);
        let data = op["data"].as_str().unwrap();
        assert_eq!(data.len(), 200 * 200);
        // Every 200th char is the lit column-0 marker, at the declared stride.
        assert!(data.as_bytes().iter().step_by(200).all(|&b| b == b'1'));
    }

    #[test]
    fn path_over_has_601_steps_worth_of_commands() {
        let v = scene("path-over");
        let d = v["ops"][0]["d"].as_str().unwrap();
        // 1 "M" plus 600 "L" tokens.
        assert_eq!(d.matches(" L ").count(), 600);
        assert!(d.starts_with("M "));
    }

    #[test]
    fn images_over_sends_exactly_nine_image_ops() {
        let v = scene("images-over");
        let count = v["ops"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|op| op["op"] == "image")
            .count();
        assert_eq!(count, 9);
    }

    #[test]
    fn image_oversize_is_past_the_88k_cap() {
        let v = scene("image-oversize");
        let b64 = v["ops"][0]["b64"].as_str().unwrap();
        assert!(b64.len() > 88 * 1024);
    }

    #[test]
    fn flood_defaults_to_1500_ops_and_respects_an_explicit_count() {
        let v = scene("flood");
        assert_eq!(v["ops"].as_array().unwrap().len(), 1500);
        let v = scene("flood:10");
        assert_eq!(v["ops"].as_array().unwrap().len(), 10);
    }

    #[test]
    fn flood_default_output_exceeds_the_servers_shared_output_clamp() {
        // MAX_SHARED_OUTPUT_BYTES in crates/slimm-server/src/store/code_runs.rs.
        const MAX_SHARED_OUTPUT_BYTES: usize = 64 * 1024;
        let out = apply("probe", "flood").unwrap();
        assert!(out.len() > MAX_SHARED_OUTPUT_BYTES);
    }

    #[test]
    fn flood_count_is_capped_rather_than_unbounded() {
        let v = scene("flood:999999999");
        assert_eq!(v["ops"].as_array().unwrap().len(), 20_000);
    }

    #[test]
    fn state_huge_state_length_matches_the_request() {
        let v = scene("state-huge:123");
        assert_eq!(v["state"].as_str().unwrap().len(), 123);
        let v = scene("state-huge");
        assert_eq!(v["state"].as_str().unwrap().len(), 70_000);
    }

    #[test]
    fn aspect_wide_and_tall_declare_extreme_ratios() {
        let wide = scene("aspect:wide");
        assert_eq!(wide["width"], 3000);
        assert_eq!(wide["height"], 3);
        let tall = scene("aspect:tall");
        assert_eq!(tall["width"], 3);
        assert_eq!(tall["height"], 3000);
    }

    #[test]
    fn a_long_unknown_scenario_name_does_not_panic_and_is_bounded() {
        let huge: String = "x".repeat(500_000);
        let out = apply("probe", &huge).unwrap();
        assert!(out.len() < huge.len());
        assert!(out.starts_with("unknown scenario"));
    }

    #[test]
    fn unicode_and_newlines_in_the_scenario_name_do_not_panic() {
        let weird = "grid\u{2028}max\u{202e}\u{1f4a3}\n\n\u{0}\u{feff}";
        let out = apply("probe", weird).unwrap();
        assert!(out.starts_with("unknown scenario"));
    }

    #[test]
    fn whitespace_and_case_around_a_known_scenario_still_matches() {
        let v = scene("  Grid-Max  ");
        assert_eq!(v["ops"][0]["cols"], 128);
    }

    #[test]
    fn a_bare_colon_with_no_name_is_unknown_not_a_panic() {
        let out = apply("probe", ":::::").unwrap();
        assert!(out.starts_with("unknown scenario"));
    }
}
