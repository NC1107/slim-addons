//! Connect Four: a seven-by-six board where a disc falls to the lowest free
//! row of the column you tap. Every frame is a fresh sandboxed call - the
//! board rides in the scene's opaque `state`, a tap arrives as an action, and
//! the module returns the next frame. No host storage, no memory between
//! calls.
//!
//! The tappable target is the whole column rather than a cell: you choose a
//! column and gravity chooses the row, so offering a cell would invite a tap
//! that cannot mean anything.

use serde::Deserialize;
use serde_json::{json, Value};

pub const COLS: usize = 7;
pub const ROWS: usize = 6;
const NEED: usize = 4;

/// The four directions a line can run. Their opposites are covered by walking
/// each one from both ends, so south-west is not a separate case from
/// north-east.
const DIRECTIONS: [(isize, isize); 4] = [(1, 0), (0, 1), (1, 1), (1, -1)];

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
        return render(&Game::new());
    }
    let (action, mut game) = match serde_json::from_str::<Action>(trimmed) {
        Ok(a) => (a.action, Game::from_state(&a.state)),
        Err(_) => (String::new(), Game::new()),
    };
    match action.as_str() {
        "new game" | "new" => game = Game::new(),
        // A column's tappable rect carries tap "c<index>".
        col if col.starts_with('c') => {
            if let Ok(i) = col[1..].parse::<usize>() {
                game.drop_disc(i);
            }
        }
        _ => {}
    }
    render(&game)
}

#[derive(Deserialize)]
struct Action {
    action: String,
    #[serde(default)]
    state: String,
}

pub struct Game {
    /// Row 0 is the top. `cells[row * COLS + col]`.
    pub cells: [char; COLS * ROWS],
    pub turn: char,
}

impl Game {
    pub fn new() -> Self {
        Game {
            cells: ['.'; COLS * ROWS],
            turn: 'R',
        }
    }

    /// Rebuilds a game from the opaque `state` this module wrote last frame:
    /// forty-two board chars, a `|`, then whose turn it is.
    pub fn from_state(s: &str) -> Self {
        let (board, turn) = s.split_once('|').unwrap_or(("", "R"));
        let mut cells = ['.'; COLS * ROWS];
        for (i, c) in board.chars().take(COLS * ROWS).enumerate() {
            if c == 'R' || c == 'Y' {
                cells[i] = c;
            }
        }
        let turn = turn
            .chars()
            .next()
            .filter(|c| *c == 'R' || *c == 'Y')
            .unwrap_or('R');
        Game { cells, turn }
    }

    pub fn state(&self) -> String {
        let board: String = self.cells.iter().collect();
        format!("{board}|{}", self.turn)
    }

    fn at(&self, col: usize, row: usize) -> char {
        self.cells[row * COLS + col]
    }

    /// The row a disc dropped into `col` would land in, or None when the
    /// column is full. Rows fill from the bottom up.
    pub fn landing_row(&self, col: usize) -> Option<usize> {
        if col >= COLS {
            return None;
        }
        (0..ROWS).rev().find(|&row| self.at(col, row) == '.')
    }

    pub fn drop_disc(&mut self, col: usize) {
        if self.over() {
            return;
        }
        if let Some(row) = self.landing_row(col) {
            self.cells[row * COLS + col] = self.turn;
            self.turn = other_player(self.turn);
        }
    }

    /// The four cells of the winning line, or None. Returned rather than just
    /// a colour so the board can show *which* four won, which is the whole
    /// reason a person scans the grid after a result.
    pub fn winning_line(&self) -> Option<[(usize, usize); NEED]> {
        for col in 0..COLS {
            for row in 0..ROWS {
                let player = self.at(col, row);
                if player == '.' {
                    continue;
                }
                for (dc, dr) in DIRECTIONS {
                    if let Some(line) = self.line_from(col, row, dc, dr, player) {
                        return Some(line);
                    }
                }
            }
        }
        None
    }

    fn line_from(
        &self,
        col: usize,
        row: usize,
        dc: isize,
        dr: isize,
        player: char,
    ) -> Option<[(usize, usize); NEED]> {
        let mut line = [(0usize, 0usize); NEED];
        for (step, slot) in line.iter_mut().enumerate() {
            let c = col as isize + dc * step as isize;
            let r = row as isize + dr * step as isize;
            if c < 0 || r < 0 || c >= COLS as isize || r >= ROWS as isize {
                return None;
            }
            let (c, r) = (c as usize, r as usize);
            if self.at(c, r) != player {
                return None;
            }
            *slot = (c, r);
        }
        Some(line)
    }

