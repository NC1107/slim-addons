//! Minesweeper: a nine-by-nine board with ten mines. Every frame is a fresh
//! sandboxed call - the board rides in the scene's opaque `state`, a tap
//! arrives as an action, and the module returns the next frame. No host
//! storage, no memory between calls.
//!
//! **The mines are not in the state.** Only a seed is, and the field is
//! rebuilt from it on every frame. Two reasons. A state that carried the
//! mine positions would hand the answer to anyone who read it, which in a
//! channel where the state is shared is the whole game. And the state stays
//! short: a seed and the revealed/flagged masks, rather than a third grid.
//!
//! The first tap is always safe. The seed is chosen at launch, before any
//! tap, so it cannot be steered by where somebody clicked - instead the
//! opening tap re-rolls the seed until that cell and its neighbours are
//! clear, which is what every implementation of this game does and is why a
//! first click never ends it.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::rng::SplitMix64;

pub const SIDE: usize = 9;
pub const CELLS: usize = SIDE * SIDE;
pub const MINES: usize = 10;

/// How many re-rolls the opening tap may spend looking for a seed whose
/// three-by-three neighbourhood is clear. Ten mines in eighty-one cells finds
/// one almost immediately; the bound only stops a pathological search.
const OPENING_ATTEMPTS: u32 = 64;

pub fn apply(command: &str, input: &str) -> Result<String, String> {
    match command {
        "play" => Ok(play(input)),
        other => Err(format!("unknown command: {other}")),
    }
}

fn play(input: &str) -> String {
    let trimmed = input.trim();
    // The launch runs with empty input: start a fresh board.
    if trimmed.is_empty() {
        return render(&Board::new(seed_from(trimmed)));
    }
    let (action, mut board) = match serde_json::from_str::<Action>(trimmed) {
        Ok(a) => {
            let board = Board::from_state(&a.state, &a.action);
            (a.action, board)
        }
        Err(_) => (String::new(), Board::new(seed_from(trimmed))),
    };
    match action.as_str() {
        "new game" | "new" => board = Board::new(board.seed.wrapping_add(0x9E37_79B9)),
        a if a.starts_with("flag") || a.starts_with("dig") => {
            board.flagging = !board.flagging
        }
        // A cell's tappable rect carries tap "c<index>".
        cell if cell.starts_with('c') => {
            if let Ok(i) = cell[1..].parse::<usize>() {
                board.tap(i);
            }
        }
        _ => {}
    }
    render(&board)
}

#[derive(Deserialize)]
struct Action {
    action: String,
    #[serde(default)]
    state: String,
}

/// A seed for a board nobody has played yet. Folded from whatever the caller
/// sent so two launches in a channel are not the same board.
fn seed_from(input: &str) -> u64 {
    SplitMix64::from_bytes(input.as_bytes()).next_u64()
}

pub struct Board {
    pub seed: u64,
    pub revealed: [bool; CELLS],
    pub flagged: [bool; CELLS],
    /// Whether taps place flags instead of revealing. A mode rather than a
    /// second gesture: the scene contract has one tap and no long press, so
    /// flagging needs somewhere to live.
    pub flagging: bool,
    pub dead: bool,
}

impl Board {
    pub fn new(seed: u64) -> Self {
        Board {
            seed,
            revealed: [false; CELLS],
            flagged: [false; CELLS],
            flagging: false,
            dead: false,
        }
    }

    /// Rebuilds from the opaque state written last frame:
    /// `seed|revealed|flagged|flags-mode|dead`, the two masks as run-free
    /// strings of `0`/`1`.
    ///
    /// `action` is needed because an opening tap has to re-roll the seed, and
    /// that decision belongs to the frame that has not yet revealed anything.
    pub fn from_state(state: &str, action: &str) -> Self {
        let mut parts = state.split('|');
        let seed = parts
            .next()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or_else(|| seed_from(state));
        let revealed = mask_from(parts.next().unwrap_or(""));
        let flagged = mask_from(parts.next().unwrap_or(""));
        let flagging = parts.next() == Some("1");
        let dead = parts.next() == Some("1");
        let mut board = Board {
            seed,
            revealed,
            flagged,
            flagging,
            dead,
        };
        // Nothing revealed yet and a reveal incoming: this is the opening tap,
        // so pick a seed that makes it safe.
        if !board.flagging && !board.dead && board.revealed.iter().all(|r| !r) {
            if let Some(index) = action.strip_prefix('c').and_then(|i| i.parse::<usize>().ok()) {
                board.reseed_for_safe_opening(index);
            }
        }
        board
    }

