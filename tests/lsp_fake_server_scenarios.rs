//! Integration harness for the LSP client worker
//! (docs/feature/lsp-integration.md Phase 1 "Testing Strategy":
//! "Integration: scriptable fake server"), driven against the real
//! `fake-lsp-server` binary — see `src/bin/fake_lsp_server.rs` for the
//! scenario step vocabulary.
//!
//! Every test here drives `token::lsp::client::spawn_server` (the same
//! seam the runtime uses) against a real child process speaking real
//! Content-Length-framed JSON-RPC, asserting on the `Msg`s the reader
//! thread produces — a genuine integration test of the client, not a
//! mock of the server.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::json;

use token::lsp::client::spawn_server;
use token::lsp::{LspServerId, ServerState};
use token::messages::{LspMsg, Msg};

fn fake_lsp_server_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fake-lsp-server"))
}

fn write_scenario(dir: &std::path::Path, steps: serde_json::Value) -> PathBuf {
    let path = dir.join("scenario.json");
    std::fs::write(&path, steps.to_string()).expect("write scenario file");
    path
}

fn recv_until<F: Fn(&Msg) -> bool>(
    rx: &mpsc::Receiver<Msg>,
    timeout: Duration,
    matches: F,
) -> Option<Msg> {
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return None;
        }
        match rx.recv_timeout(remaining) {
            Ok(msg) if matches(&msg) => return Some(msg),
            Ok(_) => continue,
            Err(_) => return None,
        }
    }
}

fn is_ready(msg: &Msg) -> bool {
    matches!(
        msg,
        Msg::Lsp(LspMsg::ServerStateChanged {
            state: ServerState::Ready,
            ..
        })
    )
}

fn is_failed(msg: &Msg) -> bool {
    matches!(
        msg,
        Msg::Lsp(LspMsg::ServerStateChanged {
            state: ServerState::Failed,
            ..
        })
    )
}

fn is_exited(msg: &Msg) -> bool {
    matches!(msg, Msg::Lsp(LspMsg::ServerExited { .. }))
}

/// Full lifecycle: `initialize` -> `initialized` -> ready. Also proves
/// `workspace/configuration` mid-init gets a reply — real servers
/// (rust-analyzer, pyright) block their init path on this, so a missing
/// reply would hang the test at the 5s timeout instead of passing.
#[test]
fn full_lifecycle_including_workspace_configuration_mid_init() {
    let dir = tempfile::tempdir().unwrap();
    let scenario = write_scenario(
        dir.path(),
        json!([
            // The server asks for configuration *before* answering our
            // `initialize` — a real deadlock scenario if the client
            // doesn't reply (design doc's Handshake ordering).
            { "op": "request", "id": 900, "method": "workspace/configuration", "params": { "items": [{}, {}] } },
            { "op": "expect_request", "method": "initialize", "respond": { "capabilities": {} } },
        ]),
    );

    let (msg_tx, msg_rx) = mpsc::channel();
    let mut handle = spawn_server(
        fake_lsp_server_path().to_str().unwrap(),
        &[scenario.to_string_lossy().into_owned()],
        dir.path(),
        LspServerId::from("fake"),
        msg_tx,
        None,
    )
    .expect("spawn fake-lsp-server");

    let ready = recv_until(&msg_rx, Duration::from_secs(5), is_ready);
    assert!(ready.is_some(), "expected Ready — client must have replied to workspace/configuration for the fake server to ever answer initialize");

    handle.kill();
}

