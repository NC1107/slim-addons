//! Parses dice notation and produces the individual rolls plus the total.

use crate::notation::{self, Notation};
use crate::rng::SplitMix64;

/// Rolls the dice notation in `input`, returning text like
/// `Rolls: 14, 7\nModifier: +3\nTotal: 24`.
pub fn roll(input: &str) -> Result<String, String> {
    let Notation {
        count,
        sides,
        modifier,
    } = notation::parse(input)?;

    let mut rng = SplitMix64::from_bytes(input.as_bytes());
    let rolls: Vec<u32> = (0..count).map(|_| rng.roll(sides)).collect();
    let rolls_sum: i64 = rolls.iter().map(|&r| r as i64).sum();
    let total = rolls_sum + modifier as i64;

    let rolls_text = rolls
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(", ");

    let mut out = format!("Rolls: {rolls_text}");
    if modifier != 0 {
        out.push_str(&format!("\nModifier: {modifier:+}"));
    }
    out.push_str(&format!("\nTotal: {total}"));
    Ok(out)
}
