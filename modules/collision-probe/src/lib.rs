//! collision-probe: not a useful module. Its manifest is the point, not this
//! wasm - three extension-point shapes the server's manifest validation and
//! the client's own name-based matching accept without complaint, even
//! though each one produces silent, undetectable ambiguity:
//!
//! 1. Two `command` extension points both named "probe", gated on different
//!    permissions. Nothing rejects the duplicate. At runtime,
//!    `http::module_commands::required_permission` finds the *first*
//!    declared one by name and stops looking, so `see-public` (declared
//!    first) is the only permission ever checked for `probe` - `see-admin`
//!    is dead weight, silently never enforced. Swap the declaration order
//!    and the enforced permission swaps with it, with no error at install
//!    time telling the reviewing admin which one actually governs.
//! 2. A `slash-command` named "co llide" (an embedded space). Manifest
//!    validation only trims and rejects control characters - it does not
//!    require a safe slug - so this installs cleanly. The composer's own
//!    `matchSlashCommand`/`_commandRows` split on the first run of
//!    whitespace to find the keyword, so "co llide" can never be typed: the
//!    keyword before the space ("co") matches nothing, and the full name
//!    including the space can never appear before a space in typed text.
//!    A working alias ("probe") is declared alongside it so the module is
//!    still reachable.
//! 3. Nothing here reaches wasm at all: the same ambiguity exists across
//!    modules, not just within one manifest's own extension_points. Two
//!    independently installed modules may declare the exact same
//!    `slash-command` name, or an `app` whose display name slugifies to
//!    another module's exact `id`; the client resolves either purely by
//!    string match over the whole discovery list, most-recently-installed
//!    first, with no per-module identity carried from whichever
//!    autocomplete row the person actually picked. See this module's PR
//!    description for how that was measured.
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
        "probe" => serialize_ok(
            "you reached this module's 'probe' command. Its manifest declares \
             two command extension points both named 'probe': the one gating \
             this call is whichever was declared first - see the module's \
             description for which, and why the second is silently dead."
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
