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

#[test]
fn workspace_symbols_real_transport_preserves_query_ownership_and_complete_locations() {
    let dir = tempfile::tempdir().unwrap();
    let uri = token::lsp::path_to_uri(&dir.path().join("symbols.rs"));
    let scenario = write_scenario(
        dir.path(),
        json!([
            {"op": "expect_request", "method": "initialize", "respond": {"capabilities": {"workspaceSymbolProvider": true}}},
            {"op": "expect_request", "method": "workspace/symbol", "respond": [
                {"name": "méthode", "kind": 6, "location": {"uri": uri,
                    "range": {"start": {"line": 1, "character": 3}, "end": {"line": 1, "character": 9}}}}
            ]},
            {"op": "expect_request", "method": "workspace/symbol", "respond_error": {"code": -32603, "message": "fixture error"}},
            {"op": "expect_request", "method": "shutdown", "respond": null}
        ]),
    );
    let (tx, rx) = mpsc::channel();
    let mut handle = spawn_server(
        fake_lsp_server_path().to_str().unwrap(),
        &[scenario.to_string_lossy().into_owned()],
        dir.path(),
        "fake".into(),
        tx,
        None,
        serde_json::Value::Null,
        serde_json::Value::Null,
    )
    .unwrap();
    let ready = recv_until(&rx, Duration::from_secs(5), is_ready);
    let id = handle.begin_request("workspace/symbol", json!({"query": "méth"}));
    let first = recv_until(&rx, Duration::from_secs(5), |msg| {
        matches!(
            msg,
            Msg::Lsp(LspMsg::WorkspaceSymbolsResponseFromServer { .. })
        )
    });
    let error_id = handle.begin_request("workspace/symbol", json!({"query": "error"}));
    let second = recv_until(&rx, Duration::from_secs(5), |msg| {
        matches!(
            msg,
            Msg::Lsp(LspMsg::WorkspaceSymbolsResponseFromServer { .. })
        )
    });
    // Terminate before assertions, so a failing contract cannot orphan the fixture.
    handle.kill();
    assert!(ready.is_some());
    let Some(Msg::Lsp(LspMsg::WorkspaceSymbolsResponseFromServer {
        request_id,
        generation,
        root,
        result: Ok(results),
        abandoned,
        ..
    })) = first
    else {
        panic!("complete workspace-symbol response")
    };
    assert_eq!(request_id, id);
    assert_eq!(generation, handle.generation);
    assert_eq!(root, dir.path());
    assert!(!abandoned);
    assert_eq!(results.items.len(), 1);
    assert_eq!(results.items[0].name, "méthode");
    assert_eq!(results.items[0].location.uri, uri);
    assert_eq!(results.items[0].location.range.start.character, 3);
    assert!(
        matches!(second, Some(Msg::Lsp(LspMsg::WorkspaceSymbolsResponseFromServer {request_id, result: Err(_), ..})) if request_id == error_id)
    );
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
        serde_json::Value::Null,
        serde_json::Value::Null,
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
        serde_json::Value::Null,
        serde_json::Value::Null,
    )
    .expect("spawn fake-lsp-server");

    // Ready never arrives (capabilities stay ungated) — but the process
    // must still be killable and the reader thread must not have
    // wedged; if it had, `kill()` (which waits on the child) would hang
    // too and the test's own harness timeout would catch it.
    let ready = recv_until(&msg_rx, Duration::from_millis(500), is_ready);
    assert!(
        ready.is_none(),
        "a never-responding server must never report Ready"
    );

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
        serde_json::Value::Null,
        serde_json::Value::Null,
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
        serde_json::Value::Null,
        serde_json::Value::Null,
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
        serde_json::Value::Null,
        serde_json::Value::Null,
    )
    .expect("spawn fake-lsp-server");

    assert!(recv_until(&msg_rx, Duration::from_secs(5), is_ready).is_some());
    let indexing = recv_until(&msg_rx, Duration::from_secs(5), |m| {
        matches!(
            m,
            Msg::Lsp(LspMsg::ServerStateChanged {
                state: ServerState::Indexing,
                ..
            })
        )
    });
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
        serde_json::Value::Null,
        serde_json::Value::Null,
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
        serde_json::Value::Null,
        serde_json::Value::Null,
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
        serde_json::Value::Null,
        serde_json::Value::Null,
    )
    .expect("spawn rust-analyzer (must be on PATH for this ignored test)");

    assert!(
        recv_until(&msg_rx, Duration::from_secs(60), is_ready).is_some(),
        "real rust-analyzer should reach Ready within 60s"
    );
    assert!(handle.capabilities_snapshot().is_some());

    handle.kill();
}

