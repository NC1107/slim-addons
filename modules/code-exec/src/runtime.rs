//! Evaluates a JavaScript snippet in a fresh boa context.
//!
//! `console.log` is shimmed to append to a per-call output buffer instead of
//! doing anything host-visible (boa has no host bindings to begin with).
//! The response combines everything printed with the script's completion
//! value, matching a REPL. Any parse or runtime error is caught and returned
//! as text; nothing here ever panics on user input.

use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::context::time::FixedClock;
use boa_engine::object::ObjectInitializer;
use boa_engine::property::Attribute;
use boa_engine::{js_string, Context, JsResult, JsValue, NativeFunction, Source};

thread_local! {
    static OUTPUT: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Runs `source` as a JS script and returns the combined console output and
/// completion value, or an error message on a parse/runtime failure.
///
/// Uses a fixed clock instead of the engine's default `StdClock`: the host
/// ABI forbids importing a clock, and `std::time::Instant`/`SystemTime` both
/// panic at runtime on wasm32-unknown-unknown, which is exactly what the
/// default clock calls as soon as a context is built.
pub fn execute(source: &str) -> Result<String, String> {
    OUTPUT.with(|buf| buf.borrow_mut().clear());

    let mut context = Context::builder()
        .clock(Rc::new(FixedClock::from_millis(0)))
        .build()
        .map_err(|err| err.to_string())?;
    install_console(&mut context).map_err(|err| err.to_string())?;

    let result = context.eval(Source::from_bytes(source));
    let printed = OUTPUT.with(|buf| buf.borrow().clone());

    match result {
        Ok(value) => Ok(combine(printed, value, &mut context)),
        Err(err) => Err(err.to_string()),
    }
}

fn combine(printed: String, value: JsValue, context: &mut Context) -> String {
    let mut out = printed;
    if !value.is_undefined() {
        if let Ok(text) = value.to_string(context) {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&text.to_std_string_lossy());
        }
    }
    out
}

fn install_console(context: &mut Context) -> JsResult<()> {
    let console = ObjectInitializer::new(context)
        .function(
            NativeFunction::from_fn_ptr(console_log),
            js_string!("log"),
            0,
        )
        .build();
    context.register_global_property(js_string!("console"), console, Attribute::all())?;
    Ok(())
}

fn console_log(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let mut parts = Vec::with_capacity(args.len());
    for arg in args {
        parts.push(arg.to_string(context)?.to_std_string_lossy());
    }
    OUTPUT.with(|buf| {
        let mut buf = buf.borrow_mut();
        if !buf.is_empty() {
            buf.push('\n');
        }
        buf.push_str(&parts.join(" "));
    });
    Ok(JsValue::undefined())
}
