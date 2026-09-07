//! Aligns CSV/TSV into a monospace table. A plain-text module: the output is
//! just a string, which slim shows in its monospace result panel - no scene.

/// The most rows and columns we lay out; past this the point is lost and the
/// output would flood the shared result, so extra input is dropped.
const MAX_ROWS: usize = 200;
const MAX_COLS: usize = 32;

pub fn apply(command: &str, input: &str) -> Result<String, String> {
    match command {
        "format" => format(input),
        other => Err(format!("unknown command: {other}")),
    }
}

fn format(input: &str) -> Result<String, String> {
    let rows = parse(input);
    if rows.is_empty() {
        return Err("give me some CSV or TSV - a header row and some data".to_string());
    }
    Ok(render(&rows))
}

/// Splits into rows of trimmed fields. The delimiter is a tab if any line has
/// one, otherwise a comma - the two common pastes, without asking.
fn parse(input: &str) -> Vec<Vec<String>> {
    let delim = if input.contains('\t') { '\t' } else { ',' };
    input
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(MAX_ROWS)
        .map(|line| {
            line.split(delim)
                .take(MAX_COLS)
                .map(|f| f.trim().to_string())
                .collect()
        })
        .collect()
}

fn render(rows: &[Vec<String>]) -> String {
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let widths: Vec<usize> = (0..cols)
        .map(|c| {
            rows.iter()
                .map(|r| r.get(c).map_or(0, |s| s.chars().count()))
                .max()
                .unwrap_or(0)
        })
        .collect();
    // A column is right-aligned when every data cell in it is a number, so figures line up on the right.
    let numeric: Vec<bool> = (0..cols)
        .map(|c| {
            rows.iter().skip(1).any(|r| cell(r, c).map(is_number).unwrap_or(false))
                && rows.iter().skip(1).all(|r| {
                    cell(r, c).map(|s| s.is_empty() || is_number(s)).unwrap_or(true)
                })
        })
        .collect();

    let mut out = String::new();
    for (i, row) in rows.iter().enumerate() {
        let line: Vec<String> = (0..cols)
            .map(|c| pad(cell(row, c).unwrap_or(""), widths[c], numeric[c] && i > 0))
            .collect();
        out.push_str(line.join(" | ").trim_end());
        out.push('\n');
        // A rule under the header row separates it from the data.
        if i == 0 {
            let rule: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
            out.push_str(rule.join("-+-").trim_end());
            out.push('\n');
        }
    }
    out.trim_end().to_string()
}

fn cell<'a>(row: &'a [String], c: usize) -> Option<&'a str> {
    row.get(c).map(String::as_str)
}

fn pad(s: &str, width: usize, right: bool) -> String {
    let gap = width.saturating_sub(s.chars().count());
    let fill = " ".repeat(gap);
    if right {
        format!("{fill}{s}")
    } else {
        format!("{s}{fill}")
    }
}

fn is_number(s: &str) -> bool {
    !s.is_empty() && s.parse::<f64>().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_is_an_error_not_a_panic() {
        assert!(apply("format", "  \n ").is_err());
    }

    #[test]
    fn aligns_columns_with_a_header_rule() {
        let out = apply("format", "name,city\nalice,portland\nbo,reno").unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "name  | city");
        assert_eq!(lines[1], "------+---------");
        assert_eq!(lines[2], "alice | portland");
        assert_eq!(lines[3], "bo    | reno");
    }

    #[test]
    fn numeric_columns_are_right_aligned() {
        let out = apply("format", "name,age\nalice,30\nbo,9").unwrap();
        let lines: Vec<&str> = out.lines().collect();
        // The "age" header sets the column to width 3, so numbers right-align in it: the 9 lines up under the 0 of 30.
        assert_eq!(lines[2], "alice |  30");
        assert_eq!(lines[3], "bo    |   9");
    }

    #[test]
    fn tabs_win_over_commas_when_present() {
        let out = apply("format", "a\tb,c\n1\t2,3").unwrap();
        // Split on tab, so "b,c" is one cell.
        assert!(out.lines().next().unwrap().starts_with("a | b,c"));
    }

    #[test]
    fn ragged_rows_are_padded() {
        let out = apply("format", "a,b,c\n1\n2,3").unwrap();
        // Three columns throughout; a short row just leaves trailing blanks (trimmed).
        assert_eq!(out.lines().count(), 4);
    }
}
