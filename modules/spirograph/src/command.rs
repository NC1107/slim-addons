//! Spirograph: a roulette curve drawn as one `path` op, with controls that
//! change the gears.
//!
//! This module exists to be the thing the `path` op made possible. Before it, a
//! scene could draw rectangles, circles, lines, a colour grid and text, so a
//! curve had to be hundreds of separate `line` ops or nothing. One `d` string is
//! now the whole drawing.
//!
//! Every frame is a fresh sandboxed call. The gears ride in the scene's opaque
//! `state` as `R,r,d`, a tap arrives as an action, and this returns the next
//! frame. There is no host storage and no memory between calls.
//!
//! The per-gear buttons are scene `controls` with names of the module's own
//! (`R+`, `d-`, ...): the client renders any name outside its reserved set as a
//! labelled button that sends the name back as the action, so they are
//! reachable by keyboard and screen reader and sized by the host.

use serde::Deserialize;
use serde_json::json;

/// Samples along the curve.
///
/// The renderer caps a path at 512 steps and silently drops the tail past that,
/// so this leaves room for the opening `M` and some headroom. At this density a
/// closed roulette reads as a smooth curve rather than a polygon.
const SAMPLES: usize = 480;

/// The gear sizes worth offering.
///
/// Bounded at both ends for reasons the drawing shows: below 2 the inner gear
/// stops producing lobes at all, and far above 60 the lobes crowd past what 480
/// samples can resolve, so the curve starts looking like noise rather than
/// getting more intricate.
const MIN_GEAR: i64 = 2;
const MAX_GEAR: i64 = 60;

pub fn apply(command: &str, input: &str) -> Result<String, String> {
    match command {
        "spiro" => Ok(draw(input)),
        other => Err(format!("unknown command: {other}")),
    }
}

fn draw(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return render(&Gears::default());
    }
    let (action, mut gears) = match serde_json::from_str::<Action>(trimmed) {
        Ok(a) => (a.action, Gears::parse(&a.state)),
        // A frame this module cannot read starts over rather than erroring: a
        // scene that vanishes on one bad state is worse than one that resets.
        Err(_) => (String::new(), Gears::default()),
    };

    match action.as_str() {
        "reset" | "clear" => gears = Gears::default(),
        "random" => gears = gears.shuffled(),
        "R+" => gears.outer = clamp(gears.outer + 1),
        "R-" => gears.outer = clamp(gears.outer - 1),
        "r+" => gears.inner = clamp(gears.inner + 1),
        "r-" => gears.inner = clamp(gears.inner - 1),
        "d+" => gears.pen = clamp(gears.pen + 1),
        "d-" => gears.pen = clamp(gears.pen - 1),
        _ => {}
    }
    render(&gears)
}

#[derive(Deserialize)]
struct Action {
    action: String,
    #[serde(default)]
    state: String,
}

struct Gears {
    /// The fixed outer gear.
    outer: i64,
    /// The rolling inner gear.
    inner: i64,
    /// How far the pen sits from the inner gear's centre.
    pen: i64,
}

impl Default for Gears {
    fn default() -> Self {
        Gears { outer: 34, inner: 13, pen: 21 }
    }
}

impl Gears {
    fn parse(state: &str) -> Self {
        let mut parts = state.split(',').map(|p| p.trim().parse::<i64>());
        match (parts.next(), parts.next(), parts.next()) {
            (Some(Ok(outer)), Some(Ok(inner)), Some(Ok(pen))) => {
                Gears { outer: clamp(outer), inner: clamp(inner), pen: clamp(pen) }
            }
            _ => Gears::default(),
        }
    }

    fn state(&self) -> String {
        format!("{},{},{}", self.outer, self.inner, self.pen)
    }

    /// A different curve, derived from the current one.
    ///
    /// A module is pure compute with no host randomness, so this hashes the
    /// current gears into the next set. Deliberately not a uniform shuffle: it
    /// only has to keep landing somewhere different and in range.
    fn shuffled(&self) -> Self {
        let seed =
            (self.outer * 73_856_093) ^ (self.inner * 19_349_663) ^ (self.pen * 83_492_791);
        let mix = |shift: u32| {
            let span = (MAX_GEAR - MIN_GEAR) as u64 + 1;
            MIN_GEAR + (((seed >> shift) as u64) % span) as i64
        };
        let mut gears = Gears { outer: mix(3), inner: mix(11), pen: mix(19) };
        // Lobes need the inner gear smaller than the outer one; swap rather than
        // reroll, so this stays a single pass.
        if gears.inner > gears.outer {
            core::mem::swap(&mut gears.inner, &mut gears.outer);
        }
        if gears.inner == gears.outer {
            gears.inner = clamp(gears.inner - 1);
        }
        gears
    }

    /// How many turns of the inner gear bring the pen home, which is how far
    /// `t` runs: `r / gcd(R, r)`.
    fn turns(&self) -> f64 {
        let g = gcd(self.outer.max(1), self.inner.max(1));
        (self.inner.max(1) / g.max(1)) as f64
    }

    /// How many lobes the drawn curve has: `R / gcd(R, r)`.
    fn lobes(&self) -> i64 {
        self.outer.max(1) / gcd(self.outer.max(1), self.inner.max(1)).max(1)
    }
}

fn clamp(value: i64) -> i64 {
    value.clamp(MIN_GEAR, MAX_GEAR)
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 { a.abs() } else { gcd(b, a % b) }
}