    pub fn state(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}",
            self.seed,
            mask_to(&self.revealed),
            mask_to(&self.flagged),
            u8::from(self.flagging),
            u8::from(self.dead),
        )
    }

    /// Re-rolls until `index` and everything around it is clear, so a first
    /// tap opens a space rather than ending the game.
    fn reseed_for_safe_opening(&mut self, index: usize) {
        if index >= CELLS {
            return;
        }
        for _ in 0..OPENING_ATTEMPTS {
            let mines = mines_for(self.seed);
            let clear = !mines[index]
                && neighbours(index)
                    .into_iter()
                    .flatten()
                    .all(|n| !mines[n]);
            if clear {
                return;
            }
            self.seed = self.seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        }
    }

    pub fn mines(&self) -> [bool; CELLS] {
        mines_for(self.seed)
    }

    pub fn adjacent(&self, index: usize) -> u8 {
        let mines = self.mines();
        neighbours(index)
            .into_iter()
            .flatten()
            .filter(|&n| mines[n])
            .count() as u8
    }

    pub fn tap(&mut self, index: usize) {
        if index >= CELLS || self.dead || self.won() {
            return;
        }
        if self.flagging {
            // Flagging a revealed cell would mark something already known.
            if !self.revealed[index] {
                self.flagged[index] = !self.flagged[index];
            }
            return;
        }
        // A flag is a deliberate "do not touch": honour it rather than
        // letting a stray tap detonate what the player marked.
        if self.flagged[index] || self.revealed[index] {
            return;
        }
        if self.mines()[index] {
            self.dead = true;
            self.revealed[index] = true;
            return;
        }
        self.flood(index);
    }

    /// Reveals `index`, and keeps going through any neighbour that also
    /// touches no mine - the cascade that makes the game playable.
    fn flood(&mut self, index: usize) {
        let mines = self.mines();
        let mut stack = vec![index];
        while let Some(current) = stack.pop() {
            if self.revealed[current] || self.flagged[current] || mines[current] {
                continue;
            }
            self.revealed[current] = true;
            if self.adjacent(current) != 0 {
                continue;
            }
            for n in neighbours(current).into_iter().flatten() {
                if !self.revealed[n] {
                    stack.push(n);
                }
            }
        }
    }

    /// Won when every cell that is not a mine has been revealed. Flags are
    /// deliberately not part of it: a player who cleared the board without
    /// marking a single mine has still cleared the board.
    pub fn won(&self) -> bool {
        let mines = self.mines();
        (0..CELLS).all(|i| mines[i] || self.revealed[i])
    }

    pub fn flags_left(&self) -> i32 {
        MINES as i32 - self.flagged.iter().filter(|f| **f).count() as i32
    }
}

/// The mine field for a seed. Deterministic, so every frame rebuilds exactly
/// the same board without carrying it in the state.
fn mines_for(seed: u64) -> [bool; CELLS] {
    let mut rng = SplitMix64::from_seed(seed);
    let mut mines = [false; CELLS];
    let mut placed = 0;
    while placed < MINES {
        let at = (rng.next_u64() % CELLS as u64) as usize;
        if !mines[at] {
            mines[at] = true;
            placed += 1;
        }
    }
    mines
}

/// The up-to-eight cells around `index`, as an array with None for the ones
/// off the edge. An array rather than a Vec: this runs for every cell of
/// every frame and allocating eighty-one times a frame is avoidable.
fn neighbours(index: usize) -> [Option<usize>; 8] {
    let mut out = [None; 8];
    if index >= CELLS {
        return out;
    }
    let (col, row) = ((index % SIDE) as isize, (index / SIDE) as isize);
    let mut slot = 0;
    for dr in -1isize..=1 {
        for dc in -1isize..=1 {
            if dr == 0 && dc == 0 {
                continue;
            }
            let (c, r) = (col + dc, row + dr);
            if c >= 0 && r >= 0 && c < SIDE as isize && r < SIDE as isize {
                out[slot] = Some(r as usize * SIDE + c as usize);
            }
            slot += 1;
        }
    }
    out
}

fn mask_to(mask: &[bool; CELLS]) -> String {
    mask.iter().map(|b| if *b { '1' } else { '0' }).collect()
}

fn mask_from(s: &str) -> [bool; CELLS] {
    let mut mask = [false; CELLS];
    for (i, c) in s.chars().take(CELLS).enumerate() {
        mask[i] = c == '1';
    }
    mask
}