    pub fn winner(&self) -> Option<char> {
        self.winning_line().map(|line| self.at(line[0].0, line[0].1))
    }

    pub fn full(&self) -> bool {
        (0..COLS).all(|col| self.landing_row(col).is_none())
    }

    pub fn over(&self) -> bool {
        self.winner().is_some() || self.full()
    }
}

fn other_player(p: char) -> char {
    if p == 'R' {
        'Y'
    } else {
        'R'
    }
}

const CELL: f64 = 26.0;
const PAD: f64 = 3.0;

/// Red and yellow are the game's own colours, and neither the theme palette
/// nor its fallback has a yellow: "warning" is not a scene colour, so the
/// first cut of this drew both players in the same fallback. Hex is allowed
/// by the renderer and is right here - these are game pieces, not interface
/// chrome, and they have to stay told apart in either theme.
fn colour_for(player: char) -> &'static str {
    if player == 'R' {
        "#d94f4f"
    } else {
        "#e3b341"
    }
}

fn render(game: &Game) -> String {
    let width = COLS as f64 * CELL;
    let height = ROWS as f64 * CELL;
    let mut ops: Vec<Value> = Vec::new();

    let over = game.over();
    let winning = game.winning_line();

    // Every playable column is one tall tappable strip. Drawn first so the
    // discs and the grid sit on top of it rather than under it.
    if !over {
        for col in 0..COLS {
            if game.landing_row(col).is_none() {
                continue;
            }
            ops.push(json!({
                "op": "rect",
                "x": col as f64 * CELL + 1.0,
                "y": 1.0,
                "w": CELL - 2.0,
                "h": height - 2.0,
                "fill": "sunken",
                "r": 3.0,
                "tap": format!("c{col}"),
            }));
        }
    }

    // The grid lines, so the columns read as slots rather than a blank field.
    for k in 1..COLS {
        let x = k as f64 * CELL;
        ops.push(json!({"op":"line","x1":x,"y1":0.0,"x2":x,"y2":height,"stroke":"border","sw":1.0}));
    }
    for k in 1..ROWS {
        let y = k as f64 * CELL;
        ops.push(json!({"op":"line","x1":0.0,"y1":y,"x2":width,"y2":y,"stroke":"border","sw":1.0}));
    }

    for col in 0..COLS {
        for row in 0..ROWS {
            let player = game.at(col, row);
            if player == '.' {
                continue;
            }
            let cx = col as f64 * CELL + CELL / 2.0;
            let cy = row as f64 * CELL + CELL / 2.0;
            ops.push(json!({
                "op": "circle",
                "cx": cx,
                "cy": cy,
                "r": CELL / 2.0 - PAD,
                "fill": colour_for(player),
            }));
            // The winning four are ringed, so a result says which line won.
            if winning.is_some_and(|line| line.contains(&(col, row))) {
                ops.push(json!({
                    "op": "circle",
                    "cx": cx,
                    "cy": cy,
                    "r": CELL / 2.0 - PAD,
                    "stroke": "text",
                    "sw": 2.0,
                }));
            }
        }
    }

    let status = match game.winner() {
        Some('R') => "Red wins".to_string(),
        Some(_) => "Yellow wins".to_string(),
        None if game.full() => "draw".to_string(),
        None if game.turn == 'R' => "Red to move".to_string(),
        None => "Yellow to move".to_string(),
    };

    json!({
        "$slim": "scene/1",
        "width": width,
        "height": height,
        "background": "surface",
        "ops": ops,
        "status": status,
        "controls": ["new game"],
        "state": game.state(),
        // Always live so "new game" keeps working after a result; drops are
        // refused once the game is over.
        "live": true,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drops a run of discs, alternating players as the game does.
    fn play_columns(cols: &[usize]) -> Game {
        let mut g = Game::new();
        for &c in cols {
            g.drop_disc(c);
        }
        g
    }

    #[test]
    fn a_fresh_game_is_empty_with_red_to_move() {
        let g = Game::new();
        assert_eq!(g.turn, 'R');
        assert!(g.cells.iter().all(|c| *c == '.'));
    }

    #[test]
    fn a_disc_falls_to_the_bottom_of_its_column() {
        let g = play_columns(&[3]);
        assert_eq!(g.at(3, ROWS - 1), 'R', "the first disc lands on the floor");
        assert_eq!(g.at(3, 0), '.', "and nowhere near the top");
    }

    #[test]
    fn discs_stack_on_each_other() {
        let g = play_columns(&[3, 3]);
        assert_eq!(g.at(3, ROWS - 1), 'R');
        assert_eq!(g.at(3, ROWS - 2), 'Y', "the second sits on the first");
    }

    #[test]
    fn a_full_column_refuses_another_disc_without_costing_the_turn() {
        let mut g = Game::new();
        for _ in 0..ROWS {
            g.drop_disc(0);
        }
        assert!(g.landing_row(0).is_none());
        let turn_before = g.turn;
        g.drop_disc(0);
        assert_eq!(
            g.turn, turn_before,
            "a refused drop must not hand the turn to the other player",
        );
    }

    #[test]
    fn state_round_trips() {
        let g = play_columns(&[3, 3, 4]);
        let again = Game::from_state(&g.state());
        assert_eq!(again.cells, g.cells);
        assert_eq!(again.turn, g.turn);
    }

    #[test]
    fn four_in_a_row_wins_horizontally() {
        // Red takes columns 0-3 along the floor; Yellow answers in column 6.
        let g = play_columns(&[0, 6, 1, 6, 2, 6, 3]);
        assert_eq!(g.winner(), Some('R'));
        let line = g.winning_line().expect("a win has a line");
        assert!(line.iter().all(|&(_, row)| row == ROWS - 1));
    }

    #[test]
    fn four_in_a_column_wins_vertically() {
        let g = play_columns(&[2, 3, 2, 3, 2, 3, 2]);
        assert_eq!(g.winner(), Some('R'));
    }

    #[test]
    fn four_on_a_diagonal_wins() {
        // Builds a staircase so Red occupies (0,5), (1,4), (2,3) and (3,2).
        let g = play_columns(&[0, 1, 1, 2, 2, 3, 2, 3, 3, 6, 3]);
        assert_eq!(g.winner(), Some('R'), "a rising diagonal is a win");
    }

    #[test]
    fn three_in_a_row_is_not_a_win() {
        let g = play_columns(&[0, 6, 1, 6, 2]);
        assert_eq!(g.winner(), None);
    }

    #[test]
    fn the_board_freezes_once_it_is_won() {
        let mut g = play_columns(&[0, 6, 1, 6, 2, 6, 3]);
        assert_eq!(g.winner(), Some('R'));
        g.drop_disc(5);
        assert_eq!(g.at(5, ROWS - 1), '.', "no disc lands after the game is won");
    }

    /// The bug that shipped in 0.1.0: "warning" is not one of the renderer's
    /// nine colour names, so yellow fell back and both players drew the same.
    /// An unknown colour never fails, it just quietly stops being that colour.
    #[test]
    fn the_two_players_are_drawn_in_colours_the_renderer_knows_and_can_tell_apart() {
        const KNOWN: [&str; 9] = [
            "bg", "surface", "sunken", "accent", "accent-soft", "muted", "text",
            "border", "danger",
        ];
        let red = colour_for('R');
        let yellow = colour_for('Y');
        assert_ne!(red, yellow, "the players must not share a colour");
        for name in [red, yellow] {
            assert!(
                name.starts_with('#') || KNOWN.contains(&name),
                "{name:?} is not a scene colour; it falls back silently",
            );
        }

        let g = play_columns(&[3, 4]);
        let scene = render(&g);
        assert!(scene.contains(red), "red disc missing from the scene");
        assert!(scene.contains(yellow), "yellow disc missing from the scene");
    }

    #[test]
    fn play_launches_a_scene_and_a_tap_drops_a_disc() {
        let initial = apply("play", "").unwrap();
        assert!(initial.contains(r#""$slim":"scene/1""#));
        assert!(initial.contains("Red to move"));

        let blank = ".".repeat(COLS * ROWS);
        let after = apply("play", &format!(r#"{{"action":"c3","state":"{blank}|R"}}"#)).unwrap();
        assert!(after.contains("Yellow to move"));
    }

    #[test]
    fn a_full_column_offers_no_tap_target() {
        let mut g = Game::new();
        for _ in 0..ROWS {
            g.drop_disc(0);
        }
        let scene = render(&g);
        assert!(
            !scene.contains(r#""tap":"c0""#),
            "a column nobody can play must not invite a tap",
        );
        assert!(scene.contains(r#""tap":"c1""#), "its neighbours still do");
    }

    #[test]
    fn new_game_resets_from_any_state() {
        let g = play_columns(&[0, 1, 2]);
        let out = apply("play", &format!(r#"{{"action":"new game","state":"{}"}}"#, g.state()))
            .unwrap();
        assert!(out.contains("Red to move"));
        assert!(out.contains(&format!(r#""state":"{}|R""#, ".".repeat(COLS * ROWS))));
    }
}