/// A request the server never answers: the client must not hang forever
/// waiting for it — the pending entry can be abandoned, but never
/// panics/blocks the reader thread (which would also starve every other
/// in-flight request).
#[test]
fn never_responding_server_does_not_wedge_the_reader() {
    let dir = tempfile::tempdir().unwrap();
    let scenario = write_scenario(
        dir.path(),
        json!([
            // No "respond": read the request, then do nothing forever.
            { "op": "expect_request", "method": "initialize" },
            { "op": "sleep_ms", "ms": 60000 },
        ]),
    );

    let (msg_tx, msg_rx) = mpsc::channel();
    let mut handle = spawn_server(
        fake_lsp_server_path().to_str().unwrap(),
        &[scenario.to_string_lossy().into_owned()],
        dir.path(),
        LspServerId::from("fake"),
        msg_tx,
        None,
    )
    .expect("spawn fake-lsp-server");

    // Ready never arrives (capabilities stay ungated) — but the process
    // must still be killable and the reader thread must not have
    // wedged; if it had, `kill()` (which waits on the child) would hang
    // too and the test's own harness timeout would catch it.
    let ready = recv_until(&msg_rx, Duration::from_millis(500), is_ready);
    assert!(ready.is_none(), "a never-responding server must never report Ready");

    handle.kill();
}

/// Server exits mid-request (crash simulation): the reader thread must
/// report `ServerExited`, not panic or hang.
#[test]
fn exit_mid_request_reports_server_exited() {
    let dir = tempfile::tempdir().unwrap();
    let scenario = write_scenario(
        dir.path(),
        json!([
            { "op": "expect_request", "method": "initialize", "respond": { "capabilities": {} } },
            { "op": "exit", "code": 1 },
        ]),
    );

    let (msg_tx, msg_rx) = mpsc::channel();
    let mut handle = spawn_server(
        fake_lsp_server_path().to_str().unwrap(),
        &[scenario.to_string_lossy().into_owned()],
        dir.path(),
        LspServerId::from("fake"),
        msg_tx,
        None,
    )
    .expect("spawn fake-lsp-server");

    assert!(recv_until(&msg_rx, Duration::from_secs(5), is_ready).is_some());
    assert!(
        recv_until(&msg_rx, Duration::from_secs(5), is_exited).is_some(),
        "expected ServerExited after the fake server exits"
    );

    handle.kill();
}

/// Malformed / partial frames (missing headers, garbage bytes): the
/// reader thread treats them as EOF/exit rather than panicking — see
/// `transport.rs`'s own unit tests for the framing-level behavior; this
/// proves the *client* (reader_loop) degrades the same way through a
/// real pipe.
#[test]
fn malformed_frame_is_treated_as_exit_not_a_panic() {
    let dir = tempfile::tempdir().unwrap();
    let scenario = write_scenario(
        dir.path(),
        json!([
            { "op": "expect_request", "method": "initialize", "respond": { "capabilities": {} } },
            { "op": "write_raw", "text": "not-a-content-length-header\r\n\r\ngarbage" },
        ]),
    );

    let (msg_tx, msg_rx) = mpsc::channel();
    let mut handle = spawn_server(
        fake_lsp_server_path().to_str().unwrap(),
        &[scenario.to_string_lossy().into_owned()],
        dir.path(),
        LspServerId::from("fake"),
        msg_tx,
        None,
    )
    .expect("spawn fake-lsp-server");

    assert!(recv_until(&msg_rx, Duration::from_secs(5), is_ready).is_some());
    assert!(
        recv_until(&msg_rx, Duration::from_secs(5), is_exited).is_some(),
        "a malformed frame must surface as ServerExited, not a hang or panic"
    );

    handle.kill();
}

/// Duplicate and unknown response ids are logged and dropped, never a
/// panic — the reader thread must keep running afterward (proven by a
/// subsequent, valid exchange still working).
#[test]
fn duplicate_and_unknown_response_ids_are_dropped_not_fatal() {
    let dir = tempfile::tempdir().unwrap();
    let scenario = write_scenario(
        dir.path(),
        json!([
            { "op": "expect_request", "method": "initialize", "respond": { "capabilities": {} } },
            // Unknown id: no request the client made ever used it.
            { "op": "respond_raw", "value": { "jsonrpc": "2.0", "id": 99999, "result": null } },
            // Duplicate: a second response to the same (now-resolved) id.
            { "op": "respond_raw", "value": { "jsonrpc": "2.0", "id": 1, "result": { "capabilities": {} } } },
            // Progress notification proves the reader thread is still
            // alive and dispatching after the above.
            { "op": "notify", "method": "$/progress", "params": { "value": { "kind": "begin" } } },
        ]),
    );

    let (msg_tx, msg_rx) = mpsc::channel();
    let mut handle = spawn_server(
        fake_lsp_server_path().to_str().unwrap(),
        &[scenario.to_string_lossy().into_owned()],
        dir.path(),
        LspServerId::from("fake"),
        msg_tx,
        None,
    )
    .expect("spawn fake-lsp-server");

    assert!(recv_until(&msg_rx, Duration::from_secs(5), is_ready).is_some());
    let indexing = recv_until(
        &msg_rx,
        Duration::from_secs(5),
        |m| matches!(
            m,
            Msg::Lsp(LspMsg::ServerStateChanged { state: ServerState::Indexing, .. })
        ),
    );
    assert!(
        indexing.is_some(),
        "reader thread must still be dispatching after duplicate/unknown ids"
    );

    handle.kill();
}

