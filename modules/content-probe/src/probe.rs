//! The actual logic, kept separate from the ABI shim in `lib.rs` so it can be
//! unit-tested as plain Rust (see `docs/modules/building-modules.md`
//! "Testing a module").
//!
//! Two commands:
//!
//! - `probe` - describes `input` (byte length, char count, and counts of the
//!   character classes most likely to break a naive markdown/lexer/JSON
//!   pipeline) rather than doing anything with it.
//! - `reflect` - returns `input` byte-for-byte, to prove what survives the
//!   `{command, input}` JSON round trip unchanged.
//!
//! Any other command is an error, same as a module that does not recognise
//! it at all.

/// Unicode bidi control characters that can reorder how surrounding text
/// renders (RTL/LTR overrides and isolates). A code block containing one of
/// these can make displayed source disagree with byte order - the classic
/// "trojan source" trick.
const BIDI_CONTROLS: [char; 9] = [
    '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}', '\u{2066}', '\u{2067}',
    '\u{2068}', '\u{2069}',
];

/// Characters that render as nothing (or as a stray marker) but still count
/// as content: zero-width joiners/spaces and a byte-order mark.
const ZERO_WIDTH: [char; 4] = ['\u{200B}', '\u{200C}', '\u{200D}', '\u{FEFF}'];

pub fn apply(command: &str, input: &str) -> Result<String, String> {
    match command {
        "probe" => Ok(describe(input)),
        "reflect" => Ok(input.to_string()),
        // Fuel-cost calibration commands: isolate the cost of one thing at a
        // time so a fuel boundary can be attributed rather than guessed at.
        "count_bytes" => Ok(format!("bytes={}", input.len())),
        "count_chars" => Ok(format!("chars={}", input.chars().count())),
        other => Err(format!("no such command: {other}")),
    }
}

fn describe(input: &str) -> String {
    let bytes = input.len();
    let chars = input.chars().count();
    let utf16_units = input.encode_utf16().count();
    let has_nul = input.contains('\0');
    let control = input
        .chars()
        .filter(|c| {
            let n = *c as u32;
            (n < 0x20 && *c != '\n' && *c != '\t' && *c != '\r') || n == 0x7f
        })
        .count();
    let bidi = input.chars().filter(|c| BIDI_CONTROLS.contains(c)).count();
    let zero_width = input.chars().filter(|c| ZERO_WIDTH.contains(c)).count();
    let lines = input.lines().count();
    let longest_line = input.lines().map(str::len).max().unwrap_or(0);

    let head: String = input.chars().take(24).flat_map(char::escape_default).collect();
    let tail: String = {
        let total = chars;
        let skip = total.saturating_sub(24);
        input.chars().skip(skip).flat_map(char::escape_default).collect()
    };

    format!(
        "bytes={bytes} chars={chars} utf16_units={utf16_units} lines={lines} \
         longest_line_bytes={longest_line} has_nul={has_nul} control_chars={control} \
         bidi_controls={bidi} zero_width={zero_width}\nhead=\"{head}\"\ntail=\"{tail}\""
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_reports_plain_ascii_with_no_flags() {
        let out = apply("probe", "console.log(1)").unwrap();
        assert!(out.contains("bytes=14"));
        assert!(out.contains("chars=14"));
        assert!(out.contains("has_nul=false"));
        assert!(out.contains("control_chars=0"));
        assert!(out.contains("bidi_controls=0"));
    }

    #[test]
    fn probe_counts_an_embedded_nul_byte() {
        let out = apply("probe", "a\0b").unwrap();
        assert!(out.contains("bytes=3"));
        assert!(out.contains("chars=3"));
        assert!(out.contains("has_nul=true"));
        // NUL itself is a control character but is explicitly not one of the
        // three we exempt (\n \t \r), so it is counted.
        assert!(out.contains("control_chars=1"));
    }

    #[test]
    fn probe_flags_a_bidi_override_used_to_disguise_source() {
        // The "trojan source" shape: an RTL override hidden in what looks
        // like an ordinary comment, reordering how later code renders.
        let payload = "// normal \u{202E} evil-looking-reversed-tail";
        let out = apply("probe", payload).unwrap();
        assert!(out.contains("bidi_controls=1"), "{out}");
    }

    #[test]
    fn probe_flags_zero_width_characters() {
        let out = apply("probe", "adm\u{200B}in").unwrap();
        assert!(out.contains("zero_width=1"), "{out}");
        assert!(out.contains("chars=6"), "{out}");
    }

    #[test]
    fn probe_distinguishes_bytes_from_chars_on_multibyte_input() {
        // Each of these is one char but multiple UTF-8 bytes.
        let out = apply("probe", "日本語").unwrap();
        assert!(out.contains("chars=3"), "{out}");
        assert!(out.contains("bytes=9"), "{out}");
    }

    #[test]
    fn reflect_returns_input_byte_for_byte() {
        let payload = "line one\nline two\t\u{202E}\0end";
        assert_eq!(apply("reflect", payload).unwrap(), payload);
    }

    #[test]
    fn unknown_command_is_a_clean_error_not_a_default() {
        assert!(apply("wat", "x").is_err());
    }

    #[test]
    fn empty_input_probes_cleanly() {
        let out = apply("probe", "").unwrap();
        assert!(out.contains("bytes=0"));
        assert!(out.contains("chars=0"));
    }
}
