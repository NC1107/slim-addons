//! Parses dice notation and produces the individual rolls plus the total.

use crate::notation::{self, Notation};
use crate::rng::SplitMix64;

/// Rolls the dice notation in `input`, echoing the notation so a shared or
/// posted result says what was rolled, e.g. `2d20+3 = 30\nRolls: 9, 18, modifier +3`.
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

    fn rolls_of(out: &str) -> Vec<i64> {
        let line = out.lines().nth(1).unwrap().strip_prefix("Rolls: ").unwrap();
        line.split(", ").filter(|p| !p.starts_with("modifier")).map(|p| p.parse().unwrap()).collect()
    }

    #[test]
    fn the_total_is_the_rolls_plus_the_modifier() {
        for input in ["2d6+3", "3d8-2", "1d20", "10d4+0"] {
            let out = roll(input).unwrap();
            let total: i64 = out.lines().next().unwrap().rsplit(" = ").next().unwrap().parse().unwrap();
            let modifier: i64 = input.split_once(['+', '-']).map_or(0, |(_, m)| m.parse().unwrap());
            let modifier = if input.contains('-') { -modifier } else { modifier };
            assert_eq!(total, rolls_of(&out).iter().sum::<i64>() + modifier, "{input}: {out}");
        }
    }

    #[test]
    fn every_die_lands_within_its_sides() {
        let out = roll("50d6").unwrap();
        assert_eq!(rolls_of(&out).len(), 50);
        assert!(rolls_of(&out).iter().all(|r| (1..=6).contains(r)), "{out}");
    }

    #[test]
    fn the_notation_is_echoed_with_a_signed_modifier() {
        assert!(roll("d20").unwrap().starts_with("1d20 = "));
        assert!(roll("2D6 - 1").unwrap().starts_with("2d6-1 = "));
        assert!(roll("2d6+3").unwrap().ends_with(", modifier +3"));
        assert!(!roll("2d6").unwrap().contains("modifier"));
    }

    #[test]
    fn the_same_input_rolls_the_same() {
        assert_eq!(roll("4d10+1").unwrap(), roll("4d10+1").unwrap());
    }
}