/// A server that floods stderr must not wedge — the stderr drain thread
/// keeps the pipe empty so the child never blocks writing to it, and the
/// handshake on stdout/stdin still completes promptly.
#[test]
fn stderr_flood_does_not_wedge_the_handshake() {
    let dir = tempfile::tempdir().unwrap();
    let scenario = write_scenario(
        dir.path(),
        json!([
            // Comfortably larger than a 64KB pipe buffer at ~40 bytes/line.
            { "op": "flood_stderr", "lines": 5000 },
            { "op": "expect_request", "method": "initialize", "respond": { "capabilities": {} } },
        ]),
    );

    let (msg_tx, msg_rx) = mpsc::channel();
    let mut handle = spawn_server(
        fake_lsp_server_path().to_str().unwrap(),
        &[scenario.to_string_lossy().into_owned()],
        dir.path(),
        LspServerId::from("fake"),
        msg_tx,
        None,
    )
    .expect("spawn fake-lsp-server");

    assert!(
        recv_until(&msg_rx, Duration::from_secs(10), is_ready).is_some(),
        "a stderr flood must not delay/block the handshake"
    );

    handle.kill();
}

/// `initialize` rejected with a JSON-RPC error: the client must report
/// `Failed`, not treat the server as healthy.
#[test]
fn initialize_error_reports_failed_not_ready() {
    let dir = tempfile::tempdir().unwrap();
    let scenario = write_scenario(
        dir.path(),
        json!([
            {
                "op": "expect_request",
                "method": "initialize",
                "respond_error": { "code": -32603, "message": "boom" },
            },
        ]),
    );

    let (msg_tx, msg_rx) = mpsc::channel();
    let mut handle = spawn_server(
        fake_lsp_server_path().to_str().unwrap(),
        &[scenario.to_string_lossy().into_owned()],
        dir.path(),
        LspServerId::from("fake"),
        msg_tx,
        None,
    )
    .expect("spawn fake-lsp-server");

    assert!(
        recv_until(&msg_rx, Duration::from_secs(5), is_failed).is_some(),
        "a rejected initialize must report Failed"
    );
    assert!(handle.capabilities_snapshot().is_none());

    handle.kill();
}

/// One test against a real, locally installed `rust-analyzer` —
/// `#[ignore]`d per the design doc's Testing Strategy (not run in CI by
/// default; run with `cargo test -- --ignored` on a machine that has it
/// on `PATH`).
#[test]
#[ignore]
fn real_rust_analyzer_completes_the_handshake() {
    let dir = tempfile::tempdir().unwrap();
    let (msg_tx, msg_rx) = mpsc::channel();
    let mut handle = spawn_server(
        "rust-analyzer",
        &[],
        dir.path(),
        LspServerId::from("rust-analyzer"),
        msg_tx,
        None,
    )
    .expect("spawn rust-analyzer (must be on PATH for this ignored test)");

    assert!(
        recv_until(&msg_rx, Duration::from_secs(60), is_ready).is_some(),
        "real rust-analyzer should reach Ready within 60s"
    );
    assert!(handle.capabilities_snapshot().is_some());

    handle.kill();
}
