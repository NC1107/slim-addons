//! A step sequencer you write a tune into, in the channel, together.
//!
//! One frame per interaction, the shape `tic-tac-toe` established: the pattern
//! rides in the scene's opaque `state`, a tap arrives as an action, and the
//! module answers with the next frame. So the tune is shared - everybody
//! looking at the message sees the same grid, and a step somebody else placed
//! is there when you press play.
//!
//! ## The pitches are slim's own
//!
//! [`LADDER`] is copied from the deployment's `assets/audio/notifications/sounds.py`,
//! the pentatonic set the seven notification chimes are built from. Two
//! consequences, both wanted: a tune written here sounds like the product
//! rather than like a tone generator, and a pentatonic grid is very hard to
//! make sound wrong in, which is what a toy people poke at in a chat needs.
//!
//! ## Why one note per step
//!
//! The host caps a scene at 32 notes (`NotesOp.maxNotes`), so a polyphonic
//! 16-step grid could ask for more than would ever play and would be silently
//! truncated mid-tune. Monophonic keeps the worst case at 16, which always
//! plays whole. It also makes the grid legible: one lit cell per column.

use serde::Deserialize;
use serde_json::{json, Value};

/// slim's own pentatonic ladder, A4 up. Index 0 is the lowest pitch, which is
/// the *bottom* row on screen.
const LADDER: [f64; ROWS] = [
    440.0,  // A4
    523.25, // C5
    587.33, // D5
    659.25, // E5
    783.99, // G5
    880.0,  // A5
    1046.50, // C6
    1174.66, // D6
];

const ROWS: usize = 8;
const STEPS: usize = 16;

/// Seconds per step, slowest first. Sixteen steps at the slowest setting is
/// 4.8s, inside the host's 8s ceiling on how late a note may start
/// (`NotesOp.maxSceneSeconds`) with room to spare.
const TEMPOS: [f64; 3] = [0.30, 0.22, 0.16];

const TEMPO_NAMES: [&str; 3] = ["slow", "medium", "fast"];

/// A note rings most of its step rather than all of it, so consecutive steps
/// on one pitch are heard as two notes instead of one held tone.
const RING: f64 = 0.85;

const CELL: f64 = 10.0;

pub fn apply(command: &str, input: &str) -> Result<String, String> {
    match command {
        "sequence" => Ok(sequence(input)),
        other => Err(format!("unknown command: {other}")),
    }
}

#[derive(Deserialize)]
struct Action {
    action: String,
    #[serde(default)]
    state: String,
}

/// What the frame this action produced should make the person who caused it
/// hear: nothing, one note they just placed, or the whole tune.
enum Sound {
    Silent,
    One(usize),
    Tune,
}

fn sequence(input: &str) -> String {
    let trimmed = input.trim();
    // The launch runs with empty input: an empty grid.
    if trimmed.is_empty() {
        return render(&Pattern::new(), Sound::Silent, true);
    }
    let (action, mut pattern) = match serde_json::from_str::<Action>(trimmed) {
        Ok(a) => (a.action, Pattern::from_state(&a.state)),
        Err(_) => (String::new(), Pattern::new()),
    };

    let sound = match action.as_str() {
        // Not "play": slim reserves that for its own animation loop, which
        // repeatedly sends `step`. See docs/modules/building-modules.md.
        "play tune" => Sound::Tune,
        "clear" => {
            pattern.clear();
            Sound::Silent
        }
        "tempo" => {
            pattern.next_tempo();
            Sound::Silent
        }
        // A cell's tap action is "c:<row>,<col>"; see the cells op in the scene contract.
        cell if cell.starts_with("c:") => match parse_cell(&cell[2..]) {
            Some((row, col)) => pattern.toggle(row, col),
            None => Sound::Silent,
        },
        _ => Sound::Silent,
    };
    render(&pattern, sound, false)
}

