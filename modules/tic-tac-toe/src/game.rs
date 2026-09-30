//! Tic-tac-toe: the whole interactive scene loop in one small module. Every
//! frame is a fresh sandboxed call - the board rides in the scene's opaque
//! `state`, a tap on a cell arrives as an action, and the module returns the
//! next frame. No host storage, no memory between calls.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::seats::{Refusal, Seats};

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

pub fn apply(command: &str, input: &str, caller: &str) -> Result<String, String> {
    match command {
        "play" => Ok(play(input, caller)),
        other => Err(format!("unknown command: {other}")),
    }
}

fn play(input: &str, caller: &str) -> String {
    let trimmed = input.trim();
    // The launch runs with empty input: start a fresh board.
    if trimmed.is_empty() {
        return render(&Game::new(), None);
    }
    // Otherwise it is an interactive frame: {action, state}.
    let (action, mut game) = match serde_json::from_str::<Action>(trimmed) {
        Ok(a) => (a.action, Game::from_state(&a.state)),
        Err(_) => (String::new(), Game::new()),
    };
    let mut note = None;
    match action.as_str() {
        "new game" | "new" => {
            if game.seats.may_reset(caller, game.over()) {
                game = Game::new();
            } else {
                note = Some(Refusal::Illegal("only a player can restart a game in progress"));
            }
        }
        // A cell's tappable rect carries tap "p<index>", so the action is "p4" for cell 4.
        cell if cell.starts_with('p') => {
            if let Ok(i) = cell[1..].parse::<usize>() {
                note = game.play_at(i, caller).err();
            }
        }
        _ => {}
    }
    render(&game, note)
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
    seats: Seats,
}

impl Game {
    fn new() -> Self {
        Game {
            cells: ['.'; 9],
            turn: 'X',
            seats: Seats::default(),
        }
    }

    /// Rebuilds a game from the opaque `state` string this module wrote last
    /// frame: nine board chars, whose turn it is, then the two seat ids, all
    /// `|` separated.
    fn from_state(s: &str) -> Self {
        let mut parts = s.splitn(4, '|');
        let board = parts.next().unwrap_or("");
        let turn = parts.next().unwrap_or("X");
        let seats = Seats::parse(parts.next().unwrap_or(""), parts.next().unwrap_or(""));
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
        Game { cells, turn, seats }
    }

