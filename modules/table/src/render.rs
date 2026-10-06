//! Aligns CSV/TSV into a monospace table. A plain-text module: the output is
//! just a string, which slim shows in its monospace result panel - no scene.

/// The most rows and columns we lay out; past this the output would flood the
/// shared result, so extra input is dropped and the output says how much.
const MAX_ROWS: usize = 200;
const MAX_COLS: usize = 32;

pub fn apply(command: &str, input: &str) -> Result<String, String> {
    match command {
        "format" => format(input),
        other => Err(format!("unknown command: {other}")),
    }
}

fn format(input: &str) -> Result<String, String> {
    let parsed = parse(input);
    if parsed.rows.is_empty() {
        return Err("give me some CSV or TSV - a header row and some data".to_string());
    }
    let mut out = render(&parsed.rows);
    if parsed.dropped_rows > 0 {
        out.push_str(&format!("\n... {} more rows not shown", parsed.dropped_rows));
    }
    if parsed.dropped_cols > 0 {
        out.push_str(&format!("\n... {} more columns not shown", parsed.dropped_cols));
    }
    Ok(out)
}

struct Parsed {
    rows: Vec<Vec<String>>,
    dropped_rows: usize,
    dropped_cols: usize,
}

/// Splits into rows of trimmed fields. The delimiter is a tab if the header
/// line has one, otherwise a comma - the two common pastes, without asking. The
/// slash command cannot carry a real newline, so a typed `\n` stands for one
/// when the text has none.
fn parse(input: &str) -> Parsed {
    let input = if input.contains('\n') { input.to_string() } else { input.replace("\\n", "\n") };
    let mut lines = input.lines().filter(|l| !l.trim().is_empty());
    let first = lines.next();
    let delim = if first.is_some_and(|l| l.contains('\t')) { '\t' } else { ',' };
    let all: Vec<Vec<String>> = first.into_iter().chain(lines).map(|l| split_fields(l, delim)).collect();
    let dropped_rows = all.len().saturating_sub(MAX_ROWS);
    let dropped_cols = all.iter().map(|r| r.len().saturating_sub(MAX_COLS)).max().unwrap_or(0);
    let rows = all
        .into_iter()
        .take(MAX_ROWS)
        .map(|mut r| {
            r.truncate(MAX_COLS);
            r
        })
        .collect();
    Parsed { rows, dropped_rows, dropped_cols }
}

/// One line into trimmed fields, honouring double quotes and `""` inside them.
fn split_fields(line: &str, delim: char) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' if quoted || field.trim().is_empty() => {
                quoted = !quoted;
                if !quoted {
                    field = field.trim_start().to_string();
                }
            }
            c if c == delim && !quoted => fields.push(std::mem::take(&mut field)),
            c => field.push(c),
        }
    }
    fields.push(field);
    fields.into_iter().map(|f| f.trim().to_string()).collect()
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
    s.parse::<f64>().is_ok_and(f64::is_finite)
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

    #[test]
    fn the_documented_slash_example_makes_two_rows() {
        let out = apply("format", r"name,age\nalice,30").unwrap();
        assert_eq!(out.lines().count(), 3, "{out}");
        assert_eq!(out.lines().nth(2).unwrap(), "alice |  30");
    }

    #[test]
    fn a_literal_backslash_n_is_left_alone_when_there_are_real_newlines() {
        let out = apply("format", "a,b\nx\\ny,2").unwrap();
        assert!(out.contains(r"x\ny"), "{out}");
    }

    #[test]
    fn a_quoted_csv_field_stays_one_cell() {
        let out = apply("format", "name,age\n\"Smith, John\",30\n\"say \"\"hi\"\"\",4").unwrap();
        assert_eq!(out.lines().nth(2).unwrap(), "Smith, John |  30");
        assert_eq!(out.lines().nth(3).unwrap(), "say \"hi\"    |   4");
    }

    #[test]
    fn a_tab_inside_a_later_row_does_not_switch_the_delimiter() {
        let out = apply("format", "a,b\n\"x\ty\",2").unwrap();
        assert!(out.lines().next().unwrap().starts_with("a "), "{out}");
        assert_eq!(out.lines().count(), 3);
    }

    #[test]
    fn nan_and_infinity_words_are_text_not_numbers() {
        let out = apply("format", "word\nnan\ninfinity").unwrap();
        assert_eq!(out.lines().nth(2).unwrap(), "nan");
    }

    #[test]
    fn dropped_rows_and_columns_are_announced() {
        let many_rows: String = (0..250).map(|i| format!("{i}\n")).collect();
        let out = apply("format", &many_rows).unwrap();
        assert!(out.ends_with("... 50 more rows not shown"), "{out}");
        let wide = (0..40).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        let out = apply("format", &wide).unwrap();
        assert!(out.ends_with("... 8 more columns not shown"), "{out}");
    }

    #[test]
    fn a_table_within_the_limits_has_no_note() {
        assert!(!apply("format", "a,b\n1,2").unwrap().contains("not shown"));
    }
}
