//! Tic-tac-toe: the whole interactive scene loop in one small module. Every
//! frame is a fresh sandboxed call - the board rides in the scene's opaque
//! `state`, a tap on a cell arrives as an action, and the module returns the
//! next frame. No host storage, no memory between calls.

use serde::Deserialize;
use serde_json::{json, Value};

const LINES: [[usize; 3]; 8] = [
    [0, 1, 2],
    [3, 4, 5],
    [6, 7, 8],
    [0, 3, 6],
    [1, 4, 7],
    [2, 5, 8],
    [0, 4, 8],
    [2, 4, 6],
];

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
    // Otherwise it is an interactive frame: {action, state}.
    let (action, mut game) = match serde_json::from_str::<Action>(trimmed) {
        Ok(a) => (a.action, Game::from_state(&a.state)),
        Err(_) => (String::new(), Game::new()),
    };
    match action.as_str() {
        "new game" | "new" => game = Game::new(),
        // A cell's tappable rect carries tap "p<index>", so the action is "p4" for cell 4.
        cell if cell.starts_with('p') => {
            if let Ok(i) = cell[1..].parse::<usize>() {
                game.mark(i);
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

struct Game {
    cells: [char; 9],
    turn: char,
}

impl Game {
    fn new() -> Self {
        Game {
            cells: ['.'; 9],
            turn: 'X',
        }
    }

    /// Rebuilds a game from the opaque `state` string this module wrote last
    /// frame: nine board chars, a `|`, then whose turn it is.
    fn from_state(s: &str) -> Self {
        let (board, turn) = s.split_once('|').unwrap_or(("", "X"));
        let mut cells = ['.'; 9];
        for (i, c) in board.chars().take(9).enumerate() {
            if c == 'X' || c == 'O' {
                cells[i] = c;
            }
        }
        let turn = turn
            .chars()
            .next()
            .filter(|c| *c == 'X' || *c == 'O')
            .unwrap_or('X');
        Game { cells, turn }
    }

    fn state(&self) -> String {
        let board: String = self.cells.iter().collect();
        format!("{board}|{}", self.turn)
    }

    fn winner(&self) -> Option<char> {
        LINES.iter().find_map(|line| {
            let a = self.cells[line[0]];
            (a != '.' && a == self.cells[line[1]] && a == self.cells[line[2]]).then_some(a)
        })
    }

    fn full(&self) -> bool {
        self.cells.iter().all(|c| *c != '.')
    }

    fn over(&self) -> bool {
        self.winner().is_some() || self.full()
    }

    fn mark(&mut self, i: usize) {
        if i < 9 && self.cells[i] == '.' && !self.over() {
            self.cells[i] = self.turn;
            self.turn = other_player(self.turn);
        }
    }
}

fn other_player(p: char) -> char {
    if p == 'X' {
        'O'
    } else {
        'X'
    }
}

const CELL: f64 = 30.0;

fn render(game: &Game) -> String {
    let mut ops: Vec<Value> = Vec::new();
    // The grid: two lines each way across a 90x90 board.
    for k in 1..3 {
        let p = k as f64 * CELL;
        ops.push(json!({"op":"line","x1":p,"y1":0.0,"x2":p,"y2":90.0,"stroke":"border","sw":1.0}));
        ops.push(json!({"op":"line","x1":0.0,"y1":p,"x2":90.0,"y2":p,"stroke":"border","sw":1.0}));
    }

    let over = game.over();
    for (i, &c) in game.cells.iter().enumerate() {
        let (x0, y0) = ((i % 3) as f64 * CELL, (i / 3) as f64 * CELL);
        match c {
            'X' => {
                ops.push(json!({"op":"line","x1":x0+7.0,"y1":y0+7.0,"x2":x0+23.0,"y2":y0+23.0,"stroke":"accent","sw":2.5}));
                ops.push(json!({"op":"line","x1":x0+23.0,"y1":y0+7.0,"x2":x0+7.0,"y2":y0+23.0,"stroke":"accent","sw":2.5}));
            }
            'O' => {
                ops.push(json!({"op":"circle","cx":x0+15.0,"cy":y0+15.0,"r":8.0,"stroke":"text","sw":2.5}));
            }
            // An empty cell, only while the game is on, is a tappable recessed square carrying its own index.
            _ if !over => {
                ops.push(json!({"op":"rect","x":x0+3.0,"y":y0+3.0,"w":24.0,"h":24.0,"fill":"sunken","r":3.0,"tap":format!("p{i}")}));
            }
            _ => {}
        }
    }

    let status = match game.winner() {
        Some(w) => format!("{w} wins"),
        None if game.full() => "draw".to_string(),
        None => format!("{} to move", game.turn),
    };

    json!({
        "$slim": "scene/1",
        "width": 90.0,
        "height": 90.0,
        "background": "surface",
        "ops": ops,
        "status": status,
        "controls": ["new game"],
        "state": game.state(),
        // Always live so "new game" keeps working after a result; marks are refused once the game is over.
        "live": true,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_game_is_empty_with_x_to_move() {
        let g = Game::new();
        assert_eq!(g.turn, 'X');
        assert!(g.cells.iter().all(|c| *c == '.'));
    }

    #[test]
    fn state_round_trips() {
        let mut g = Game::new();
        g.mark(0);
        g.mark(4);
        let g2 = Game::from_state(&g.state());
        assert_eq!(g2.cells, g.cells);
        assert_eq!(g2.turn, g.turn);
    }

    #[test]
    fn marking_places_the_current_mark_and_swaps_the_turn() {
        let mut g = Game::new();
        g.mark(4);
        assert_eq!(g.cells[4], 'X');
        assert_eq!(g.turn, 'O');
    }

    #[test]
    fn a_taken_cell_cannot_be_overwritten() {
        let mut g = Game::new();
        g.mark(4);
        g.mark(4);
        assert_eq!(g.cells[4], 'X');
        assert_eq!(g.turn, 'O', "the refused move must not have swapped the turn");
    }

    #[test]
    fn a_row_wins_and_freezes_the_board() {
        let mut g = Game::new();
        for i in [0, 3, 1, 4, 2] {
            g.mark(i); // X:0, O:3, X:1, O:4, X:2 -> X takes the top row
        }
        assert_eq!(g.winner(), Some('X'));
        g.mark(5);
        assert_eq!(g.cells[5], '.', "no move lands after the game is won");
    }

    #[test]
    fn play_launches_a_scene_and_a_tap_marks_it() {
        let initial = apply("play", "").unwrap();
        assert!(initial.contains(r#""$slim":"scene/1""#));
        assert!(initial.contains("X to move"));
        let after = apply("play", r#"{"action":"p4","state":".........|X"}"#).unwrap();
        assert!(after.contains("O to move"));
    }

    #[test]
    fn new_game_resets_from_any_state() {
        let out = apply("play", r#"{"action":"new game","state":"XXXOO....|X"}"#).unwrap();
        assert!(out.contains("X to move"));
        assert!(out.contains(r#""state":".........|X""#));
    }
}
