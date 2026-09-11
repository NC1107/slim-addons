//! game-of-life: a slim-m WASM module that runs Conway's Game of Life and
//! draws it as a slim scene.
//!
//! Conforms to slim-m's module ABI v1 (docs/decisions/0021): exports `memory`,
//! `alloc(len) -> ptr` and `run(in_ptr, in_len) -> packed(out_ptr, out_len)`,
//! and imports nothing - the whole simulation is a pure function of its input.
//!
//! The single `life` command answers two kinds of input. A plain string (a
//! preset name, an ASCII drawing, `random`, or empty) seeds a fresh board. A
//! JSON control message `{"action":...,"state":...}` advances an existing one:
//! the client holds the board in the scene's opaque `state` and hands it back,
//! so stepping the world is just another stateless call. Never traps on bad
//! input; always answers with `{"ok":true,"output":<scene json>}` or
//! `{"ok":false,"error":...}`.

mod life;
mod rng;
mod scene;

use std::alloc::{alloc as std_alloc, Layout};
use std::cell::RefCell;

use serde::{Deserialize, Serialize};

use life::Board;
use scene::Scene;

#[derive(Deserialize)]
struct Request {
    command: String,
    input: String,
}

#[derive(Deserialize)]
struct Control {
    action: String,
    #[serde(default)]
    state: String,
}

#[derive(Serialize)]
struct OkResponse {
    ok: bool,
    output: String,
}

#[derive(Serialize)]
struct ErrResponse {
    ok: bool,
    error: String,
}

