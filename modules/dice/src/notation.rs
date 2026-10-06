//! Parses dice notation of the form `NdM`, `NdM+K` or `NdM-K` (e.g. `2d20+3`).
//! `N` defaults to 1 when omitted (`d20` means `1d20`).

pub struct Notation {
    pub count: u32,
    pub sides: u32,
    pub modifier: i32,
}

const MAX_COUNT: u32 = 1000;
const MAX_SIDES: u32 = 1_000_000;

/// Parses `text` as dice notation, trimming surrounding whitespace and
/// accepting either case for the `d` separator.
pub fn parse(text: &str) -> Result<Notation, String> {
    let text = text.trim();
    let usage = "expected dice notation like 2d20+3";

    let lower = text.to_ascii_lowercase();
    let d_index = lower.find('d').ok_or_else(|| usage.to_string())?;
    let (count_part, rest) = text.split_at(d_index);
    let rest = &rest[1..]; // skip the 'd'

    let count: u32 = if count_part.is_empty() {
        1
    } else {
        count_part
            .parse()
            .map_err(|_| format!("invalid dice count {count_part:?}: {usage}"))?
    };

    let (sides_part, modifier) = split_modifier(rest);
    if sides_part.is_empty() {
        return Err(format!("missing side count: {usage}"));
    }
    let sides: u32 = sides_part
        .parse()
        .map_err(|_| format!("invalid side count {sides_part:?}: {usage}"))?;

    let modifier = match modifier {
        Some(text) => signed(text).parse().map_err(|_| format!("invalid modifier {text:?}: {usage}"))?,
        None => 0,
    };

    if count == 0 || count > MAX_COUNT {
        return Err(format!("dice count must be between 1 and {MAX_COUNT}"));
    }
    if sides == 0 || sides > MAX_SIDES {
        return Err(format!("side count must be between 1 and {MAX_SIDES}"));
    }

    Ok(Notation {
        count,
        sides,
        modifier,
    })
}

/// Drops the spaces between a modifier's sign and its digits, so `+ 3` reads as `+3`.
fn signed(text: &str) -> String {
    let (sign, digits) = text.split_at(1);
    format!("{sign}{}", digits.trim_start())
}

/// Splits `rest` at the first `+`/`-` that follows the side-count digits,
/// returning the side count text and, if present, the signed modifier text.
fn split_modifier(rest: &str) -> (&str, Option<&str>) {
    match rest.find(['+', '-']) {
        Some(idx) => (rest[..idx].trim_end(), Some(rest[idx..].trim_end())),
        None => (rest.trim_end(), None),
    }
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn spaces_around_the_modifier_are_accepted() {
        for text in ["2d20 + 3", "2d20 +3", "2d20+ 3", "2d20 - 3"] {
            let n = parse(text).unwrap_or_else(|e| panic!("{text:?}: {e}"));
            assert_eq!((n.count, n.sides, n.modifier.abs()), (2, 20, 3), "{text:?}");
        }
        assert_eq!(parse("2d20 - 3").unwrap().modifier, -3);
    }

    #[test]
    fn spaces_inside_the_digits_are_still_an_error() {
        assert!(parse("2d2 0").is_err());
        assert!(parse("2d20+ 1 2").is_err());
    }
}
