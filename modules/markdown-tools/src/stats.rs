//! Word count, character count and estimated reading time for markdown text.
//!
//! Reading time assumes 200 words per minute, rounded up to the nearest
//! minute (a partial minute still shows as 1, unless there are no words).

const WORDS_PER_MINUTE: usize = 200;

/// Summarizes `input` as `Words: N`, `Characters: N`, `Reading time: N min`.
pub fn summarize(input: &str) -> String {
    let words = input.split_whitespace().count();
    let characters = input.chars().count();
    let minutes = reading_minutes(words);

    format!("Words: {words}\nCharacters: {characters}\nReading time: {minutes} min")
}

fn reading_minutes(words: usize) -> usize {
    if words == 0 {
        return 0;
    }
    words.div_ceil(WORDS_PER_MINUTE).max(1)
}