thread_local! {
    static LAST_RESPONSE: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Reserves `len` bytes in this module's own linear memory and returns a
/// pointer to them. Called by the host before `run` to get somewhere to write
/// the request bytes.
#[no_mangle]
pub extern "C" fn alloc(len: i32) -> i32 {
    raw_alloc(len.max(0) as usize) as i32
}

/// Runs the request written at `in_ptr`/`in_len` and returns a packed
/// `(out_ptr << 32) | out_len` pointing at the UTF-8 JSON response.
#[no_mangle]
pub extern "C" fn run(in_ptr: i32, in_len: i32) -> i64 {
    let request = unsafe { std::slice::from_raw_parts(in_ptr as *const u8, in_len as usize) };
    let body = handle(request);

    let out_len = body.len();
    let out_ptr = if out_len == 0 {
        raw_alloc(0)
    } else {
        let ptr = raw_alloc(out_len);
        unsafe { std::ptr::copy_nonoverlapping(body.as_ptr(), ptr, out_len) };
        ptr
    };

    LAST_RESPONSE.with(|slot| *slot.borrow_mut() = body);

    let packed = ((out_ptr as u64) << 32) | (out_len as u64 & 0xFFFF_FFFF);
    packed as i64
}

fn handle(request: &[u8]) -> Vec<u8> {
    let parsed: Result<Request, _> = serde_json::from_slice(request);
    let parsed = match parsed {
        Ok(req) => req,
        Err(err) => return serialize_err(format!("invalid request: {err}")),
    };

    if parsed.command != "life" {
        return serialize_err(format!("unknown command: {}", parsed.command));
    }

    match run_life(&parsed.input) {
        Ok(scene) => serialize_ok(scene.to_json()),
        Err(message) => serialize_err(message),
    }
}

/// Either seeds a new board from a plain input or applies a control action to
/// the board carried in the message's `state`.
fn run_life(input: &str) -> Result<Scene, String> {
    if let Some(control) = parse_control(input) {
        return apply_control(control);
    }
    Ok(Scene::of(&Board::seed(input), true))
}

/// A control only when the input is a JSON object with an `action`; a preset
/// name or ASCII drawing is left to seed a board instead.
fn parse_control(input: &str) -> Option<Control> {
    let trimmed = input.trim_start();
    if !trimmed.starts_with('{') {
        return None;
    }
    serde_json::from_str::<Control>(trimmed).ok()
}

fn apply_control(control: Control) -> Result<Scene, String> {
    if let Some(rest) = control.action.strip_prefix("toggle:") {
        let mut board = Board::from_state(&control.state);
        for cell in rest.split(';') {
            let (row, col) = parse_cell(cell)?;
            board.toggle(col, row);
        }
        return Ok(Scene::of(&board, true));
    }
    match control.action.as_str() {
        "step" => {
            let (next, changed) = Board::from_state(&control.state).step();
            Ok(Scene::of(&next, changed))
        }
        "random" => Ok(Scene::of(&Board::random(control.state.as_bytes()), true)),
        "clear" => Ok(Scene::of(&Board::blank(), true)),
        other => Err(format!("unknown action: {other}")),
    }
}

/// Parses one `row,col` pair out of a `toggle:` action.
///
/// The action carries `;`-separated cells so a drag across the board is one
/// sandboxed run rather than one per cell: at a round trip each, a line drawn
/// with a finger took as long as the network did, and dropped cells whenever
/// the client's own in-flight guard refused a second call. A single cell is
/// just a list of one, so a client that knows nothing about this still works.
/// The scene advertises it with `tap_batch` so a client only sends a list to a
/// module that can read one.
fn parse_cell(rest: &str) -> Result<(usize, usize), String> {
    let mut parts = rest.split(',');
    let row = parts.next().and_then(|p| p.trim().parse::<usize>().ok());
    let col = parts.next().and_then(|p| p.trim().parse::<usize>().ok());
    match (row, col) {
        (Some(row), Some(col)) => Ok((row, col)),
        _ => Err(format!("bad cell: {rest}")),
    }
}

fn serialize_ok(output: String) -> Vec<u8> {
    serde_json::to_vec(&OkResponse { ok: true, output })
        .unwrap_or_else(|_| br#"{"ok":false,"error":"failed to serialize output"}"#.to_vec())
}

fn serialize_err(error: String) -> Vec<u8> {
    serde_json::to_vec(&ErrResponse { ok: false, error })
        .unwrap_or_else(|_| br#"{"ok":false,"error":"failed to serialize error"}"#.to_vec())
}

fn raw_alloc(len: usize) -> *mut u8 {
    if len == 0 {
        return std::ptr::NonNull::<u8>::dangling().as_ptr();
    }
    let layout = Layout::from_size_align(len, 1).expect("valid layout");
    unsafe { std_alloc(layout) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::life::COLS;

    /// One state string in, one out, through the same path the host calls.
    fn act(state: &str, action: &str) -> Scene {
        apply_control(Control {
            action: action.to_owned(),
            state: state.to_owned(),
        })
        .expect("the action should apply")
    }

    #[test]
    fn a_single_cell_still_toggles_as_it_always_did() {
        let blank = Board::blank().to_state();
        let after = act(&blank, "toggle:3,4");
        assert!(after.to_json().contains("\"status\""));
        let board = Board::from_state(&scene_state(&after));
        assert_eq!(board.population(), 1, "exactly the named cell");
        assert!(board.cell_string().as_bytes()[3 * COLS + 4] == b'1');
    }

    #[test]
    fn a_list_of_cells_toggles_every_one_in_a_single_run() {
        let blank = Board::blank().to_state();
        let after = act(&blank, "toggle:0,0;0,1;5,5");
        let board = Board::from_state(&scene_state(&after));
        let cells = board.cell_string();
        let live = |row: usize, col: usize| cells.as_bytes()[row * COLS + col] == b'1';
        assert!(live(0, 0) && live(0, 1) && live(5, 5), "every named cell");
        assert_eq!(board.population(), 3, "and nothing else");
    }

    #[test]
    fn the_same_cell_twice_in_one_list_toggles_twice() {
        // Not deduped here on purpose: the client decides what it drew, and a
        // list is applied exactly as given so the two cannot disagree.
        let blank = Board::blank().to_state();
        let board = Board::from_state(&scene_state(&act(&blank, "toggle:2,2;2,2")));
        assert_eq!(board.population(), 0);
    }

    #[test]
    fn a_bad_cell_anywhere_in_the_list_refuses_the_whole_action() {
        let blank = Board::blank().to_state();
        let err = apply_control(Control {
            action: "toggle:0,0;nonsense".to_owned(),
            state: blank,
        });
        assert!(err.is_err(), "a half-applied drag would be worse than none");
    }

    /// Digs the opaque state back out of a rendered scene.
    fn scene_state(scene: &Scene) -> String {
        let json: serde_json::Value =
            serde_json::from_str(&scene.to_json()).expect("the scene is json");
        json["state"].as_str().expect("a state string").to_owned()
    }
}