/// The hypotrochoid the gears trace, as a `d` string.
///
/// Points are emitted at two decimal places. On a 100-unit canvas that is well
/// under a pixel at any size slim draws one, and it keeps the string - which is
/// what rides the wire inside the module's output - roughly a third of what full
/// float formatting would make it.
fn path_data(gears: &Gears) -> String {
    let outer = gears.outer as f64;
    let inner = gears.inner as f64;
    let pen = gears.pen as f64;
    let ratio = (outer - inner) / inner.max(1.0);

    // Fit to the drawing area from the curve's own extent, so every gear
    // combination fills the frame instead of some of them being specks.
    let extent = ((outer - inner).abs() + pen).max(1.0);
    let scale = 44.0 / extent;
    let (cx, cy) = (50.0, 46.0);

    let span = core::f64::consts::TAU * gears.turns();
    let mut d = String::with_capacity(SAMPLES * 12);
    for i in 0..=SAMPLES {
        let t = span * (i as f64) / (SAMPLES as f64);
        let x = cx + scale * ((outer - inner) * t.cos() + pen * (ratio * t).cos());
        let y = cy + scale * ((outer - inner) * t.sin() - pen * (ratio * t).sin());
        d.push(if i == 0 { 'M' } else { 'L' });
        push_fixed(&mut d, x);
        d.push(' ');
        push_fixed(&mut d, y);
        d.push(' ');
    }
    d.push('Z');
    d
}

/// Two decimal places, without the formatting machinery a wasm module pays for
/// in bytes.
fn push_fixed(out: &mut String, value: f64) {
    let scaled = (value * 100.0).round() as i64;
    let whole = scaled / 100;
    let frac = (scaled % 100).abs();
    if scaled < 0 && whole == 0 {
        out.push('-');
    }
    out.push_str(&whole.to_string());
    out.push('.');
    if frac < 10 {
        out.push('0');
    }
    out.push_str(&frac.to_string());
}

fn render(gears: &Gears) -> String {
    let ops = vec![json!({
        "op": "path",
        "d": path_data(gears),
        "stroke": "accent",
        "sw": 0.5
    })];
    json!({
        "$slim": "scene/1",
        "width": 100,
        "height": 100,
        "background": "surface",
        "ops": ops,
        "state": gears.state(),
        "controls": ["R-", "R+", "r-", "r+", "d-", "d+", "random", "reset"],
        "status": format!(
            "R {} · r {} · d {} · {} lobes",
            gears.outer,
            gears.inner,
            gears.pen,
            gears.lobes()
        ),
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn scene(input: &str) -> Value {
        serde_json::from_str(&apply("spiro", input).unwrap()).unwrap()
    }

    fn act(action: &str, state: &str) -> Value {
        scene(&json!({ "action": action, "state": state }).to_string())
    }

    fn controls(scene: &Value) -> Vec<&str> {
        scene["controls"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect()
    }

    #[test]
    fn the_status_counts_lobes_as_outer_over_gcd() {
        let cases = [("5,3,5", 5), ("34,13,21", 34), ("8,6,4", 4), ("12,4,4", 3)];
        for (state, lobes) in cases {
            let status = act("", state)["status"].as_str().unwrap().to_owned();
            assert!(status.ends_with(&format!("{lobes} lobes")), "{state}: {status}");
        }
    }

    #[test]
    fn the_gear_buttons_are_scene_controls_not_tap_rects() {
        let scene = scene("");
        for name in ["R-", "R+", "r-", "r+", "d-", "d+", "random", "reset"] {
            assert!(controls(&scene).contains(&name), "missing control {name}");
        }
        let ops = scene["ops"].as_array().unwrap();
        assert!(ops.iter().all(|op| op.get("tap").is_none()), "a tap rect is left in the scene");
    }

    #[test]
    fn each_gear_action_steps_one_gear_by_one() {
        let cases = [
            ("R+", "35,13,21"), ("R-", "33,13,21"), ("r+", "34,14,21"),
            ("r-", "34,12,21"), ("d+", "34,13,22"), ("d-", "34,13,20"),
        ];
        for (action, state) in cases {
            assert_eq!(act(action, "34,13,21")["state"], state, "{action}");
        }
    }

    #[test]
    fn the_gears_stop_at_the_bounds() {
        assert_eq!(act("R+", "60,13,21")["state"], "60,13,21");
        assert_eq!(act("r-", "34,2,21")["state"], "34,2,21");
        assert_eq!(Gears::parse("1,999,0").state(), "2,60,2");
    }

    #[test]
    fn an_unreadable_state_or_action_falls_back_to_the_default_gears() {
        assert_eq!(act("", "garbage")["state"], "34,13,21");
        assert_eq!(act("reset", "5,3,5")["state"], "34,13,21");
        assert_eq!(scene("not json")["state"], "34,13,21");
    }

    #[test]
    fn shuffling_stays_in_range_with_the_inner_gear_smaller() {
        let mut gears = Gears::default();
        for _ in 0..200 {
            gears = gears.shuffled();
            for g in [gears.outer, gears.inner, gears.pen] {
                assert!((MIN_GEAR..=MAX_GEAR).contains(&g));
            }
            assert!(gears.inner < gears.outer);
        }
    }

    #[test]
    fn the_path_fits_the_renderer_step_cap_for_every_gear_pair() {
        for outer in MIN_GEAR..=MAX_GEAR {
            for inner in [MIN_GEAR, 7, outer.min(MAX_GEAR)] {
                let d = path_data(&Gears { outer, inner, pen: 10 });
                let steps = d.chars().filter(|c| matches!(c, 'M' | 'L')).count();
                assert!(steps <= 512, "{outer},{inner}: {steps} steps");
            }
        }
    }

    #[test]
    fn push_fixed_writes_two_decimals_and_keeps_a_small_negative_sign() {
        let mut out = String::new();
        push_fixed(&mut out, -0.05);
        out.push(' ');
        push_fixed(&mut out, 12.5);
        out.push(' ');
        push_fixed(&mut out, 3.0);
        assert_eq!(out, "-0.05 12.50 3.00");
    }
}