/// A server that advertises `signatureHelpProvider` and answers
/// `textDocument/signatureHelp`: the reader mirrors the trigger characters
/// and parses the reply into `SignatureHelpResponseFromServer`.
#[test]
fn signature_help_request_round_trips_through_the_fake_server() {
    let dir = tempfile::tempdir().unwrap();
    let scenario = write_scenario(
        dir.path(),
        json!([
            { "op": "expect_request", "method": "initialize", "respond": { "capabilities": {
                "signatureHelpProvider": { "triggerCharacters": ["("], "retriggerCharacters": [")"] }
            } } },
            { "op": "expect_request", "method": "textDocument/signatureHelp", "respond": {
                "signatures": [{ "label": "fn f(a: i32)", "parameters": [{ "label": [5, 11] }] }],
                "activeSignature": 0,
                "activeParameter": 0
            } },
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
        serde_json::Value::Null,
        serde_json::Value::Null,
    )
    .expect("spawn fake-lsp-server");

    assert!(recv_until(&msg_rx, Duration::from_secs(5), is_ready).is_some());
    let triggers = recv_until(&msg_rx, Duration::from_secs(5), |m| {
        matches!(m, Msg::Lsp(LspMsg::ServerSignatureTriggers { .. }))
    });
    let Some(Msg::Lsp(LspMsg::ServerSignatureTriggers {
        trigger, retrigger, ..
    })) = triggers
    else {
        panic!("expected ServerSignatureTriggers");
    };
    assert_eq!(trigger, vec!["(".to_owned()]);
    assert_eq!(retrigger, vec![")".to_owned()]);

    let request_id = handle.begin_request(
        "textDocument/signatureHelp",
        json!({
            "textDocument": { "uri": "file:///tmp/main.rs" },
            "position": { "line": 0, "character": 5 },
            "context": { "triggerKind": 2, "triggerCharacter": "(", "isRetrigger": false }
        }),
    );
    let reply = recv_until(&msg_rx, Duration::from_secs(5), |m| {
        matches!(m, Msg::Lsp(LspMsg::SignatureHelpResponseFromServer { .. }))
    });
    let Some(Msg::Lsp(LspMsg::SignatureHelpResponseFromServer {
        request_id: id,
        help,
        abandoned,
        ..
    })) = reply
    else {
        panic!("expected SignatureHelpResponseFromServer");
    };
    assert_eq!(id, request_id);
    assert!(!abandoned);
    let help = help.expect("parsed SignatureHelp");
    assert_eq!(help.signatures[0].label, "fn f(a: i32)");
    assert_eq!(help.active_parameter, Some(0));

    handle.kill();
}

/// A server that advertises `renameProvider` and answers
/// `textDocument/prepareRename` + `textDocument/rename`: the reader parses
/// both replies into their `*ResponseFromServer` messages.
#[test]
fn rename_requests_round_trip_through_the_fake_server() {
    let dir = tempfile::tempdir().unwrap();
    let scenario = write_scenario(
        dir.path(),
        json!([
            { "op": "expect_request", "method": "initialize", "respond": { "capabilities": {
                "renameProvider": { "prepareProvider": true }
            } } },
            { "op": "expect_request", "method": "textDocument/prepareRename", "respond": {
                "range": { "start": { "line": 0, "character": 3 }, "end": { "line": 0, "character": 7 } },
                "placeholder": "main"
            } },
            { "op": "expect_request", "method": "textDocument/rename", "respond": {
                "changes": { "file:///tmp/main.rs": [
                    { "range": { "start": { "line": 0, "character": 3 }, "end": { "line": 0, "character": 7 } },
                      "newText": "start" }
                ] }
            } },
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
        serde_json::Value::Null,
        serde_json::Value::Null,
    )
    .expect("spawn fake-lsp-server");
    assert!(recv_until(&msg_rx, Duration::from_secs(5), is_ready).is_some());

    let prepare_id = handle.begin_request(
        "textDocument/prepareRename",
        json!({
            "textDocument": { "uri": "file:///tmp/main.rs" },
            "position": { "line": 0, "character": 5 }
        }),
    );
    let reply = recv_until(&msg_rx, Duration::from_secs(5), |m| {
        matches!(m, Msg::Lsp(LspMsg::PrepareRenameResponseFromServer { .. }))
    });
    let Some(Msg::Lsp(LspMsg::PrepareRenameResponseFromServer {
        request_id,
        response,
        abandoned,
        ..
    })) = reply
    else {
        panic!("expected PrepareRenameResponseFromServer");
    };
    assert_eq!(request_id, prepare_id);
    assert!(!abandoned);
    assert!(matches!(
        response,
        Some(lsp_types::PrepareRenameResponse::RangeWithPlaceholder { placeholder, .. })
            if placeholder == "main"
    ));

    let rename_id = handle.begin_request(
        "textDocument/rename",
        json!({
            "textDocument": { "uri": "file:///tmp/main.rs" },
            "position": { "line": 0, "character": 5 },
            "newName": "start"
        }),
    );
    let reply = recv_until(&msg_rx, Duration::from_secs(5), |m| {
        matches!(m, Msg::Lsp(LspMsg::RenameResponseFromServer { .. }))
    });
    let Some(Msg::Lsp(LspMsg::RenameResponseFromServer {
        request_id, edit, ..
    })) = reply
    else {
        panic!("expected RenameResponseFromServer");
    };
    assert_eq!(request_id, rename_id);
    let edit = edit.expect("parsed WorkspaceEdit");
    assert_eq!(
        edit.changes.unwrap().values().next().unwrap()[0].new_text,
        "start"
    );

    handle.kill();
}

/// A server that advertises `codeActionProvider` and answers
/// `textDocument/codeAction` with a mixed `(Command | CodeAction)[]`: the
/// reader flattens it into `CodeActionsResponseFromServer` rows.
#[test]
fn code_action_request_round_trips_through_the_fake_server() {
    let dir = tempfile::tempdir().unwrap();
    let scenario = write_scenario(
        dir.path(),
        json!([
            { "op": "expect_request", "method": "initialize", "respond": { "capabilities": {
                "codeActionProvider": true
            } } },
            { "op": "expect_request", "method": "textDocument/codeAction", "respond": [
                { "title": "Fix it", "kind": "quickfix", "isPreferred": true,
                  "edit": { "changes": {} } },
                { "title": "Run", "command": "server.doIt" }
            ] },
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
        serde_json::Value::Null,
        serde_json::Value::Null,
    )
    .expect("spawn fake-lsp-server");

    assert!(recv_until(&msg_rx, Duration::from_secs(5), is_ready).is_some());
    let request_id = handle.begin_request(
        "textDocument/codeAction",
        json!({
            "textDocument": { "uri": "file:///tmp/main.rs" },
            "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } },
            "context": { "diagnostics": [], "triggerKind": 1 }
        }),
    );
    let reply = recv_until(&msg_rx, Duration::from_secs(5), |m| {
        matches!(m, Msg::Lsp(LspMsg::CodeActionsResponseFromServer { .. }))
    });
    let Some(Msg::Lsp(LspMsg::CodeActionsResponseFromServer {
        request_id: id,
        actions,
        abandoned,
        ..
    })) = reply
    else {
        panic!("expected CodeActionsResponseFromServer");
    };
    assert_eq!(id, request_id);
    assert!(!abandoned);
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0].title, "Fix it");
    assert!(actions[0].is_preferred);
    assert!(actions[0].edit.is_some());
    assert_eq!(actions[0].kind.as_deref(), Some("quickfix"));
    assert_eq!(
        actions[1].command.as_ref().map(|c| c.command.as_str()),
        Some("server.doIt")
    );

    handle.kill();
}