/// `"<row>,<col>"`, or None for anything else. A malformed action is ignored
/// rather than answered with an error: the frame it would replace is a working
/// grid, and losing it to a bad tap would be the worse outcome.
fn parse_cell(raw: &str) -> Option<(usize, usize)> {
    let (row, col) = raw.split_once(',')?;
    let row = row.trim().parse::<usize>().ok()?;
    let col = col.trim().parse::<usize>().ok()?;
    (row < ROWS && col < STEPS).then_some((row, col))
}

struct Pattern {
    /// The pitch each step plays, as an index into [`LADDER`], or None for a
    /// step that is silent.
    steps: [Option<usize>; STEPS],
    tempo: usize,
}

impl Pattern {
    fn new() -> Self {
        Pattern {
            steps: [None; STEPS],
            tempo: 1,
        }
    }

    /// Rebuilds from the opaque `state` this module wrote last frame: one char
    /// per step (`.` or a pitch digit), a `|`, then the tempo digit.
    fn from_state(s: &str) -> Self {
        let (steps, tempo) = s.split_once('|').unwrap_or((s, "1"));
        let mut pattern = Pattern::new();
        for (i, c) in steps.chars().take(STEPS).enumerate() {
            if let Some(pitch) = c.to_digit(10) {
                let pitch = pitch as usize;
                if pitch < ROWS {
                    pattern.steps[i] = Some(pitch);
                }
            }
        }
        if let Some(t) = tempo.chars().next().and_then(|c| c.to_digit(10)) {
            if (t as usize) < TEMPOS.len() {
                pattern.tempo = t as usize;
            }
        }
        pattern
    }

    fn state(&self) -> String {
        let mut out = String::with_capacity(STEPS + 2);
        for step in &self.steps {
            match step {
                Some(pitch) => out.push((b'0' + *pitch as u8) as char),
                None => out.push('.'),
            }
        }
        out.push('|');
        out.push((b'0' + self.tempo as u8) as char);
        out
    }

    /// Row 0 is the top of the grid and the *highest* pitch, so the ladder is
    /// read upside down on purpose: pitch rising up the screen is the one
    /// convention every sequencer and every stave already shares.
    fn pitch_of_row(row: usize) -> usize {
        ROWS - 1 - row
    }

    /// Places a pitch on a step, or clears the step if that pitch was already
    /// the one there. Only the placement is worth hearing back.
    fn toggle(&mut self, row: usize, col: usize) -> Sound {
        let pitch = Self::pitch_of_row(row);
        if self.steps[col] == Some(pitch) {
            self.steps[col] = None;
            return Sound::Silent;
        }
        self.steps[col] = Some(pitch);
        Sound::One(pitch)
    }

    fn clear(&mut self) {
        self.steps = [None; STEPS];
    }

    fn next_tempo(&mut self) {
        self.tempo = (self.tempo + 1) % TEMPOS.len();
    }

    fn step_seconds(&self) -> f64 {
        TEMPOS[self.tempo]
    }

    fn filled(&self) -> usize {
        self.steps.iter().filter(|s| s.is_some()).count()
    }
}

/// The grid as one `cells` op: a digit per cell indexing the palette, read
/// left to right and top to bottom.
///
/// Palette index 2 is an off cell on a downbeat (every fourth step). It is a
/// different shade rather than a drawn line because `cells` fills the whole
/// scene and nothing can be drawn between its cells - and the beat guide is
/// what makes a 16-step grid countable at a glance.
fn grid_data(pattern: &Pattern) -> String {
    let mut data = String::with_capacity(ROWS * STEPS);
    for row in 0..ROWS {
        let pitch = Pattern::pitch_of_row(row);
        for col in 0..STEPS {
            let lit = pattern.steps[col] == Some(pitch);
            data.push(if lit {
                '1'
            } else if col % 4 == 0 {
                '2'
            } else {
                '0'
            });
        }
    }
    data
}

