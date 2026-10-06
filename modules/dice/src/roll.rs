//! Parses dice notation and produces the individual rolls plus the total.

use crate::notation::{self, Notation};
use crate::rng::SplitMix64;

/// Rolls the dice notation in `input`, echoing the notation so a shared or
/// posted result says what was rolled, e.g. `2d20+3 = 30\nRolls: 9, 18, modifier +3`.
pub fn roll(input: &str, entropy: &str) -> Result<String, String> {
    let Notation {
        count,
        sides,
        modifier,
    } = notation::parse(input)?;

    let mut rng = SplitMix64::seeded(entropy.as_bytes(), input.as_bytes());
    let rolls: Vec<u32> = (0..count).map(|_| rng.roll(sides)).collect();
    let rolls_sum: i64 = rolls.iter().map(|&r| r as i64).sum();
    let total = rolls_sum + modifier as i64;

    let rolls_text = rolls
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(", ");

    let notation_text = if modifier != 0 {
        format!("{count}d{sides}{modifier:+}")
    } else {
        format!("{count}d{sides}")
    };

    let mut out = format!("{notation_text} = {total}\nRolls: {rolls_text}");
    if modifier != 0 {
        out.push_str(&format!(", modifier {modifier:+}"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::roll;

    #[test]
    fn a_roll_varies_with_the_runs_entropy() {
        let outputs: std::collections::BTreeSet<String> = (0..40)
            .map(|i| roll("d20", &format!("run-{i}")).unwrap())
            .collect();
        assert!(
            outputs.len() > 5,
            "40 runs gave only {} distinct rolls",
            outputs.len()
        );
    }

    #[test]
    fn a_roll_without_entropy_is_still_valid_notation_output() {
        let out = roll("2d6+1", "").unwrap();
        assert!(out.starts_with("2d6+1 = "), "{out}");
    }

    #[test]
    fn a_bad_notation_is_still_refused() {
        assert!(roll("banana", "x").is_err());
    }
}
