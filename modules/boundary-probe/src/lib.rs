//! boundary-probe: not a useful module. Every command deliberately misbehaves
//! at a distinct point in slim's wasm ABI - the seam `module_runtime::
//! ModuleHost::run` and `http::module_commands::execute_command` sit on - so
//! that the exact `{ "ok": false, "error": ... }` this deployment answers
//! with for each failure shape is visible from a live install, not just from
//! reading the host's own source.
//!
//! Conforms to slim-m's module ABI v1 (docs/decisions/0021): exports
//! `memory`, `alloc(len) -> ptr` and `run(in_ptr, in_len) -> packed(out_ptr,
//! out_len)`, and imports nothing. Unlike a well-behaved module, several
//! commands here deliberately return bytes that are not the ABI's
//! `{ok,output}`/`{ok,error}` shape, or do not return at all - that is the
//! whole point, so this file writes raw response bytes directly rather than
//! going through a single always-well-formed serializer.

use std::alloc::{alloc as std_alloc, Layout};

use serde::Deserialize;

#[derive(Deserialize)]
struct Request {
    command: String,
    #[allow(dead_code)]
    input: String,
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

/// Dispatches to one deliberate misbehavior per command. Every arm returns
/// raw bytes, not always through a JSON serializer, because several arms
/// deliberately do not produce valid `{ok,...}` JSON at all.
fn handle(request: &[u8]) -> Vec<u8> {
    let parsed: Result<Request, _> = serde_json::from_slice(request);
    let command = match &parsed {
        Ok(req) => req.command.as_str(),
        Err(err) => return err_json(&format!("invalid request: {err}")),
    };

    match command {
        "malformed-not-json" => b"this is not json at all".to_vec(),
        "malformed-wrong-shape" => br#"[1, 2, 3]"#.to_vec(),
        "malformed-both-fields" => {
            br#"{"ok":true,"output":"the real output","error":"a contradicting error"}"#.to_vec()
        }
        "malformed-null" => b"null".to_vec(),
        "no-output" => Vec::new(),
        "trap-unreachable" => trap_unreachable(),
        "trap-out-of-bounds" => trap_out_of_bounds(),
        "loop-forever" => loop_forever(),
        other => err_json(&format!("no such command: {other}")),
    }
}

fn err_json(message: &str) -> Vec<u8> {
    let escaped = message.replace('\\', "\\\\").replace('"', "\\\"");
    format!(r#"{{"ok":false,"error":"{escaped}"}}"#).into_bytes()
}

/// A genuine Rust panic, which this crate's `panic = "abort"` profile lowers
/// to wasm's `unreachable` instruction - the same trap a bare `unreachable!()`
/// or an index-out-of-bounds slice access would produce. It never returns.
fn trap_unreachable() -> Vec<u8> {
    panic!("boundary-probe: deliberate trap-unreachable");
}

/// A raw pointer read far past this module's own linear memory (its
/// `runtime.limits.memory_mb` caps it at 16 MiB), bypassing Rust's own slice
/// bounds checks by using a raw pointer instead of indexing - so the actual
/// wasm `load` instruction is what traps, not a Rust-level panic first.
/// `read_volatile` keeps the optimizer from proving the read unreachable and
/// deleting it.
fn trap_out_of_bounds() -> Vec<u8> {
    let ptr = 0x7fff_fff0usize as *const u8;
    let byte = unsafe { std::ptr::read_volatile(ptr) };
    std::hint::black_box(byte);
    unreachable!("read byte {byte} from far out of bounds; the host should have trapped first");
}

/// An unconditional loop that does only cheap, branch-only work per
/// iteration. This manifest deliberately sets `fuel` near the host's own
/// ceiling (`MAX_FUEL`) with a short `wall_ms`, so the wall-clock deadline -
/// not fuel exhaustion - is what actually stops this call. It never returns
/// on its own.
fn loop_forever() -> Vec<u8> {
    let mut x: u32 = 0;
    loop {
        x = x.wrapping_add(1);
        std::hint::black_box(x);
    }
}

fn raw_alloc(len: usize) -> *mut u8 {
    if len == 0 {
        return std::ptr::NonNull::<u8>::dangling().as_ptr();
    }
    let layout = Layout::from_size_align(len, 1).expect("valid layout");
    unsafe { std_alloc(layout) }
}