/// The notes this frame asks to be played. Empty for a frame nobody should
/// hear, which is most of them.
///
/// The host only ever plays these for the viewer whose own action produced the
/// frame, never for somebody watching the shared scene update, so a tune is
/// heard by whoever pressed play and by nobody else.
fn notes_for(pattern: &Pattern, sound: Sound) -> Vec<Value> {
    let step = pattern.step_seconds();
    match sound {
        Sound::Silent => Vec::new(),
        Sound::One(pitch) => vec![json!({"f": LADDER[pitch], "t": 0.0, "d": step * RING})],
        Sound::Tune => pattern
            .steps
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                s.map(|pitch| {
                    json!({"f": LADDER[pitch], "t": i as f64 * step, "d": step * RING})
                })
            })
            .collect(),
    }
}

fn render(pattern: &Pattern, sound: Sound, fresh: bool) -> String {
    let mut ops: Vec<Value> = vec![json!({
        "op": "cells",
        "cols": STEPS,
        "rows": ROWS,
        "data": grid_data(pattern),
        "palette": ["sunken", "accent", "surface"],
        "gap": 0.12,
        "tap": "c",
    })];

    let notes = notes_for(pattern, sound);
    if !notes.is_empty() {
        ops.push(json!({"op": "notes", "notes": notes}));
    }

    let filled = pattern.filled();
    let tempo = TEMPO_NAMES[pattern.tempo];
    let status = if fresh || filled == 0 {
        format!("tap the grid to write a tune, then press play tune ({tempo})")
    } else if filled == 1 {
        format!("1 note, {tempo}")
    } else {
        format!("{filled} notes, {tempo}")
    };

    json!({
        "$slim": "scene/1",
        "width": STEPS as f64 * CELL,
        "height": ROWS as f64 * CELL,
        "background": "surface",
        "ops": ops,
        "status": status,
        "controls": ["play tune", "tempo", "clear"],
        "state": pattern.state(),
        "live": true,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene_of(input: &str) -> Value {
        serde_json::from_str(&apply("sequence", input).unwrap()).unwrap()
    }

    fn notes_in(scene: &Value) -> Vec<Value> {
        scene["ops"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|op| op["op"] == "notes")
            .flat_map(|op| op["notes"].as_array().unwrap().clone())
            .collect()
    }

    #[test]
    fn a_launch_is_an_empty_grid_that_makes_no_sound() {
        let scene = scene_of("");
        assert_eq!(scene["$slim"], "scene/1");
        assert_eq!(scene["state"], "................|1");
        assert!(
            notes_in(&scene).is_empty(),
            "a scene must never make a noise nobody asked for"
        );
    }

    #[test]
    fn state_round_trips() {
        let mut p = Pattern::new();
        p.toggle(0, 3);
        p.toggle(7, 15);
        p.next_tempo();
        let back = Pattern::from_state(&p.state());
        assert_eq!(back.steps, p.steps);
        assert_eq!(back.tempo, p.tempo);
    }

    #[test]
    fn junk_state_reads_as_an_empty_grid_rather_than_failing() {
        let p = Pattern::from_state("!!!!!!|9");
        assert!(p.steps.iter().all(|s| s.is_none()));
        assert_eq!(p.tempo, 1, "an out-of-range tempo falls back");
    }

    #[test]
    fn the_top_row_is_the_highest_pitch() {
        let mut p = Pattern::new();
        p.toggle(0, 0);
        assert_eq!(p.steps[0], Some(ROWS - 1));
        p.toggle(ROWS - 1, 1);
        assert_eq!(p.steps[1], Some(0));
        assert!(LADDER[ROWS - 1] > LADDER[0]);
    }

    #[test]
    fn tapping_a_cell_places_it_and_plays_that_one_note_back() {
        let scene = scene_of(r#"{"action":"c:0,0","state":"................|1"}"#);
        assert_eq!(scene["state"], "7...............|1");
        let notes = notes_in(&scene);
        assert_eq!(notes.len(), 1, "the note just placed is worth hearing");
        assert_eq!(notes[0]["f"], LADDER[ROWS - 1]);
        assert_eq!(notes[0]["t"], 0.0);
    }

    #[test]
    fn tapping_the_lit_cell_again_clears_it_silently() {
        let scene = scene_of(r#"{"action":"c:0,0","state":"7...............|1"}"#);
        assert_eq!(scene["state"], "................|1");
        assert!(
            notes_in(&scene).is_empty(),
            "removing a note is not a note to play"
        );
    }

    #[test]
    fn a_second_pitch_in_one_step_replaces_the_first() {
        let mut p = Pattern::new();
        p.toggle(0, 4);
        p.toggle(3, 4);
        assert_eq!(p.steps[4], Some(Pattern::pitch_of_row(3)));
        assert_eq!(p.filled(), 1, "a step holds one pitch, never two");
    }

    #[test]
    fn play_lays_every_step_out_in_time() {
        let scene = scene_of(r#"{"action":"play tune","state":"7..0............|1"}"#);
        let notes = notes_in(&scene);
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0]["t"], 0.0);
        assert_eq!(notes[1]["t"], 3.0 * TEMPOS[1]);
        assert_eq!(notes[1]["f"], LADDER[0]);
    }

    #[test]
    fn a_full_pattern_stays_inside_the_hosts_ceilings() {
        let full = "0123456701234567|0";
        let scene = scene_of(&format!(r#"{{"action":"play tune","state":"{full}"}}"#));
        let notes = notes_in(&scene);
        assert_eq!(notes.len(), STEPS);
        assert!(
            notes.len() <= 32,
            "the host silently drops notes past 32, which would truncate a tune"
        );
        let last = notes.last().unwrap();
        let end = last["t"].as_f64().unwrap() + last["d"].as_f64().unwrap();
        assert!(
            end <= 8.0,
            "the host clamps a note starting past 8s, so a tune must finish inside it: {end}"
        );
        for note in &notes {
            let f = note["f"].as_f64().unwrap();
            assert!((20.0..=7800.0).contains(&f), "outside the host's audible band");
            assert!(note["d"].as_f64().unwrap() <= 3.0);
        }
    }

    #[test]
    fn tempo_cycles_and_survives_in_the_state() {
        let scene = scene_of(r#"{"action":"tempo","state":"7...............|1"}"#);
        assert_eq!(scene["state"], "7...............|2");
        assert!(notes_in(&scene).is_empty());
        let wrapped = scene_of(r#"{"action":"tempo","state":"7...............|2"}"#);
        assert_eq!(wrapped["state"], "7...............|0");
    }

    #[test]
    fn clear_empties_the_grid_and_keeps_the_tempo() {
        let scene = scene_of(r#"{"action":"clear","state":"70123456........|2"}"#);
        assert_eq!(scene["state"], "................|2");
    }

    #[test]
    fn an_unknown_or_malformed_action_leaves_the_tune_alone() {
        for action in ["c:9,0", "c:0,99", "c:nonsense", "wat", ""] {
            let scene = scene_of(&format!(
                r#"{{"action":"{action}","state":"7...............|1"}}"#
            ));
            assert_eq!(
                scene["state"], "7...............|1",
                "action {action} must not disturb the pattern"
            );
            assert!(notes_in(&scene).is_empty());
        }
    }

    #[test]
    fn the_grid_marks_every_fourth_step_so_it_can_be_counted() {
        let p = Pattern::new();
        let data = grid_data(&p);
        assert_eq!(data.len(), ROWS * STEPS);
        let first_row = &data[0..STEPS];
        assert_eq!(first_row.chars().nth(0), Some('2'));
        assert_eq!(first_row.chars().nth(3), Some('0'));
        assert_eq!(first_row.chars().nth(4), Some('2'));
    }

    #[test]
    fn a_lit_cell_draws_lit_in_its_own_row() {
        let mut p = Pattern::new();
        p.toggle(2, 5);
        let data = grid_data(&p);
        assert_eq!(data.chars().nth(2 * STEPS + 5), Some('1'));
        assert_eq!(data.chars().nth(5), Some('0'), "only its own row lights up");
    }

    #[test]
    fn an_unknown_command_is_refused() {
        assert!(apply("nope", "").is_err());
    }
}