/// What the mode button should say next. It names the mode it switches *to*,
/// which is what a button does, and it means the current mode is readable
/// without hunting for the status line.
fn flag_control(flagging: bool) -> &'static str {
    if flagging {
        "dig mode"
    } else {
        "flag mode"
    }
}

const CELL: f64 = 22.0;

/// The count colours. Only the names in `resolveSceneColor` exist - there is
/// no success or warning - and an unknown name does not fail, it silently
/// falls back, which is how the first cut of this shipped looking fine and
/// rendering flat. Low counts read calm, high counts read loud.
fn count_colour(count: u8) -> &'static str {
    match count {
        1 => "accent",
        2 => "muted",
        3 => "danger",
        _ => "text",
    }
}

fn render(board: &Board) -> String {
    let size = SIDE as f64 * CELL;
    let mut ops: Vec<Value> = Vec::new();
    let mines = board.mines();
    let dead = board.dead;
    let won = board.won();
    let over = dead || won;

    for index in 0..CELLS {
        let (col, row) = ((index % SIDE) as f64, (index / SIDE) as f64);
        let (x, y) = (col * CELL, row * CELL);
        let revealed = board.revealed[index];

        if revealed || (over && mines[index]) {
            // A revealed cell is flat; an unexploded mine shown at the end is
            // marked so the board explains itself rather than just stopping.
            ops.push(json!({
                "op": "rect", "x": x + 1.0, "y": y + 1.0,
                "w": CELL - 2.0, "h": CELL - 2.0,
                "fill": "bg", "r": 2.0,
            }));
            if mines[index] {
                ops.push(json!({
                    "op": "circle", "cx": x + CELL / 2.0, "cy": y + CELL / 2.0,
                    "r": CELL / 2.0 - 6.0,
                    "fill": if revealed && dead { "danger" } else { "muted" },
                }));
            } else {
                let count = board.adjacent(index);
                if count > 0 {
                    ops.push(json!({
                        "op": "text", "x": x + CELL / 2.0, "y": y + CELL / 2.0,
                        "s": count.to_string(),
                        "fill": count_colour(count),
                        "align": "center",
                    }));
                }
            }
            continue;
        }

        // Unrevealed: a raised cell, tappable while the game is on.
        let mut cell = json!({
            "op": "rect", "x": x + 1.0, "y": y + 1.0,
            "w": CELL - 2.0, "h": CELL - 2.0,
            "fill": "sunken", "r": 2.0,
            "stroke": if board.flagging { "danger" } else { "border" },
            "sw": if board.flagging { 1.5 } else { 0.5 },
        });
        if !over {
            cell["tap"] = json!(format!("c{index}"));
        }
        ops.push(cell);

        if board.flagged[index] {
            // A filled marker rather than a glyph: it has to read at
            // twenty-two pixels on a phone, where a character does not.
            ops.push(json!({
                "op": "circle", "cx": x + CELL / 2.0, "cy": y + CELL / 2.0,
                "r": CELL / 2.0 - 7.0, "fill": "danger",
            }));
        }
    }

    let status = if dead {
        "boom - new game to try again".to_string()
    } else if won {
        "cleared".to_string()
    } else if board.flagging {
        format!("flagging - {} to place", board.flags_left())
    } else {
        format!("{} mines left", board.flags_left())
    };

    json!({
        "$slim": "scene/1",
        "width": size,
        "height": size,
        "background": "surface",
        "ops": ops,
        "status": status,
        // The flag control says which mode it will put you in, so the board
        // does not depend on the status line to tell you what a tap will do.
        "controls": [flag_control(board.flagging), "new game"],
        "state": board.state(),
        "live": true,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A board whose mines are known, so a test can aim at one.
    fn board_with_known_field() -> (Board, [bool; CELLS]) {
        let board = Board::new(12_345);
        let mines = board.mines();
        (board, mines)
    }

    fn first_mine(mines: &[bool; CELLS]) -> usize {
        (0..CELLS).find(|&i| mines[i]).expect("a field has mines")
    }

    fn first_safe(mines: &[bool; CELLS]) -> usize {
        (0..CELLS).find(|&i| !mines[i]).expect("a field has blanks")
    }

    #[test]
    fn a_field_has_exactly_ten_mines() {
        for seed in [0u64, 1, 12_345, u64::MAX] {
            let mines = mines_for(seed);
            assert_eq!(
                mines.iter().filter(|m| **m).count(),
                MINES,
                "seed {seed} placed the wrong number of mines",
            );
        }
    }

    #[test]
    fn the_same_seed_always_gives_the_same_field() {
        assert_eq!(mines_for(999), mines_for(999));
    }

    #[test]
    fn different_seeds_give_different_fields() {
        assert_ne!(mines_for(1), mines_for(2));
    }

    #[test]
    fn the_state_never_carries_the_mines() {
        let (mut board, mines) = board_with_known_field();
        board.tap(first_safe(&mines));
        let state = board.state();

        // The field written out the way the masks are. If this ever appears in
        // the state, anyone who can read the state can read the mines - which
        // in a shared channel is the entire game.
        let field = mask_to(&mines);
        assert!(
            !state.contains(&field),
            "the mine field leaked into the state",
        );

        // And structurally: seed, two masks, two flags. A third grid would
        // show up here as an extra part.
        let parts: Vec<&str> = state.split('|').collect();
        assert_eq!(parts.len(), 5, "state shape changed: {state}");
        assert_eq!(parts[1].len(), CELLS, "revealed mask is one bit per cell");
        assert_eq!(parts[2].len(), CELLS, "flagged mask is one bit per cell");
    }

    #[test]
    fn state_round_trips() {
        let (mut board, mines) = board_with_known_field();
        board.tap(first_safe(&mines));
        board.flagging = true;
        board.tap(first_mine(&mines));
        let again = Board::from_state(&board.state(), "");
        assert_eq!(again.seed, board.seed);
        assert_eq!(again.revealed, board.revealed);
        assert_eq!(again.flagged, board.flagged);
        assert_eq!(again.flagging, board.flagging);
    }

    #[test]
    fn tapping_a_mine_ends_the_game() {
        let (mut board, mines) = board_with_known_field();
        board.tap(first_mine(&mines));
        assert!(board.dead);
    }

    #[test]
    fn a_dead_board_takes_no_further_taps() {
        let (mut board, mines) = board_with_known_field();
        board.tap(first_mine(&mines));
        let revealed_before = board.revealed;
        board.tap(first_safe(&mines));
        assert_eq!(board.revealed, revealed_before);
    }

    #[test]
    fn a_flagged_cell_cannot_be_detonated_by_a_stray_tap() {
        let (mut board, mines) = board_with_known_field();
        let mine = first_mine(&mines);
        board.flagging = true;
        board.tap(mine);
        board.flagging = false;
        board.tap(mine);
        assert!(!board.dead, "a flag is a do-not-touch, not a decoration");
    }

    #[test]
    fn flagging_a_revealed_cell_does_nothing() {
        let (mut board, mines) = board_with_known_field();
        let safe = first_safe(&mines);
        board.tap(safe);
        board.flagging = true;
        board.tap(safe);
        assert!(!board.flagged[safe]);
    }

    #[test]
    fn revealing_a_blank_cascades_past_its_neighbours() {
        // A zero-adjacency cell must open more than itself, or the game is
        // unplayable one square at a time.
        let board_seed = (0..2000u64)
            .find(|&seed| {
                let b = Board::new(seed);
                (0..CELLS).any(|i| !b.mines()[i] && b.adjacent(i) == 0)
            })
            .expect("some seed has an empty cell");
        let mut board = Board::new(board_seed);
        let blank = (0..CELLS)
            .find(|&i| !board.mines()[i] && board.adjacent(i) == 0)
            .unwrap();
        board.tap(blank);
        assert!(
            board.revealed.iter().filter(|r| **r).count() > 1,
            "revealing an empty cell must open its neighbourhood",
        );
    }

    #[test]
    fn the_cascade_never_reveals_a_mine() {
        for seed in 0..200u64 {
            let mut board = Board::new(seed);
            let mines = board.mines();
            if let Some(blank) = (0..CELLS).find(|&i| !mines[i] && board.adjacent(i) == 0) {
                board.tap(blank);
                assert!(
                    (0..CELLS).all(|i| !(mines[i] && board.revealed[i])),
                    "seed {seed} cascaded onto a mine",
                );
            }
        }
    }

    #[test]
    fn an_opening_tap_is_always_safe() {
        // Every cell, over many seeds: the first tap must never end the game.
        for seed in 0..60u64 {
            let fresh = Board::new(seed);
            for index in [0usize, 4, 40, 80] {
                let mut board =
                    Board::from_state(&fresh.state(), &format!("c{index}"));
                board.tap(index);
                assert!(
                    !board.dead,
                    "seed {seed} killed an opening tap on cell {index}",
                );
            }
        }
    }

    #[test]
    fn the_opening_tap_opens_a_space_rather_than_a_single_square() {
        let fresh = Board::new(7);
        let mut board = Board::from_state(&fresh.state(), "c40");
        board.tap(40);
        assert!(
            board.revealed.iter().filter(|r| **r).count() > 1,
            "a safe opening clears its neighbourhood, which is why it is safe",
        );
    }

    #[test]
    fn clearing_every_safe_cell_wins() {
        let mut board = Board::new(4_242);
        let mines = board.mines();
        for i in 0..CELLS {
            if !mines[i] {
                board.revealed[i] = true;
            }
        }
        assert!(board.won());
        assert!(!board.dead);
    }

    #[test]
    fn a_won_board_takes_no_further_taps() {
        let mut board = Board::new(4_242);
        let mines = board.mines();
        for i in 0..CELLS {
            if !mines[i] {
                board.revealed[i] = true;
            }
        }
        board.tap(first_mine(&mines));
        assert!(!board.dead, "the game was already over");
    }

    /// The bug that shipped in 0.1.0: the renderer reads a text op's content
    /// from `s`, and this module wrote `text`, so every number was dropped and
    /// the board rendered blank. Unit tests passed and the module ran fine
    /// under wasmi - nothing compared the emitted ops against the client.
    #[test]
    fn revealed_counts_are_emitted_where_the_renderer_reads_them() {
        let fresh = Board::new(2_024);
        let mut board = Board::from_state(&fresh.state(), "c40");
        board.tap(40);
        // Open more of the board so a numbered cell is certain to be on screen.
        for i in 0..CELLS {
            if !board.mines()[i] && board.adjacent(i) > 0 && !board.flagged[i] {
                board.revealed[i] = true;
            }
        }
        let scene = render(&board);
        assert!(
            scene.contains(r#""s":"#),
            "counts must be written to the key the renderer reads",
        );
        assert!(
            !scene.contains(r#""op":"text","#) || !scene.contains(r#""text":""#),
            "a text op carrying its content under `text` is dropped on the floor",
        );
    }

    /// Only the nine names in `resolveSceneColor` exist. An unknown one does
    /// not fail, it silently falls back - which is how success/warning shipped.
    #[test]
    fn every_colour_named_is_one_the_renderer_knows() {
        const KNOWN: [&str; 9] = [
            "bg", "surface", "sunken", "accent", "accent-soft", "muted", "text",
            "border", "danger",
        ];
        let mut board = Board::new(31);
        board.flagging = true;
        board.tap(0);
        board.flagging = false;
        board.revealed[1] = true;
        let scenes = [render(&board), render(&Board::new(7)), apply("play", "").unwrap()];
        for scene in scenes {
            let parsed: serde_json::Value = serde_json::from_str(&scene).unwrap();
            for op in parsed["ops"].as_array().unwrap() {
                for key in ["fill", "stroke"] {
                    if let Some(name) = op[key].as_str() {
                        assert!(
                            name.starts_with('#') || KNOWN.contains(&name),
                            "{key} {name:?} is not a scene colour; it falls back silently",
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn play_launches_a_scene_with_every_cell_hidden() {
        let scene = apply("play", "").unwrap();
        assert!(scene.contains(r#""$slim":"scene/1""#));
        assert!(scene.contains("10 mines left"));
        assert!(scene.contains(r#""tap":"c0""#));
    }

    #[test]
    fn the_flag_control_toggles_the_mode() {
        let fresh = Board::new(3);
        let out = apply("play", &format!(r#"{{"action":"flag","state":"{}"}}"#, fresh.state()))
            .unwrap();
        assert!(out.contains("flagging"));
    }

    #[test]
    fn new_game_produces_a_different_board() {
        let fresh = Board::new(11);
        let out = apply(
            "play",
            &format!(r#"{{"action":"new game","state":"{}"}}"#, fresh.state()),
        )
        .unwrap();
        assert!(out.contains("10 mines left"));
        assert!(
            !out.contains(&format!(r#""state":"{}"#, fresh.seed)),
            "a new game must not hand back the same field",
        );
    }

    #[test]
    fn a_finished_board_offers_no_taps() {
        let (mut board, mines) = board_with_known_field();
        board.tap(first_mine(&mines));
        let scene = render(&board);
        assert!(
            !scene.contains(r#""tap":"#),
            "a dead board must not invite more taps",
        );
    }
}
