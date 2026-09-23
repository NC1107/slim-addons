//! surface-hijack-probe: not a useful module. Its point is the manifest
//! description and the run route it demonstrates, not this wasm.
//!
//! Launch this module's `app` extension point in a channel: it posts a
//! message whose block 0 carries this module's own output. `app_surfaces`
//! then records `(module_id: "surface-hijack-probe", command: "board")`
//! against that message, and every viewer's client re-runs and renders
//! block 0 through `POST /messages/{id}/blocks/0/run`.
//!
//! That route (`crates/slimm-server/src/http/code_runs.rs::run` in slim-m)
//! takes `module_id` and `command` from the *caller's own request body*, not
//! from the message's `app_surfaces` row. It checks the caller can view the
//! message, then gates purely on whichever module/command the caller named
//! (`http::module_commands::execute_command`). Nothing cross-checks that
//! against what this specific message actually launched.
//!
//! So: install any second module (anything - the `dice` module in this same
//! registry is enough), grant some other user only *that* module's
//! permission, and have them POST `{"module_id": "dice", "command": "roll",
//! "input": ""}` to this app's own message/block-0 run endpoint. It
//! succeeds, and the shared, broadcast board every viewer of this app sees
//! is now the dice module's output - overwritten by someone who never held
//! `own-board` at all. `app_surfaces` itself is untouched: it still says
//! this module launched the message, permanently disagreeing with what is
//! actually rendered.
//!
//! Conforms to slim-m's module ABI v1 (docs/decisions/0021): exports
//! `memory`, `alloc(len) -> ptr` and `run(in_ptr, in_len) -> packed(out_ptr,
//! out_len)`, and imports nothing.

use std::alloc::{Layout, alloc as std_alloc};

use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Request {
    command: String,
    #[allow(dead_code)]
    input: String,
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

#[no_mangle]
pub extern "C" fn alloc(len: i32) -> i32 {
    raw_alloc(len.max(0) as usize) as i32
}

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

    let packed = ((out_ptr as u64) << 32) | (out_len as u64 & 0xFFFF_FFFF);
    packed as i64
}

fn handle(request: &[u8]) -> Vec<u8> {
    let parsed: Result<Request, _> = serde_json::from_slice(request);
    let parsed = match parsed {
        Ok(req) => req,
        Err(err) => return serialize_err(format!("invalid request: {err}")),
    };

    match parsed.command.as_str() {
        "board" => serialize_ok(
            "this is surface-hijack-probe's own board. If you are seeing \
             different text than this, someone holding a different \
             module's permission just overwrote it through this message's \
             block-0 run endpoint - see this module's manifest description."
                .to_owned(),
        ),
        other => serialize_err(format!("no such command: {other}")),
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
