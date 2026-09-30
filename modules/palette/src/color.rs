//! Hex <-> HSL and the palette schemes built from a seed colour.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsl {
    pub h: f64,
    pub s: f64,
    pub l: f64,
}

pub const SCHEMES: [&str; 4] = ["analogous", "complementary", "triad", "shades"];

/// Accepts `#abc`, `abc`, `#aabbcc` and `aabbcc`, returning lowercase six digits.
pub fn normalize_hex(text: &str) -> Option<String> {
    let digits = text.trim().trim_start_matches('#').to_ascii_lowercase();
    if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    match digits.len() {
        3 => Some(digits.chars().flat_map(|c| [c, c]).collect()),
        6 => Some(digits),
        _ => None,
    }
}

pub fn to_hsl(hex: &str) -> Hsl {
    let channel = |i: usize| f64::from(u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0)) / 255.0;
    let (r, g, b) = (channel(0), channel(2), channel(4));
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d == 0.0 {
        return Hsl { h: 0.0, s: 0.0, l };
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    Hsl { h: h * 60.0, s, l }
}

pub fn to_hex(c: Hsl) -> String {
    let h = c.h.rem_euclid(360.0);
    let s = c.s.clamp(0.0, 1.0);
    let l = c.l.clamp(0.0, 1.0);
    let chroma = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = chroma * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let m = l - chroma / 2.0;
    let byte = |v: f64| ((v + m) * 255.0).round() as u8;
    format!("{:02x}{:02x}{:02x}", byte(r), byte(g), byte(b))
}

fn shifted(c: Hsl, hue: f64, light: f64) -> String {
    to_hex(Hsl { h: c.h + hue, s: c.s, l: c.l + light })
}

/// The swatches (six-digit hex, no `#`) for a scheme; an unknown scheme is analogous.
pub fn palette(seed: &str, scheme: &str) -> Vec<String> {
    let c = to_hsl(seed);
    match scheme {
        "complementary" => vec![shifted(c, 0.0, 0.0), shifted(c, 180.0, 0.0), shifted(c, 0.0, 0.25), shifted(c, 180.0, 0.25)],
        "triad" => vec![shifted(c, 0.0, 0.0), shifted(c, 120.0, 0.0), shifted(c, 240.0, 0.0)],
        "shades" => [-0.3, -0.15, 0.0, 0.15, 0.3].iter().map(|d| shifted(c, 0.0, *d)).collect(),
        _ => [-60.0, -30.0, 0.0, 30.0, 60.0].iter().map(|d| shifted(c, *d, 0.0)).collect(),
    }
}
