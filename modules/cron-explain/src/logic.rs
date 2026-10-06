//! Explains a five-field cron schedule field by field, and lists upcoming runs.

use crate::field::{self, FIELDS};
use crate::schedule::{parse_stamp, Cron};

const MAX_INPUT: usize = 200;
const RUNS_SHOWN: usize = 5;

pub fn apply(command: &str, input: &str) -> Result<String, String> {
    match command {
        "explain" => explain(input),
        other => Err(format!("unknown command: {other}")),
    }
}

fn explain(input: &str) -> Result<String, String> {
    if input.len() > MAX_INPUT {
        return Err("too long for a cron expression".to_string());
    }
    let (expr, from) = match input.split_once(" @ ") {
        Some((e, f)) => (e.trim(), Some(f)),
        None => (input.trim(), None),
    };
    let expr = alias(expr);
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() != 5 {
        return Err("a cron expression has five fields: minute hour day-of-month month day-of-week".to_string());
    }
    let mut parsed = Vec::new();
    for (text, spec) in parts.iter().zip(FIELDS.iter()) {
        parsed.push(field::parse(text, spec)?);
    }
    let mut lines: Vec<String> = parsed
        .iter()
        .zip(FIELDS.iter())
        .map(|(f, spec)| format!("{:<13}{}", spec.label, field::describe(f, spec)))
        .collect();
    let mut it = parsed.into_iter();
    let cron = Cron {
        minute: it.next().unwrap(),
        hour: it.next().unwrap(),
        dom: it.next().unwrap(),
        month: it.next().unwrap(),
        dow: it.next().unwrap(),
    };
    if let Some(from) = from {
        let stamp = parse_stamp(from)?;
        let runs = cron.next_runs(&stamp, RUNS_SHOWN);
        lines.push(format!("\nnext runs after {} UTC", stamp.show()));
        if runs.is_empty() {
            lines.push("  none in the next eight years".to_string());
        }
        lines.extend(runs.iter().map(|r| format!("  {}", r.show())));
    }
    Ok(lines.join("\n"))
}

fn alias(expr: &str) -> String {
    match expr {
        "@yearly" | "@annually" => "0 0 1 1 *",
        "@monthly" => "0 0 1 * *",
        "@weekly" => "0 0 * * 0",
        "@daily" | "@midnight" => "0 0 * * *",
        "@hourly" => "0 * * * *",
        other => other,
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(input: &str) -> String {
        apply("explain", input).unwrap()
    }

    #[test]
    fn explains_each_field() {
        let out = run("*/15 9-17 * * 1-5");
        assert_eq!(
            out,
            "minute       0, 15, 30, 45\nhour         9 through 17\nday of month every day of month\nmonth        every month\nday of week  Monday through Friday"
        );
    }

    #[test]
    fn names_and_lists_work() {
        let out = run("30 8 1,15 jan,jul mon");
        assert!(out.contains("month        January, July"));
        assert!(out.contains("day of week  Monday"));
    }

    #[test]
    fn aliases_expand() {
        assert_eq!(run("@daily"), run("0 0 * * *"));
    }

    #[test]
    fn sunday_can_be_zero_or_seven() {
        assert_eq!(run("0 0 * * 7"), run("0 0 * * 0"));
    }

    #[test]
    fn next_runs_are_computed_from_the_given_time() {
        let out = run("0 9 * * 1-5 @ 2026-01-02 09:00");
        assert!(out.ends_with("next runs after 2026-01-02 09:00 UTC\n  2026-01-05 09:00\n  2026-01-06 09:00\n  2026-01-07 09:00\n  2026-01-08 09:00\n  2026-01-09 09:00"));
    }

    #[test]
    fn a_leap_day_schedule_skips_to_the_next_leap_year() {
        let out = run("0 0 29 2 * @ 2026-03-01 00:00");
        assert!(out.contains("  2028-02-29 00:00"));
    }

    #[test]
    fn an_impossible_date_ends_the_search() {
        assert!(run("0 0 31 2 * @ 2026-01-01 00:00").contains("none in the next eight years"));
    }

    #[test]
    fn day_of_month_and_weekday_match_either() {
        let out = run("0 0 13 * 5 @ 2026-02-01 00:00");
        assert!(out.contains("  2026-02-06 00:00"));
        assert!(out.contains("  2026-02-13 00:00"));
    }

    #[test]
    fn bad_fields_name_the_field() {
        assert!(apply("explain", "61 * * * *").unwrap_err().starts_with("minute field"));
        assert!(apply("explain", "* * * *").unwrap_err().contains("five fields"));
        assert!(apply("explain", "*/0 * * * *").unwrap_err().contains("step"));
        assert!(apply("explain", "5-1 * * * *").unwrap_err().contains("backwards"));
    }

    #[test]
    fn a_bad_start_time_is_an_error() {
        assert!(apply("explain", "* * * * * @ tomorrow").is_err());
        assert!(apply("explain", "* * * * * @ 2026-02-30 00:00").is_err());
    }

    #[test]
    fn a_starred_day_of_month_still_has_to_match_the_weekday() {
        let out = run("0 0 */2 * 1 @ 2026-02-01 00:00");
        let runs: Vec<&str> = out.lines().skip_while(|l| !l.starts_with("next runs")).skip(1).map(str::trim).collect();
        assert_eq!(runs, ["2026-02-09 00:00", "2026-02-23 00:00", "2026-03-09 00:00", "2026-03-23 00:00", "2026-04-13 00:00"]);
    }

    #[test]
    fn a_starred_day_of_month_is_still_described_by_its_values() {
        assert!(run("0 0 */2 * 1").contains("day of month 1, 3, 5, 7"));
    }
}
