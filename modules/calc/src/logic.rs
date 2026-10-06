//! Runs a calc block: one expression or `name = expression` per line.

use std::collections::BTreeMap;

use crate::expr;

const MAX_INPUT: usize = 8_000;
const MAX_LINES: usize = 200;

pub fn apply(command: &str, input: &str) -> Result<String, String> {
    match command {
        "eval" => eval(input),
        other => Err(format!("unknown command: {other}")),
    }
}

fn eval(input: &str) -> Result<String, String> {
    if input.len() > MAX_INPUT {
        return Err(format!("too long: keep it under {MAX_INPUT} characters"));
    }
    let mut vars: BTreeMap<String, f64> = BTreeMap::new();
    let mut out = Vec::new();
    for (n, raw) in input.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if out.len() >= MAX_LINES {
            return Err(format!("too many lines: the limit is {MAX_LINES}"));
        }
        let (name, source) = split_assignment(line);
        let value = expr::evaluate(source, &vars).map_err(|e| format!("line {}: {e}", n + 1))?;
        let shown = expr::format_number(value);
        vars.insert("ans".to_string(), value);
        match name {
            Some(name) => {
                vars.insert(name.to_string(), value);
                out.push(format!("{name} = {shown}"));
            }
            None => out.push(format!("{line} = {shown}")),
        }
    }
    if out.is_empty() {
        return Err("give me an expression, like 2 * (3 + 4)".to_string());
    }
    Ok(out.join("\n"))
}

fn split_assignment(line: &str) -> (Option<&str>, &str) {
    match line.split_once('=') {
        Some((name, rest)) if is_name(name.trim()) => (Some(name.trim()), rest),
        _ => (None, line),
    }
}

fn is_name(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && s != "ans"
        && !expr::is_reserved(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(input: &str) -> String {
        apply("eval", input).unwrap()
    }

    #[test]
    fn precedence_and_parentheses() {
        assert_eq!(run("2 + 3 * 4"), "2 + 3 * 4 = 14");
        assert_eq!(run("(2 + 3) * 4"), "(2 + 3) * 4 = 20");
    }

    #[test]
    fn power_is_right_associative_and_binds_tighter_than_negation() {
        assert_eq!(run("2 ^ 3 ^ 2"), "2 ^ 3 ^ 2 = 512");
        assert_eq!(run("-2 ^ 2"), "-2 ^ 2 = -4");
    }

    #[test]
    fn variables_and_ans_carry_between_lines() {
        assert_eq!(run("r = 3\narea = pi * r ^ 2\nans / 2"), "r = 3\narea = 28.2743338823\nans / 2 = 14.1371669412");
    }

    #[test]
    fn functions_and_constants() {
        assert_eq!(run("sqrt(16) + max(1, 7, 3)"), "sqrt(16) + max(1, 7, 3) = 11");
        assert_eq!(run("round(2.5) + floor(-0.5)"), "round(2.5) + floor(-0.5) = 2");
    }

    #[test]
    fn comments_and_blank_lines_are_skipped() {
        assert_eq!(run("# tax\n\n100 * 1.2"), "100 * 1.2 = 120");
    }

    #[test]
    fn errors_name_the_line() {
        assert_eq!(apply("eval", "1\n2 +").unwrap_err(), "line 2: expected a number, name or ( after +");
        assert_eq!(apply("eval", "1 / 0").unwrap_err(), "line 1: division by zero");
        assert_eq!(apply("eval", "x + 1").unwrap_err(), "line 1: unknown name x");
    }

    #[test]
    fn deep_nesting_is_refused_not_a_stack_overflow() {
        let deep = format!("{}1{}", "(".repeat(500), ")".repeat(500));
        assert!(apply("eval", &deep).unwrap_err().contains("too deeply nested"));
    }

    #[test]
    fn empty_input_asks_for_an_expression() {
        assert!(apply("eval", "  \n").is_err());
    }

    #[test]
    fn big_and_small_numbers_use_exponent_form() {
        assert_eq!(run("10 ^ 20"), "10 ^ 20 = 1e20");
        assert_eq!(run("1 / 3 / 1000000000000"), "1 / 3 / 1000000000000 = 3.33333333e-13");
    }

    #[test]
    fn an_overflow_inside_an_expression_is_an_error_not_a_quiet_number() {
        for source in ["1 / 10 ^ 400", "min(0 * 10 ^ 400, 5)", "max(10 ^ 400 - 10 ^ 400, 5)"] {
            let err = apply("eval", source).unwrap_err();
            assert!(err.contains("not a finite number"), "{source}: {err}");
        }
    }
}
