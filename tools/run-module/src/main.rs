//! Runs a module the way slim runs it, and prints the JSON it answers with.
//!
//! Usage: run-module <module.wasm> <command> [input]
//!
//! This exists because `scripts/check-scene-ops.py` needs something to drive a
//! module, and because a module author otherwise has no way to see their own
//! output short of installing it into a space. It implements slim's module ABI
//! v1 and nothing else:
//!
//! - the module exports `memory`, `alloc(len) -> ptr` and
//!   `run(in_ptr, in_len) -> packed(out_ptr << 32 | out_len)`
//! - the module imports nothing, and this refuses one that does, exactly as
//!   slim refuses it - catching an accidental wasi or wasm-bindgen dependency
//!   here rather than at install time
//!
//! What it deliberately does not do is enforce slim's fuel, memory and
//! wall-clock limits. A module that passes here can still be refused there for
//! being too expensive; the manifest's `runtime.limits` is the authority on
//! that, and this tool is about correctness of output.

use std::process::ExitCode;

use wasmi::{Engine, Extern, Instance, Linker, Memory, Module, Store};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (path, command) = match args.as_slice() {
        [path, command, ..] => (path, command),
        _ => {
            eprintln!("usage: run-module <module.wasm> <command> [input]");
            return ExitCode::from(2);
        }
    };
    let input = args.get(2).map(String::as_str).unwrap_or("");

    match run(path, command, input) {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run(path: &str, command: &str, input: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let engine = Engine::default();
    let module = Module::new(&engine, &bytes[..]).map_err(|e| format!("not valid wasm: {e}"))?;

    // The import-free rule, enforced the way slim enforces it: an empty linker
    // means instantiation fails if the module wants anything from the host.
    if module.imports().count() != 0 {
        let wanted: Vec<String> = module
            .imports()
            .map(|i| format!("{}::{}", i.module(), i.name()))
            .collect();
        return Err(format!(
            "module imports {}, but slim only instantiates import-free modules",
            wanted.join(", ")
        ));
    }

    let mut store = Store::new(&engine, ());
    let linker = Linker::<()>::new(&engine);
    let instance = linker
        .instantiate_and_start(&mut store, &module)
        .map_err(|e| format!("cannot instantiate: {e}"))?;

    let request = serde_request(command, input);
    call_run(&mut store, &instance, request.as_bytes())
}

/// The request shape slim sends: `{"command":..., "input":...}`.
///
/// Hand-built rather than pulling in serde, so this tool stays a single small
/// dependency on the same engine slim uses and nothing more.
fn serde_request(command: &str, input: &str) -> String {
    format!(
        "{{\"command\":\"{}\",\"input\":\"{}\"}}",
        escape(command),
        escape(input)
    )
}

fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn call_run(
    store: &mut Store<()>,
    instance: &Instance,
    request: &[u8],
) -> Result<String, String> {
    let Some(Extern::Memory(memory)) = instance.get_export(&*store, "memory") else {
        return Err("module exports no memory".to_owned());
    };
    let alloc = instance
        .get_typed_func::<i32, i32>(&*store, "alloc")
        .map_err(|e| format!("module exports no usable alloc: {e}"))?;
    let run = instance
        .get_typed_func::<(i32, i32), i64>(&*store, "run")
        .map_err(|e| format!("module exports no usable run: {e}"))?;

    let in_len = request.len() as i32;
    let in_ptr = alloc
        .call(&mut *store, in_len)
        .map_err(|e| format!("alloc trapped: {e}"))?;
    memory
        .write(&mut *store, in_ptr as usize, request)
        .map_err(|e| format!("alloc returned memory that cannot hold the request: {e}"))?;

    let packed = run
        .call(&mut *store, (in_ptr, in_len))
        .map_err(|e| format!("run trapped: {e}"))?;
    let out_ptr = (packed >> 32) as u32 as usize;
    let out_len = (packed & 0xffff_ffff) as u32 as usize;

    read_guest(&memory, store, out_ptr, out_len)
        .ok_or_else(|| "run returned a region outside the module's memory".to_owned())
}

/// A bounds-checked read, for the same reason slim's own is: the pointer and
/// length both come from the module, so a module claiming a huge response gets
/// a refusal rather than this tool trying to allocate it.
fn read_guest(
    memory: &Memory,
    store: &Store<()>,
    ptr: usize,
    len: usize,
) -> Option<String> {
    let data = memory.data(store);
    let end = ptr.checked_add(len)?;
    let slice = data.get(ptr..end)?;
    Some(String::from_utf8_lossy(slice).into_owned())
}
