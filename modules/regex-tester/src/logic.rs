//! Shows every match of a regex in some text, with its capture groups.

use regex::RegexBuilder;

const MAX_INPUT: usize = 20_000;
const MAX_MATCHES: usize = 50;
const MAX_SHOWN_CHARS: usize = 60;
const COMPILED_SIZE_LIMIT: usize = 1 << 20;

pub fn apply(command: &str, input: &str) -> Result<String, String> {
    match command {
        "test" => test(input),
        other => Err(format!("unknown command: {other}")),
    }
}

/// The pattern is the first line; the rest is the text. A first line with ` :: `
/// in it is the one-line form, and everything after the separator is the text.
fn split_input(input: &str) -> Result<(&str, &str), String> {
    let first_line = input.split('\n').next().unwrap_or("");
    let split = if first_line.contains(" :: ") { input.split_once(" :: ") } else { input.split_once('\n') };
    let (pattern, subject) = split
        .ok_or("give me a pattern, then the text on the next line (or pattern :: text)")?;
    let pattern = pattern.trim_end_matches('\r');
    if pattern.is_empty() {
        return Err("the pattern is empty".to_string());
    }
    Ok((pattern, subject))
}

fn test(input: &str) -> Result<String, String> {
    if input.len() > MAX_INPUT {
        return Err(format!("too long: keep it under {MAX_INPUT} bytes"));
    }
    let (pattern, subject) = split_input(input)?;
    let re = RegexBuilder::new(pattern)
        .size_limit(COMPILED_SIZE_LIMIT)
        .build()
        .map_err(|e| format!("bad pattern: {}", e.to_string().lines().last().unwrap_or("invalid")))?;

    let total = re.captures_iter(subject).count();
    if total == 0 {
        return Ok("no matches".to_string());
    }
    let names: Vec<Option<&str>> = re.capture_names().collect();
    let mut out = vec![format!("{total} {}", if total == 1 { "match" } else { "matches" })];
    for (n, caps) in re.captures_iter(subject).take(MAX_MATCHES).enumerate() {
        let whole = caps.get(0).expect("group 0 always matches");
        out.push(format!("{}: {} at {}", n + 1, show(whole.as_str()), char_range(subject, whole.range())));
        for (i, name) in names.iter().enumerate().skip(1) {
            let label = name.map_or_else(|| i.to_string(), str::to_string);
            let value = caps.get(i).map_or_else(|| "(unset)".to_string(), |m| show(m.as_str()));
            out.push(format!("   {label} = {value}"));
        }
    }
    if total > MAX_MATCHES {
        out.push(format!("... and {} more", total - MAX_MATCHES));
    }
    Ok(out.join("\n"))
}

fn show(text: &str) -> String {
    let clipped: String = text.chars().take(MAX_SHOWN_CHARS).collect();
    let dots = if text.chars().count() > MAX_SHOWN_CHARS { "..." } else { "" };
    format!("\"{}{dots}\"", clipped.escape_debug())
}

fn char_range(subject: &str, bytes: std::ops::Range<usize>) -> String {
    let start = subject[..bytes.start].chars().count();
    format!("{start}..{}", start + subject[bytes].chars().count())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(input: &str) -> String {
        apply("test", input).unwrap()
    }

    #[test]
    fn lists_each_match_with_its_position() {
        assert_eq!(run("\\d+\na1 b22 c333"), "3 matches\n1: \"1\" at 1..2\n2: \"22\" at 4..6\n3: \"333\" at 8..11");
    }

    #[test]
    fn shows_numbered_and_named_groups() {
        let out = run("(?P<key>\\w+)=(\\d+)\nx=1");
        assert_eq!(out, "1 match\n1: \"x=1\" at 0..3\n   key = \"x\"\n   2 = \"1\"");
    }

    #[test]
    fn an_optional_group_that_did_not_take_part_is_unset() {
        assert!(run("a(b)?\na").ends_with("   1 = (unset)"));
    }

    #[test]
    fn the_one_line_form_splits_on_the_first_separator() {
        assert_eq!(run("o+ :: foo boo"), "2 matches\n1: \"oo\" at 1..3\n2: \"oo\" at 5..7");
    }

    #[test]
    fn positions_count_characters_not_bytes() {
        assert!(run("b\naéb").contains("at 2..3"));
    }

    #[test]
    fn no_match_says_so() {
        assert_eq!(run("z\nabc"), "no matches");
    }

    #[test]
    fn a_bad_pattern_is_an_error_not_a_panic() {
        assert!(apply("test", "(\nabc").unwrap_err().starts_with("bad pattern"));
    }

    #[test]
    fn a_pattern_that_compiles_too_big_is_refused() {
        assert!(apply("test", "(\\w{100}){100}\nabc").unwrap_err().starts_with("bad pattern"));
    }

    #[test]
    fn matches_are_capped() {
        let out = run(&format!("a\n{}", "a".repeat(80)));
        assert!(out.starts_with("80 matches") && out.ends_with("... and 30 more"));
    }

    #[test]
    fn input_without_a_subject_asks_for_one() {
        assert!(apply("test", "abc").is_err());
    }

    #[test]
    fn the_one_line_form_keeps_newlines_in_the_subject() {
        assert!(run("\\d+ :: a1\nb2").starts_with("2 matches"));
    }

    #[test]
    fn a_separator_on_a_later_line_does_not_split_the_two_line_form() {
        assert_eq!(run("a\nx :: a"), "1 match\n1: \"a\" at 5..6");
    }

    #[test]
    fn the_size_limit_is_worded_in_bytes() {
        let err = apply("test", &format!("a\n{}", "e".repeat(MAX_INPUT))).unwrap_err();
        assert!(err.contains("bytes"), "{err}");
    }
}