    fn state(&self) -> String {
        let board: String = self.cells.iter().collect();
        format!("{board}|{}|{}|{}", self.turn, self.seats.first(), self.seats.second())
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

    /// One person's move: seat and turn are checked before the cell is.
    fn play_at(&mut self, i: usize, caller: &str) -> Result<(), Refusal> {
        if self.over() {
            return Err(Refusal::Illegal("the game is over"));
        }
        let seats = self.seats.admit(caller, self.turn == 'X')?;
        if i >= 9 || self.cells[i] != '.' {
            return Err(Refusal::Illegal("that square is taken"));
        }
        self.mark(i);
        self.seats = seats;
        Ok(())
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

fn render(game: &Game, note: Option<Refusal>) -> String {
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
    let status = match note {
        Some(refusal) => format!("{} - {status}", refusal.note()),
        None => status,
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

    const A: &str = "aa";
    const B: &str = "bb";
    const C: &str = "cc";

    /// Plays `(caller, cell)` moves in order and returns the final state string.
    fn run(moves: &[(&str, usize)]) -> String {
        let mut state = Game::new().state();
        for (who, cell) in moves {
            let input = format!(r#"{{"action":"p{cell}","state":"{state}"}}"#);
            let out = apply("play", &input, who).unwrap();
            state = scene_state(&out);
        }
        state
    }

    fn scene_state(scene: &str) -> String {
        let v: Value = serde_json::from_str(scene).unwrap();
        v["state"].as_str().unwrap().to_string()
    }

    fn status_after(moves: &[(&str, usize)]) -> String {
        let state = run(&moves[..moves.len() - 1]);
        let (who, cell) = moves[moves.len() - 1];
        let input = format!(r#"{{"action":"p{cell}","state":"{state}"}}"#);
        let v: Value = serde_json::from_str(&apply("play", &input, who).unwrap()).unwrap();
        v["status"].as_str().unwrap().to_string()
    }

    #[test]
    fn play_launches_a_scene_and_a_tap_marks_it() {
        let initial = apply("play", "", A).unwrap();
        assert!(initial.contains(r#""$slim":"scene/1""#));
        assert!(initial.contains("X to move"));
        let after = apply("play", r#"{"action":"p4","state":".........|X||"}"#, A).unwrap();
        assert!(after.contains("O to move"));
    }

    #[test]
    fn new_game_resets_from_any_state() {
        let out = apply("play", r#"{"action":"new game","state":"XXXOO....|X||"}"#, A).unwrap();
        assert!(out.contains("X to move"));
        assert!(out.contains(r#""state":".........|X||""#));
    }

    #[test]
    fn a_row_wins_between_two_players() {
        let state = run(&[(A, 0), (B, 3), (A, 1), (B, 4), (A, 2)]);
        assert!(state.starts_with("XXXOO...."));
        assert_eq!(status_after(&[(A, 0), (B, 3), (A, 1), (B, 4), (A, 2)]), "X wins");
    }

    #[test]
    fn a_full_board_with_no_line_is_a_draw() {
        // X O X / X O O / O X X fills the board with no line.
        let moves = [(A, 0), (B, 1), (A, 2), (B, 4), (A, 3), (B, 5), (A, 7), (B, 6), (A, 8)];
        assert_eq!(status_after(&moves), "draw");
    }

    #[test]
    fn the_same_person_cannot_move_twice_in_a_row() {
        let state = run(&[(A, 4)]);
        let again = apply("play", &format!(r#"{{"action":"p0","state":"{state}"}}"#), A).unwrap();
        assert!(again.contains("not your turn"));
        assert_eq!(scene_state(&again), state);
    }

    #[test]
    fn the_second_person_cannot_move_out_of_turn() {
        let status = status_after(&[(A, 0), (B, 1), (B, 2)]);
        assert!(status.starts_with("not your turn"), "{status}");
        let early = status_after(&[(A, 0), (B, 1), (A, 2), (B, 3), (B, 4)]);
        assert!(early.starts_with("not your turn"), "{early}");
    }

    #[test]
    fn a_taken_square_is_refused_without_costing_the_turn() {
        let status = status_after(&[(A, 4), (B, 4)]);
        assert!(status.starts_with("that square is taken"));
        let state = run(&[(A, 4)]);
        let out = apply("play", &format!(r#"{{"action":"p4","state":"{state}"}}"#), B).unwrap();
        assert_eq!(scene_state(&out), state, "B did not claim a seat with an illegal move");
    }

    #[test]
    fn a_third_person_watches_and_cannot_move() {
        assert!(status_after(&[(A, 0), (B, 1), (C, 2)]).starts_with("both sides are taken"));
    }

    #[test]
    fn nobody_moves_after_the_game_is_won() {
        let won = [(A, 0), (B, 3), (A, 1), (B, 4), (A, 2)];
        let state = run(&won);
        let out = apply("play", &format!(r#"{{"action":"p8","state":"{state}"}}"#), B).unwrap();
        assert!(out.contains("the game is over"));
        assert_eq!(scene_state(&out), state);
    }

    #[test]
    fn an_unidentified_caller_cannot_move() {
        let out = apply("play", r#"{"action":"p0","state":".........|X||"}"#, "").unwrap();
        assert!(out.contains("did not say who you are"));
    }

    #[test]
    fn only_a_player_can_restart_a_game_in_progress() {
        let state = run(&[(A, 0), (B, 1)]);
        let input = format!(r#"{{"action":"new game","state":"{state}"}}"#);
        assert!(apply("play", &input, C).unwrap().contains("only a player can restart"));
        assert!(apply("play", &input, A).unwrap().contains(r#""state":".........|X||""#));
    }

    #[test]
    fn a_finished_game_can_be_restarted_by_anyone() {
        let state = run(&[(A, 0), (B, 3), (A, 1), (B, 4), (A, 2)]);
        let input = format!(r#"{{"action":"new game","state":"{state}"}}"#);
        assert!(apply("play", &input, C).unwrap().contains(r#""state":".........|X||""#));
    }

    #[test]
    fn the_same_inputs_give_the_same_board() {
        let moves = [(A, 4), (B, 0), (A, 8)];
        assert_eq!(run(&moves), run(&moves));
    }
}
