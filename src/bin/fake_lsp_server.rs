//! Scriptable fake LSP server for the Phase 1 integration harness
//! (docs/feature/lsp-integration.md "Integration: scriptable fake
//! server"). Speaks real Content-Length-framed JSON-RPC over stdio
//! (reusing `token::lsp::transport`) so it exercises the real client
//! code, not a mock of it.
//!
//! Driven by a scenario file: a JSON array of steps, executed in order.
//! Steps (`"op"` tag):
//! - `expect_request`: block until a message with `"method"` arrives;
//!   if `"respond"` is present, write it back as the result (an
//!   `"error"` key sends a JSON-RPC error instead); if absent, the
//!   request is read and left unanswered (the "never responds" case).
//! - `notify`: write a server -> client notification.
//! - `request`: write a server -> client request (`"id"` + `"method"` +
//!   `"params"`).
//! - `respond_raw`: write an arbitrary JSON value verbatim (duplicate /
//!   unknown response ids).
//! - `write_raw`: write a literal string with no framing (malformed /
//!   partial frame scenarios).
//! - `sleep_ms`: pause.
//! - `flood_stderr`: write N lines to stderr rapidly (drain-wedge check).
//! - `exit`: exit the process immediately with the given code (crash
//!   simulation).
//! - `record_until_exit`: a generic session recorder for whole-session
//!   integration tests (the "edit-heavy session stays in sync" gate) —
//!   loops reading every message, appends one line per message to
//!   `"file"` (`request:<method>`, `notify:<method>`, or the raw body
//!   for anything with an id it didn't recognize as a method), and
//!   auto-replies to `initialize` (full-sync + `includeText` save
//!   capabilities) and any other request (`null`) so the real client
//!   never hangs waiting on a reply. Runs until the stream closes.

use std::io::{self, BufRead, BufReader, Write};
use std::time::Duration;

use serde_json::Value;
use token::lsp::transport::{read_message, write_message};

fn main() {
    let scenario_path = std::env::args()
        .nth(1)
        .expect("usage: fake-lsp-server <scenario.json>");
    let scenario = std::fs::read_to_string(&scenario_path).expect("read scenario file");
    let steps: Vec<Value> = serde_json::from_str(&scenario).expect("scenario is a JSON array");

    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let stdout = io::stdout();
    let mut writer = stdout.lock();

    for step in &steps {
        run_step(step, &mut reader, &mut writer);
    }
}

/// A short summary of a `textDocument/*` notification's identifying
/// fields (uri + version), for the transcript file — enough to assert
/// ordering and version monotonicity without parsing full JSON back out
/// in the test.
fn body_of(message: &Value) -> String {
    let uri = message.pointer("/params/textDocument/uri").and_then(Value::as_str);
    let version = message.pointer("/params/textDocument/version");
    format!("uri={uri:?} version={version:?}")
}

fn run_step(step: &Value, reader: &mut impl BufRead, writer: &mut impl Write) {
    let op = step.get("op").and_then(Value::as_str).unwrap_or("");
    match op {
        "expect_request" => {
            let expected_method = step.get("method").and_then(Value::as_str);
            loop {
                let message = match read_message(reader) {
                    Ok(m) => m,
                    Err(_) => return, // stream closed; nothing left to do
                };
                if expected_method.is_some_and(|m| message.get("method").and_then(Value::as_str) != Some(m))
                {
                    continue; // not the message this step is waiting for
                }
                let Some(id) = message.get("id").cloned() else {
                    continue; // a notification, not the request we're expecting
                };
                if let Some(respond) = step.get("respond") {
                    let body = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": respond,
                    });
                    let _ = write_message(writer, &body);
                } else if let Some(error) = step.get("respond_error") {
                    let body = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": error,
                    });
                    let _ = write_message(writer, &body);
                }
                // No "respond"/"respond_error": read and leave unanswered
                // (never-responds / abandonment scenario).
                return;
            }
        }
        "notify" => {
            let body = serde_json::json!({
                "jsonrpc": "2.0",
                "method": step.get("method").and_then(Value::as_str).unwrap_or(""),
                "params": step.get("params").cloned().unwrap_or(Value::Null),
            });
            let _ = write_message(writer, &body);
        }
        "request" => {
            let body = serde_json::json!({
                "jsonrpc": "2.0",
                "id": step.get("id").cloned().unwrap_or(Value::Null),
                "method": step.get("method").and_then(Value::as_str).unwrap_or(""),
                "params": step.get("params").cloned().unwrap_or(Value::Null),
            });
            let _ = write_message(writer, &body);
        }
        "respond_raw" => {
            if let Some(value) = step.get("value") {
                let _ = write_message(writer, value);
            }
        }
        "write_raw" => {
            if let Some(raw) = step.get("text").and_then(Value::as_str) {
                let _ = writer.write_all(raw.as_bytes());
                let _ = writer.flush();
            }
        }
        "sleep_ms" => {
            let ms = step.get("ms").and_then(Value::as_u64).unwrap_or(0);
            std::thread::sleep(Duration::from_millis(ms));
        }
        "flood_stderr" => {
            let lines = step.get("lines").and_then(Value::as_u64).unwrap_or(0);
            let err = io::stderr();
            let mut err = err.lock();
            for i in 0..lines {
                let _ = writeln!(err, "fake-lsp-server stderr flood line {i}");
            }
        }
        "exit" => {
            let code = step.get("code").and_then(Value::as_i64).unwrap_or(1) as i32;
            std::process::exit(code);
        }
        "record_until_exit" => {
            let file_path = step
                .get("file")
                .and_then(Value::as_str)
                .expect("record_until_exit needs a \"file\"");
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(file_path)
                .expect("open transcript file");
            while let Ok(message) = read_message(reader) {
                let method = message.get("method").and_then(Value::as_str);
                let id = message.get("id").cloned();
                let line = match method {
                    Some(m) if id.is_some() => format!("request:{m}:{}", body_of(&message)),
                    Some(m) => format!("notify:{m}:{}", body_of(&message)),
                    None => format!("response:{:?}", id),
                };
                let _ = writeln!(file, "{line}");
                let _ = file.flush();

                if let (Some(m), Some(id)) = (method, id) {
                    let result = match m {
                        "initialize" => serde_json::json!({
                            "capabilities": {
                                "textDocumentSync": {
                                    "openClose": true,
                                    "change": 1,
                                    "save": { "includeText": true },
                                },
                            }
                        }),
                        _ => Value::Null,
                    };
                    let body = serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result });
                    let _ = write_message(writer, &body);
                }
            }
        }
        other => {
            eprintln!("fake-lsp-server: unknown step op {other:?}");
        }
    }
}
