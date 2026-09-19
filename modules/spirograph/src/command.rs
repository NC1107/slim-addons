//! Spirograph: a roulette curve drawn as one `path` op, with tappable buttons
//! that change the gears.
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
//! The scene's `controls` vocabulary is fixed (`play`, `step`, `random`,
//! `clear`, `reset`), so the per-gear buttons are tappable `rect` ops instead.
//! That is the general way a module gets controls of its own shape.

use serde::Deserialize;
use serde_json::{Value, json};

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

    /// How many lobes the curve closes after, which is also how far `t` runs:
    /// `r / gcd(R, r)` turns of the inner gear brings the pen home.
    fn turns(&self) -> f64 {
        let g = gcd(self.outer.max(1), self.inner.max(1));
        (self.inner.max(1) / g.max(1)) as f64
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

/// One tappable button: a rect that carries the action, and its label.
fn button(x: f64, label: &str, action: &str) -> [Value; 2] {
    [
        json!({
            "op": "rect",
            "x": x, "y": 88.0, "w": 11.0, "h": 9.0,
            "fill": "sunken", "stroke": "border", "sw": 0.4, "r": 1.5,
            "tap": action
        }),
        json!({
            "op": "text",
            "x": x + 5.5, "y": 92.5, "s": label,
            "fill": "text", "size": 5.0, "align": "center"
        }),
    ]
}

fn render(gears: &Gears) -> String {
    let mut ops = vec![json!({
        "op": "path",
        "d": path_data(gears),
        "stroke": "accent",
        "sw": 0.5
    })];
    for (i, (label, action)) in [
        ("R-", "R-"),
        ("R+", "R+"),
        ("r-", "r-"),
        ("r+", "r+"),
        ("d-", "d-"),
        ("d+", "d+"),
    ]
    .iter()
    .enumerate()
    {
        ops.extend(button(3.0 + (i as f64) * 15.5, label, action));
    }

    json!({
        "$slim": "scene/1",
        "width": 100,
        "height": 100,
        "background": "surface",
        "ops": ops,
        "state": gears.state(),
        "controls": ["random", "reset"],
        "status": format!(
            "R {} · r {} · d {} · {} lobes",
            gears.outer,
            gears.inner,
            gears.pen,
            gears.turns() as i64
        ),
    })
    .to_string()
}
