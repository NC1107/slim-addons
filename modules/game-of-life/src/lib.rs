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
        let (row, col) = parse_cell(rest)?;
        board.toggle(col, row);
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

/// Parses a `row,col` pair from a `toggle:` action.
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
