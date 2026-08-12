//! Per-server worker: process spawn, JSON-RPC handshake, request-id
//! correlation, and the read/dispatch loop.
//!
//! Three threads per server, matching the "one thread owns one pipe"
//! shape `terminal::pty::spawn_pty` already uses:
//! - **writer**: owns the child's stdin and the `HandshakeGate`; every
//!   outbound frame (ours or an auto-reply to the server) funnels
//!   through it so write order is never interleaved.
//! - **reader**: owns the child's stdout; parses frames, resolves
//!   pending requests, computes auto-replies, and forwards `Msg`s.
//! - **stderr drain**: reads and discards (into `tracing`) so a chatty
//!   server (rust-analyzer) never wedges on a full pipe buffer.

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use lsp_types::{
    ClientCapabilities, CompletionClientCapabilities, CompletionItemCapability,
    CompletionItemCapabilityResolveSupport, DidChangeWatchedFilesClientCapabilities,
    GeneralClientCapabilities, GotoCapability, HoverClientCapabilities, MarkupKind,
    PositionEncodingKind, PublishDiagnosticsClientCapabilities, ServerCapabilities, TagSupport,
    TextDocumentClientCapabilities, TextDocumentSyncCapability, TextDocumentSyncClientCapabilities,
    TextDocumentSyncKind, TextDocumentSyncSaveOptions, WindowClientCapabilities,
    WorkspaceClientCapabilities,
};
use serde_json::{json, Value};

use super::transport::{read_message, write_message};
use super::{LspServerId, ServerState};
use crate::messages::{LspMsg, Msg};

/// The exact client capabilities block from lsp-integration.md's
/// "Client Capabilities" section. Rule stated there: never advertise a
/// capability we don't implement — every field here is deliberate.
pub fn client_capabilities() -> ClientCapabilities {
    ClientCapabilities {
        general: Some(GeneralClientCapabilities {
            position_encodings: Some(vec![PositionEncodingKind::UTF16]),
            ..Default::default()
        }),
        text_document: Some(TextDocumentClientCapabilities {
            synchronization: Some(TextDocumentSyncClientCapabilities {
                dynamic_registration: Some(false),
                will_save: Some(false),
                will_save_wait_until: Some(false),
                did_save: Some(true),
            }),
            publish_diagnostics: Some(PublishDiagnosticsClientCapabilities {
                related_information: Some(true),
                version_support: Some(true),
                tag_support: Some(TagSupport {
                    value_set: vec![
                        lsp_types::DiagnosticTag::UNNECESSARY,
                        lsp_types::DiagnosticTag::DEPRECATED,
                    ],
                }),
                ..Default::default()
            }),
            definition: Some(GotoCapability {
                dynamic_registration: Some(false),
                link_support: Some(false),
            }),
            hover: Some(HoverClientCapabilities {
                dynamic_registration: Some(false),
                content_format: Some(vec![MarkupKind::PlainText, MarkupKind::Markdown]),
            }),
            completion: Some(CompletionClientCapabilities {
                dynamic_registration: Some(false),
                completion_item: Some(CompletionItemCapability {
                    snippet_support: Some(false),
                    resolve_support: Some(CompletionItemCapabilityResolveSupport {
                        properties: vec![
                            "documentation".to_owned(),
                            "detail".to_owned(),
                            "additionalTextEdits".to_owned(),
                        ],
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }),
        workspace: Some(WorkspaceClientCapabilities {
            did_change_watched_files: Some(DidChangeWatchedFilesClientCapabilities {
                dynamic_registration: Some(false),
                relative_pattern_support: None,
            }),
            ..Default::default()
        }),
        window: Some(WindowClientCapabilities {
            work_done_progress: Some(true),
            ..Default::default()
        }),
        ..Default::default()
    }
}

// ============================================================================
// Server capability gating
// ============================================================================
//
// `InitializeResult.capabilities` is parsed and stored on `ServerHandle`
// (see `spawn_server`'s reader loop); these are the read side future
// sends gate on (design doc's "Server capability gating"). `textDocumentSync`
// may be a bare number, a full options object, or absent entirely — all
// three mean something different and are handled here rather than at
// every call site.

/// What kind of document sync a server wants, collapsing the
/// number/object/absent shapes of `textDocumentSync` into one type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    /// Field absent, or explicitly `None`: send no sync messages at all.
    None,
    Full,
    Incremental,
}

fn sync_mode_from_kind(kind: TextDocumentSyncKind) -> SyncMode {
    match kind {
        TextDocumentSyncKind::FULL => SyncMode::Full,
        TextDocumentSyncKind::INCREMENTAL => SyncMode::Incremental,
        _ => SyncMode::None,
    }
}

/// The document-sync mode a server advertised, per the doc's
/// "`textDocumentSync` may be a number, an object, or absent" rule.
pub fn sync_mode(caps: &ServerCapabilities) -> SyncMode {
    match &caps.text_document_sync {
        None => SyncMode::None,
        Some(TextDocumentSyncCapability::Kind(kind)) => sync_mode_from_kind(*kind),
        Some(TextDocumentSyncCapability::Options(opts)) => {
            opts.change.map_or(SyncMode::None, sync_mode_from_kind)
        }
    }
}

/// Whether `didSave` should carry full text (`save: { includeText: true
/// }`) — `save` absent means no `didSave` at all; `save: true`/a bare
/// options object with no `includeText` means send `didSave` without
/// text.
pub fn save_includes_text(caps: &ServerCapabilities) -> bool {
    let Some(TextDocumentSyncCapability::Options(opts)) = &caps.text_document_sync else {
        return false;
    };
    matches!(
        &opts.save,
        Some(TextDocumentSyncSaveOptions::SaveOptions(o)) if o.include_text == Some(true)
    )
}

/// Whether the server advertised `save` support at all (bare `true` or an
/// options object) — gates whether `didSave` is sent, independent of
/// whether it carries text.
pub fn wants_did_save(caps: &ServerCapabilities) -> bool {
    match &caps.text_document_sync {
        Some(TextDocumentSyncCapability::Options(opts)) => matches!(
            &opts.save,
            Some(TextDocumentSyncSaveOptions::Supported(true))
                | Some(TextDocumentSyncSaveOptions::SaveOptions(_))
        ),
        _ => false,
    }
}

pub fn supports_definition(caps: &ServerCapabilities) -> bool {
    caps.definition_provider.is_some()
}

pub fn supports_hover(caps: &ServerCapabilities) -> bool {
    caps.hover_provider.is_some()
}

pub fn supports_completion(caps: &ServerCapabilities) -> bool {
    caps.completion_provider.is_some()
}

// ============================================================================
// Server -> client requests and notifications
// ============================================================================

/// The Phase 1 reply table for server-initiated requests
/// (lsp-integration.md "Server -> client requests"). Every incoming
/// request must get a reply or a real server hangs; anything not listed
/// here gets `MethodNotFound`.
///
/// `params` is only inspected for `workspace/configuration`, where the
/// reply must have one array entry per requested item.
pub fn reply_for_server_request(method: &str, params: &Value) -> Result<Value, JsonRpcError> {
    match method {
        "workspace/configuration" => {
            let item_count = params
                .get("items")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            Ok(Value::Array(vec![Value::Null; item_count]))
        }
        "client/registerCapability" | "client/unregisterCapability" => Ok(Value::Null),
        "window/workDoneProgress/create" => Ok(Value::Null),
        "workspace/applyEdit" => Ok(json!({ "applied": false })),
        "window/showMessageRequest" => Ok(Value::Null),
        _ => Err(JsonRpcError::method_not_found()),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

impl JsonRpcError {
    fn method_not_found() -> Self {
        Self {
            code: -32601,
            message: "Method not found".to_owned(),
        }
    }

    fn to_value(&self) -> Value {
        json!({ "code": self.code, "message": self.message })
    }
}

/// What to do with a server->client *notification* (no reply expected).
/// Everything but `$/progress` is either logged or silently dropped —
/// per the doc, a chatty server must never wake the render loop.
enum NotificationAction {
    /// `window/logMessage`/`showMessage`/`telemetry/event`: tracing only.
    Log,
    /// `$/progress`: drives `ServerState::Indexing`/`Ready`.
    Progress,
    /// `textDocument/publishDiagnostics`: forwarded as `Msg` (lsp-integration.md
    /// Phase 2) — the one notification besides `$/progress` allowed to wake
    /// the render loop, since it carries data the UI must show.
    PublishDiagnostics,
    /// Unknown notification: ignored silently.
    Ignore,
}

fn classify_notification(method: &str) -> NotificationAction {
    match method {
        "window/logMessage" | "window/showMessage" | "telemetry/event" => NotificationAction::Log,
        "$/progress" => NotificationAction::Progress,
        "textDocument/publishDiagnostics" => NotificationAction::PublishDiagnostics,
        _ => NotificationAction::Ignore,
    }
}

// ============================================================================
// Handshake outbound queueing
// ============================================================================

/// One frame the writer thread wants to send that is *not* a reply to a
/// server-initiated request (replies always bypass the gate — see
/// `Handshake ordering` in the design doc: a server can send
/// `workspace/configuration` before answering our `initialize`, and it
/// must get a reply or the test scenario the doc names deadlocks).
#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    Request {
        id: i64,
        method: String,
        params: Value,
    },
    Notification {
        method: String,
        params: Value,
    },
}

impl Frame {
    fn to_json(&self) -> Value {
        match self {
            Frame::Request { id, method, params } => {
                json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
            }
            Frame::Notification { method, params } => {
                json!({ "jsonrpc": "2.0", "method": method, "params": params })
            }
        }
    }
}

#[derive(Debug, PartialEq)]
enum GateState {
    /// Waiting for the `initialize` response. Nothing else may be
    /// written yet — `initialize` itself is sent once, outside the gate,
    /// before the worker loop starts.
    AwaitingInitializeResponse,
    /// Response arrived; queued frames stay queued until `initialized`
    /// has actually been written.
    AwaitingInitializedSent,
    Ready,
}

/// Queues outbound client->server traffic until the handshake completes,
/// then flushes it in order. Pure state machine — no I/O — so it's
/// tested in isolation from process spawning.
pub struct HandshakeGate {
    state: GateState,
    queue: VecDeque<Frame>,
}

impl HandshakeGate {
    pub fn new() -> Self {
        Self {
            state: GateState::AwaitingInitializeResponse,
            queue: VecDeque::new(),
        }
    }

    pub fn is_ready(&self) -> bool {
        self.state == GateState::Ready
    }

    /// Offers a frame for sending. Returns it back if it can go out
    /// immediately (gate already open), otherwise queues it and returns
    /// `None`.
    pub fn offer(&mut self, frame: Frame) -> Option<Frame> {
        if self.is_ready() {
            Some(frame)
        } else {
            self.queue.push_back(frame);
            None
        }
    }

    /// Call when the `initialize` response arrives. Does not unqueue
    /// anything yet — `initialized` must be written first.
    pub fn mark_initialize_response_received(&mut self) {
        debug_assert_eq!(self.state, GateState::AwaitingInitializeResponse);
        self.state = GateState::AwaitingInitializedSent;
    }

    /// Call right after writing `initialized`. Opens the gate and
    /// drains everything queued so far, in order.
    pub fn mark_initialized_sent(&mut self) -> Vec<Frame> {
        self.state = GateState::Ready;
        self.queue.drain(..).collect()
    }
}

impl Default for HandshakeGate {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Request-id correlation with abandoned-entry semantics
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct PendingEntry {
    pub method: String,
    /// Advisory only: a superseded/timed-out request stays in the map
    /// until its response arrives (see doc's "Requests, Guards,
    /// Timeouts") — the server still owns the id and will reply.
    pub abandoned: bool,
}

/// Tracks in-flight client->server requests by id. Entries are removed
/// only by `resolve` (the response arrived) or `take_all` (server
/// died) — never by `abandon`.
#[derive(Default)]
pub struct PendingRequests {
    next_id: i64,
    entries: HashMap<i64, PendingEntry>,
}

impl PendingRequests {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            entries: HashMap::new(),
        }
    }

    /// Allocates a fresh id and records the request as pending.
    pub fn begin(&mut self, method: impl Into<String>) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        self.entries.insert(
            id,
            PendingEntry {
                method: method.into(),
                abandoned: false,
            },
        );
        id
    }

    /// Marks a pending request abandoned (superseded or timed out).
    /// Returns `false` if `id` isn't pending (already resolved or
    /// unknown).
    pub fn abandon(&mut self, id: i64) -> bool {
        match self.entries.get_mut(&id) {
            Some(entry) => {
                entry.abandoned = true;
                true
            }
            None => false,
        }
    }

    /// Consumes and returns the entry for a response that just arrived.
    /// `None` means an unknown/duplicate id — logged and dropped by the
    /// caller, never a panic.
    pub fn resolve(&mut self, id: i64) -> Option<PendingEntry> {
        self.entries.remove(&id)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ============================================================================
// Root resolution
// ============================================================================

/// Canonicalizes when the path exists, otherwise returns it unchanged —
/// same fallback `lsp/uri.rs::path_to_uri` uses. Raw `PathBuf` comparison
/// breaks on macOS (`/tmp` -> `/private/tmp`) and home-dir symlinks: a
/// workspace opened through a symlink would miss the workspace-root
/// branch below and get charged against the detached-root cap instead.
fn canonicalize_or_self(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Workspace root if the file is under it; else the nearest ancestor
/// directory containing one of `project_markers`; else the file's own
/// parent directory. `marker_exists` is injected so this is testable
/// without touching the filesystem.
pub fn resolve_root(
    file_path: &Path,
    workspace_root: Option<&Path>,
    project_markers: &[&str],
    marker_exists: impl Fn(&Path) -> bool,
) -> PathBuf {
    if let Some(root) = workspace_root {
        let canonical_file = canonicalize_or_self(file_path);
        let canonical_root = canonicalize_or_self(root);
        if canonical_file.starts_with(&canonical_root) {
            return root.to_path_buf();
        }
    }

    let mut dir = file_path.parent();
    while let Some(candidate) = dir {
        if project_markers
            .iter()
            .any(|marker| marker_exists(&candidate.join(marker)))
        {
            return candidate.to_path_buf();
        }
        dir = candidate.parent();
    }

    file_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

// ============================================================================
// Command resolution (PATHEXT on Windows)
// ============================================================================

/// Pure search over a list of directories and extensions — the part of
/// `resolve_command` that's worth testing without touching the real
/// filesystem or `PATH`.
#[cfg(any(windows, test))]
fn resolve_in_dirs(
    command: &str,
    dirs: impl Iterator<Item = PathBuf>,
    extensions: &[String],
    exists: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    for dir in dirs {
        for ext in extensions {
            let candidate = dir.join(format!("{command}{ext}"));
            if exists(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

/// Resolves a server's `command` to an executable path.
///
/// On Windows, `Command::new("typescript-language-server")` fails to
/// find `typescript-language-server.cmd`: `CreateProcess` does not
/// consult `PATHEXT` the way a shell does. Elsewhere, `Command` already
/// resolves bare names against `PATH`, so this is a no-op passthrough.
#[cfg(windows)]
pub fn resolve_command(command: &str) -> PathBuf {
    if command.contains(std::path::MAIN_SEPARATOR) || Path::new(command).extension().is_some() {
        return PathBuf::from(command);
    }
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    let pathext_var = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_owned());
    let extensions: Vec<String> = pathext_var.split(';').map(str::to_owned).collect();
    resolve_in_dirs(
        command,
        std::env::split_paths(&path_var),
        &extensions,
        |p| p.is_file(),
    )
    .unwrap_or_else(|| PathBuf::from(command))
}

#[cfg(not(windows))]
pub fn resolve_command(command: &str) -> PathBuf {
    PathBuf::from(command)
}

// ============================================================================
// Worker: process spawn + threads
// ============================================================================

/// Commands the writer thread accepts, from either the main thread
/// (`ServerHandle::outbound_tx`, future didOpen/didChange/requests) or
/// the reader thread (auto-replies, the handshake-ready signal).
pub enum WorkerCmd {
    Notify {
        method: String,
        params: Value,
    },
    /// A client -> server request (`shutdown` today; future phases add
    /// definition/hover/completion). The id must already be registered
    /// in `PendingRequests` — `ServerHandle::begin_request` does both.
    Request {
        id: i64,
        method: String,
        params: Value,
    },
    /// Reader thread signals "the `initialize` response just arrived":
    /// writer sends `initialized`, then flushes the queue.
    HandshakeReady,
    /// Reader thread computed a reply to a server-initiated request.
    ReplyToServer {
        id: Value,
        result: Result<Value, JsonRpcError>,
    },
    /// Ask the writer to stop; used when tearing the server down.
    Shutdown,
}

/// What the main thread keeps for a running server. Not `Debug`/`Clone`
/// on purpose — lives only in the runtime's `LspManager`, never in
/// `AppModel` (see the design doc's Process Model).
pub struct ServerHandle {
    pub id: LspServerId,
    pub outbound_tx: Sender<WorkerCmd>,
    pub capabilities: Arc<Mutex<Option<lsp_types::ServerCapabilities>>>,
    /// Shared with the reader thread so the main thread can allocate ids
    /// for its own requests (`shutdown` today) without a second
    /// correlation table.
    pub pending: Arc<Mutex<PendingRequests>>,
    /// Distinguishes this incarnation from a later restart at the same
    /// `(id, root)` — the reader thread echoes it back in
    /// `LspMsg::ServerExited` so a deliberate kill's EOF doesn't get
    /// mistaken for the *replacement* process exiting and cause a
    /// spurious extra restart (`spawn_server`'s doc comment has the
    /// full race).
    pub generation: u64,
    child: Child,
}

impl ServerHandle {
    /// Snapshot of the parsed `InitializeResult.capabilities`, `None`
    /// until the `initialize` response has arrived. Every future send
    /// (`didOpen`/`didChange`/`didSave`, feature requests) gates on this
    /// rather than assuming a server supports everything.
    pub fn capabilities_snapshot(&self) -> Option<ServerCapabilities> {
        self.capabilities.lock().unwrap().clone()
    }

    /// Registers and sends a client -> server request, returning its id
    /// for correlation.
    pub fn begin_request(&self, method: impl Into<String>, params: Value) -> i64 {
        let method = method.into();
        let id = self.pending.lock().unwrap().begin(method.clone());
        let _ = self
            .outbound_tx
            .send(WorkerCmd::Request { id, method, params });
        id
    }

    /// The design doc's shutdown sequence: `shutdown` request -> await
    /// its response (capped) -> `exit` notification -> await process
    /// exit (capped) -> kill. Sending `exit` before the `shutdown`
    /// response makes rust-analyzer/gopls exit non-zero, indistinguishable
    /// from a crash — hence the wait in between.
    ///
    /// `await_shutdown_ack` polls `msg_rx` itself (blocking, up to
    /// `timeout`) rather than going through the normal async message
    /// loop: this only ever runs during quit teardown, where the loop
    /// has already stopped pumping messages for the frame.
    pub fn graceful_shutdown(
        &mut self,
        msg_rx: &Receiver<Msg>,
        timeout: std::time::Duration,
    ) -> bool {
        let _id = self.begin_request("shutdown", Value::Null);
        let deadline = std::time::Instant::now() + timeout;
        let mut acked = false;
        while std::time::Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            match msg_rx.recv_timeout(remaining) {
                Ok(Msg::Lsp(LspMsg::ShutdownAcked {
                    server_id,
                    generation,
                })) if server_id == self.id && generation == self.generation => {
                    acked = true;
                    break;
                }
                Ok(_) => continue, // unrelated traffic; the app is quitting anyway
                Err(_) => break,   // timeout or disconnected sender
            }
        }
        let _ = self.outbound_tx.send(WorkerCmd::Notify {
            method: "exit".to_owned(),
            params: Value::Null,
        });

        let exit_deadline = std::time::Instant::now() + timeout;
        let mut exited = false;
        while std::time::Instant::now() < exit_deadline {
            match self.child.try_wait() {
                Ok(Some(_)) => {
                    exited = true;
                    break;
                }
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(20)),
                Err(_) => break,
            }
        }
        if !exited {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        let _ = self.outbound_tx.send(WorkerCmd::Shutdown);
        acked && exited
    }

    /// Best-effort: ask the writer thread to stop and kill the process
    /// immediately, no handshake. Used for crash-restart (the old
    /// process is already misbehaving) and as `graceful_shutdown`'s own
    /// fallback; quit teardown prefers `graceful_shutdown`.
    pub fn kill(&mut self) {
        let _ = self.outbound_tx.send(WorkerCmd::Shutdown);
        let _ = self.child.kill();
        // Reap immediately: without wait(), a killed child stays a zombie
        // until the editor exits (up to MAX_RESTART_ATTEMPTS per root, plus
        // every manual restart).
        let _ = self.child.wait();
    }
}

static NEXT_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Spawns a server process and its worker threads. Returns once the
/// child is spawned and `initialize` has been written — the handshake
/// response arrives asynchronously via `msg_tx`.
///
/// Race note: killing a handle and spawning its replacement at the same
/// `(id, root)` races the killed process's own EOF against the caller
/// installing the new handle. `generation` (assigned here, echoed back
/// in `ServerExited`) lets the runtime tell "the process I just killed
/// exited" apart from "the process I just started already died" without
/// that race corrupting either handle.
pub fn spawn_server(
    command: &str,
    args: &[String],
    root: &Path,
    server_id: LspServerId,
    msg_tx: Sender<Msg>,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
) -> std::io::Result<ServerHandle> {
    let root_for_state = root.to_path_buf();
    let resolved_command = resolve_command(command);
    let mut cmd = Command::new(&resolved_command);
    cmd.args(args)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn()?;
    let stdin = child.stdin.take().expect("piped stdin");
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");

    let generation = NEXT_GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let (outbound_tx, outbound_rx) = std::sync::mpsc::channel::<WorkerCmd>();
    let capabilities = Arc::new(Mutex::new(None));
    let pending = Arc::new(Mutex::new(PendingRequests::new()));

    // stderr drain: prevents an undrained 64 KB pipe buffer from wedging
    // the child (rust-analyzer is chatty on stderr).
    {
        let server_id = server_id.clone();
        std::thread::Builder::new()
            .name(format!("lsp-stderr-{}", server_id.0))
            .spawn(move || drain_stderr(stderr, &server_id))?;
    }

    // Writer thread: sends `initialize` immediately (the one exception
    // to "nothing before the response"), then serves `outbound_rx`.
    {
        let server_id = server_id.clone();
        let root_uri = super::path_to_uri(root);
        let init_id = {
            let mut pending = pending.lock().unwrap();
            pending.begin("initialize")
        };
        std::thread::Builder::new()
            .name(format!("lsp-writer-{}", server_id.0))
            .spawn(move || {
                writer_loop(stdin, outbound_rx, init_id, root_uri, std::process::id())
            })?;
    }

    // Reader thread: parses frames, resolves pending requests, computes
    // auto-replies, and forwards `Msg`s to the main thread.
    {
        let server_id = server_id.clone();
        let capabilities = Arc::clone(&capabilities);
        let pending = Arc::clone(&pending);
        let outbound_tx_for_reader = outbound_tx.clone();
        std::thread::Builder::new()
            .name(format!("lsp-reader-{}", server_id.0))
            .spawn(move || {
                reader_loop(
                    stdout,
                    server_id,
                    root_for_state,
                    generation,
                    msg_tx,
                    wake,
                    capabilities,
                    pending,
                    outbound_tx_for_reader,
                )
            })?;
    }

    Ok(ServerHandle {
        id: server_id,
        outbound_tx,
        capabilities,
        pending,
        generation,
        child,
    })
}

fn drain_stderr(stderr: std::process::ChildStderr, server_id: &LspServerId) {
    let reader = BufReader::new(stderr);
    for line in reader.lines() {
        match line {
            Ok(line) => tracing::debug!("[{} stderr] {}", server_id.0, line),
            Err(_) => break,
        }
    }
}

fn writer_loop(
    mut stdin: ChildStdin,
    outbound_rx: Receiver<WorkerCmd>,
    init_id: i64,
    root_uri: lsp_types::Uri,
    process_id: u32,
) {
    let mut gate = HandshakeGate::new();

    let init_params = json!({
        "processId": process_id,
        "rootUri": root_uri.as_str(),
        "rootPath": uri_to_root_path(&root_uri),
        "workspaceFolders": [{ "uri": root_uri.as_str(), "name": root_uri.as_str() }],
        "capabilities": client_capabilities(),
    });
    let init_frame = Frame::Request {
        id: init_id,
        method: "initialize".to_owned(),
        params: init_params,
    };
    if write_message(&mut stdin, &init_frame.to_json()).is_err() {
        return;
    }

    while let Ok(cmd) = outbound_rx.recv() {
        match cmd {
            WorkerCmd::HandshakeReady => {
                gate.mark_initialize_response_received();
                let initialized =
                    json!({ "jsonrpc": "2.0", "method": "initialized", "params": {} });
                if write_message(&mut stdin, &initialized).is_err() {
                    return;
                }
                for frame in gate.mark_initialized_sent() {
                    if write_message(&mut stdin, &frame.to_json()).is_err() {
                        return;
                    }
                }
            }
            WorkerCmd::Notify { method, params } => {
                if let Some(frame) = gate.offer(Frame::Notification { method, params }) {
                    if write_message(&mut stdin, &frame.to_json()).is_err() {
                        return;
                    }
                }
            }
            WorkerCmd::Request { id, method, params } => {
                if let Some(frame) = gate.offer(Frame::Request { id, method, params }) {
                    if write_message(&mut stdin, &frame.to_json()).is_err() {
                        return;
                    }
                }
            }
            WorkerCmd::ReplyToServer { id, result } => {
                // Replies to server-initiated requests bypass the gate:
                // a server can ask `workspace/configuration` before
                // answering our `initialize`, and must get a reply to
                // make progress (see Handshake ordering in the doc).
                let body = match result {
                    Ok(value) => json!({ "jsonrpc": "2.0", "id": id, "result": value }),
                    Err(err) => json!({ "jsonrpc": "2.0", "id": id, "error": err.to_value() }),
                };
                if write_message(&mut stdin, &body).is_err() {
                    return;
                }
            }
            WorkerCmd::Shutdown => return,
        }
    }
}

fn uri_to_root_path(uri: &lsp_types::Uri) -> Option<String> {
    super::uri_to_path(uri).map(|p| p.to_string_lossy().into_owned())
}

#[allow(clippy::too_many_arguments)]
fn reader_loop(
    stdout: ChildStdout,
    server_id: LspServerId,
    root: PathBuf,
    generation: u64,
    msg_tx: Sender<Msg>,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
    capabilities: Arc<Mutex<Option<lsp_types::ServerCapabilities>>>,
    pending: Arc<Mutex<PendingRequests>>,
    outbound_tx: Sender<WorkerCmd>,
) {
    let mut reader = BufReader::new(stdout);
    loop {
        let message = match read_message(&mut reader) {
            Ok(message) => message,
            Err(_) => {
                // EOF or malformed frame: treat as server exit; the
                // runtime's `LspManager` owns backoff/restart.
                let _ = msg_tx.send(Msg::Lsp(LspMsg::ServerExited {
                    server_id: server_id.clone(),
                    generation,
                }));
                if let Some(wake) = wake.as_deref() {
                    wake();
                }
                break;
            }
        };

        if let Some(method) = message.get("method").and_then(Value::as_str) {
            if let Some(id) = message.get("id").cloned() {
                // Server -> client request: always needs a reply.
                let params = message.get("params").cloned().unwrap_or(Value::Null);
                let result = reply_for_server_request(method, &params);
                let _ = outbound_tx.send(WorkerCmd::ReplyToServer { id, result });
            } else {
                // Notification.
                match classify_notification(method) {
                    NotificationAction::Log => {
                        tracing::debug!(
                            "[{}] {}: {:?}",
                            server_id.0,
                            method,
                            message.get("params")
                        );
                    }
                    NotificationAction::Progress => {
                        handle_progress(&server_id, &root, &message, &msg_tx, wake.as_deref());
                    }
                    NotificationAction::PublishDiagnostics => {
                        handle_publish_diagnostics(&message, &msg_tx, wake.as_deref());
                    }
                    NotificationAction::Ignore => {}
                }
            }
            continue;
        }

        // Response to one of our requests.
        if let Some(id) = message.get("id").and_then(Value::as_i64) {
            let entry = pending.lock().unwrap().resolve(id);
            let Some(entry) = entry else {
                tracing::debug!("[{}] response for unknown id {}", server_id.0, id);
                continue;
            };
            if entry.method == "initialize" {
                if let Some(error) = message.get("error") {
                    // A server that rejects `initialize` is not healthy —
                    // never open the handshake gate or report Ready for
                    // it; leave capabilities None so every send stays
                    // gated off, and surface the failure like a crash.
                    tracing::warn!("[{}] initialize failed: {:?}", server_id.0, error);
                    send_state(
                        &server_id,
                        &root,
                        ServerState::Failed,
                        &msg_tx,
                        wake.as_deref(),
                    );
                    continue;
                }
                let result = message.get("result").cloned();
                let parsed: Option<lsp_types::ServerCapabilities> = result
                    .and_then(|r| r.get("capabilities").cloned())
                    .and_then(|c| serde_json::from_value(c).ok());
                *capabilities.lock().unwrap() = parsed;
                let _ = outbound_tx.send(WorkerCmd::HandshakeReady);
                send_state(
                    &server_id,
                    &root,
                    ServerState::Ready,
                    &msg_tx,
                    wake.as_deref(),
                );
            } else if entry.method == "shutdown" {
                // Quit teardown (`ServerHandle::graceful_shutdown`) polls
                // `msg_rx` directly for this rather than going through
                // `update()` — see its doc comment.
                let _ = msg_tx.send(Msg::Lsp(LspMsg::ShutdownAcked {
                    server_id: server_id.clone(),
                    generation,
                }));
                if let Some(wake) = wake.as_deref() {
                    wake();
                }
            }
            // Other request kinds (definition/hover/completion) are
            // routed by future phases.
        }
    }
}

fn handle_progress(
    server_id: &LspServerId,
    root: &Path,
    message: &Value,
    msg_tx: &Sender<Msg>,
    wake: Option<&(dyn Fn() + Send + Sync)>,
) {
    let kind = message
        .pointer("/params/value/kind")
        .and_then(Value::as_str);
    match kind {
        Some("begin") => send_state(server_id, root, ServerState::Indexing, msg_tx, wake),
        Some("end") => send_state(server_id, root, ServerState::Ready, msg_tx, wake),
        _ => {}
    }
}

/// Parses a `textDocument/publishDiagnostics` notification's `params` into
/// `(uri, version, diagnostics)`. Pulled out of `handle_publish_diagnostics`
/// so it's testable without spinning up threads/channels.
fn parse_publish_diagnostics(
    params: &Value,
) -> Option<(lsp_types::Uri, Option<i64>, Vec<lsp_types::Diagnostic>)> {
    let parsed: lsp_types::PublishDiagnosticsParams =
        serde_json::from_value(params.clone()).ok()?;
    Some((
        parsed.uri,
        parsed.version.map(i64::from),
        parsed.diagnostics,
    ))
}

/// `textDocument/publishDiagnostics`: forwarded straight to `Msg` — the
/// runtime's `LspManager` (not this worker) owns the authoritative store
/// and the out-of-order/version bookkeeping (see `LspMsg::DiagnosticsPublished`'s
/// doc comment).
///
/// ponytail: successive publishes for the same URI aren't coalesced here
/// before sending — each one becomes its own `Msg`. Correctness is
/// unaffected (each publish is a full replacement, so processing them in
/// arrival order converges on the same end state as coalescing would);
/// this only costs redundant projection/redraw work if a server floods
/// publishes for one URI within a single drain of `msg_rx`. Add
/// last-wins coalescing in the `App::process_async_messages` drain loop
/// if profiling ever shows that redundant work matters.
fn handle_publish_diagnostics(
    message: &Value,
    msg_tx: &Sender<Msg>,
    wake: Option<&(dyn Fn() + Send + Sync)>,
) {
    let Some(params) = message.get("params") else {
        return;
    };
    let Some((uri, version, diagnostics)) = parse_publish_diagnostics(params) else {
        return;
    };
    if msg_tx
        .send(Msg::Lsp(LspMsg::DiagnosticsPublished {
            uri,
            version,
            diagnostics,
        }))
        .is_err()
    {
        return;
    }
    if let Some(wake) = wake {
        wake();
    }
}

fn send_state(
    server_id: &LspServerId,
    root: &Path,
    state: ServerState,
    msg_tx: &Sender<Msg>,
    wake: Option<&(dyn Fn() + Send + Sync)>,
) {
    if msg_tx
        .send(Msg::Lsp(LspMsg::ServerStateChanged {
            server_id: server_id.clone(),
            root: root.to_path_buf(),
            state,
        }))
        .is_err()
    {
        return;
    }
    if let Some(wake) = wake {
        wake();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // ---- client_capabilities: pins the exact advertised shape ----

    #[test]
    fn advertises_utf16_position_encoding_only() {
        let caps = client_capabilities();
        assert_eq!(
            caps.general.unwrap().position_encodings,
            Some(vec![PositionEncodingKind::UTF16])
        );
    }

    #[test]
    fn advertises_definition_without_link_support() {
        let caps = client_capabilities();
        let def = caps.text_document.unwrap().definition.unwrap();
        assert_eq!(def.link_support, Some(false));
    }

    #[test]
    fn advertises_completion_without_snippet_support() {
        let caps = client_capabilities();
        let item = caps
            .text_document
            .unwrap()
            .completion
            .unwrap()
            .completion_item
            .unwrap();
        assert_eq!(item.snippet_support, Some(false));
        assert!(item.resolve_support.is_some());
    }

    #[test]
    fn dynamic_registration_is_false_everywhere_it_is_set() {
        let caps = client_capabilities();
        let text_document = caps.text_document.unwrap();
        assert_eq!(
            text_document.synchronization.unwrap().dynamic_registration,
            Some(false)
        );
        assert_eq!(
            text_document.definition.unwrap().dynamic_registration,
            Some(false)
        );
        assert_eq!(
            text_document.hover.unwrap().dynamic_registration,
            Some(false)
        );
        assert_eq!(
            text_document.completion.unwrap().dynamic_registration,
            Some(false)
        );
        assert_eq!(
            caps.workspace
                .unwrap()
                .did_change_watched_files
                .unwrap()
                .dynamic_registration,
            Some(false)
        );
    }

    #[test]
    fn advertises_work_done_progress() {
        let caps = client_capabilities();
        assert_eq!(caps.window.unwrap().work_done_progress, Some(true));
    }

    // ---- server capability gating ----

    fn caps_with_sync(sync: Option<TextDocumentSyncCapability>) -> ServerCapabilities {
        ServerCapabilities {
            text_document_sync: sync,
            ..Default::default()
        }
    }

    #[test]
    fn sync_mode_is_none_when_field_absent() {
        assert_eq!(sync_mode(&caps_with_sync(None)), SyncMode::None);
    }

    #[test]
    fn sync_mode_reads_bare_number() {
        let caps = caps_with_sync(Some(TextDocumentSyncCapability::Kind(
            TextDocumentSyncKind::FULL,
        )));
        assert_eq!(sync_mode(&caps), SyncMode::Full);
    }

    #[test]
    fn sync_mode_reads_options_object() {
        let caps = caps_with_sync(Some(TextDocumentSyncCapability::Options(
            lsp_types::TextDocumentSyncOptions {
                change: Some(TextDocumentSyncKind::INCREMENTAL),
                ..Default::default()
            },
        )));
        assert_eq!(sync_mode(&caps), SyncMode::Incremental);
    }

    #[test]
    fn save_includes_text_requires_explicit_include_text_true() {
        let no_save = caps_with_sync(Some(TextDocumentSyncCapability::Options(
            lsp_types::TextDocumentSyncOptions::default(),
        )));
        assert!(!save_includes_text(&no_save));
        assert!(!wants_did_save(&no_save));

        let bare_save = caps_with_sync(Some(TextDocumentSyncCapability::Options(
            lsp_types::TextDocumentSyncOptions {
                save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                ..Default::default()
            },
        )));
        assert!(!save_includes_text(&bare_save));
        assert!(wants_did_save(&bare_save));

        let full_save = caps_with_sync(Some(TextDocumentSyncCapability::Options(
            lsp_types::TextDocumentSyncOptions {
                save: Some(TextDocumentSyncSaveOptions::SaveOptions(
                    lsp_types::SaveOptions {
                        include_text: Some(true),
                    },
                )),
                ..Default::default()
            },
        )));
        assert!(save_includes_text(&full_save));
        assert!(wants_did_save(&full_save));
    }

    #[test]
    fn feature_support_reads_provider_presence() {
        let caps = ServerCapabilities {
            definition_provider: Some(lsp_types::OneOf::Left(true)),
            ..Default::default()
        };
        assert!(supports_definition(&caps));
        assert!(!supports_hover(&caps));
        assert!(!supports_completion(&caps));
    }

    // ---- server -> client reply table ----

    #[test]
    fn workspace_configuration_replies_with_one_null_per_item() {
        let params = json!({ "items": [{}, {}, {}] });
        let reply = reply_for_server_request("workspace/configuration", &params).unwrap();
        assert_eq!(reply, json!([null, null, null]));
    }

    #[test]
    fn register_capability_replies_null() {
        assert_eq!(
            reply_for_server_request("client/registerCapability", &Value::Null).unwrap(),
            Value::Null
        );
        assert_eq!(
            reply_for_server_request("client/unregisterCapability", &Value::Null).unwrap(),
            Value::Null
        );
    }

    #[test]
    fn apply_edit_replies_not_applied() {
        let reply = reply_for_server_request("workspace/applyEdit", &Value::Null).unwrap();
        assert_eq!(reply, json!({ "applied": false }));
    }

    #[test]
    fn unknown_request_is_method_not_found() {
        let err = reply_for_server_request("textDocument/foldingRange", &Value::Null).unwrap_err();
        assert_eq!(err.code, -32601);
    }

    // ---- handshake gate ----

    #[test]
    fn queues_notifications_until_initialized_is_sent() {
        let mut gate = HandshakeGate::new();
        let frame = Frame::Notification {
            method: "textDocument/didOpen".to_owned(),
            params: json!({}),
        };
        assert!(gate.offer(frame.clone()).is_none());
        assert!(!gate.is_ready());

        gate.mark_initialize_response_received();
        // Still queued: `initialized` hasn't been sent yet.
        let another = Frame::Notification {
            method: "textDocument/didChange".to_owned(),
            params: json!({}),
        };
        assert!(gate.offer(another.clone()).is_none());

        let flushed = gate.mark_initialized_sent();
        assert_eq!(flushed, vec![frame, another]);
        assert!(gate.is_ready());
    }

    #[test]
    fn frames_offered_after_ready_pass_straight_through() {
        let mut gate = HandshakeGate::new();
        gate.mark_initialize_response_received();
        gate.mark_initialized_sent();

        let frame = Frame::Notification {
            method: "textDocument/didSave".to_owned(),
            params: json!({}),
        };
        assert_eq!(gate.offer(frame.clone()), Some(frame));
    }

    // ---- pending requests: abandoned-entry semantics ----

    #[test]
    fn abandoned_entries_stay_pending_until_resolved() {
        let mut pending = PendingRequests::new();
        let id = pending.begin("textDocument/definition");

        assert!(pending.abandon(id));
        assert_eq!(pending.len(), 1); // still in the map: cancel is advisory

        let entry = pending.resolve(id).unwrap();
        assert!(entry.abandoned);
        assert!(pending.is_empty());
    }

    #[test]
    fn abandon_of_unknown_id_is_a_no_op() {
        let mut pending = PendingRequests::new();
        assert!(!pending.abandon(999));
    }

    #[test]
    fn resolve_of_unknown_id_returns_none_not_a_panic() {
        let mut pending = PendingRequests::new();
        assert!(pending.resolve(42).is_none());
    }

    #[test]
    fn ids_are_unique_and_increasing() {
        let mut pending = PendingRequests::new();
        let a = pending.begin("initialize");
        let b = pending.begin("shutdown");
        assert_ne!(a, b);
        assert!(b > a);
    }

    // ---- root resolution ----

    #[test]
    fn prefers_workspace_root_when_file_is_under_it() {
        let root = resolve_root(
            Path::new("/ws/src/main.rs"),
            Some(Path::new("/ws")),
            &["Cargo.toml"],
            |_| false,
        );
        assert_eq!(root, PathBuf::from("/ws"));
    }

    #[test]
    fn falls_back_to_nearest_project_marker_outside_the_workspace() {
        let markers = ["Cargo.toml"];
        let root = resolve_root(
            Path::new("/home/user/other-project/src/main.rs"),
            Some(Path::new("/ws")),
            &markers,
            |p| p == Path::new("/home/user/other-project/Cargo.toml"),
        );
        assert_eq!(root, PathBuf::from("/home/user/other-project"));
    }

    #[test]
    fn falls_back_to_file_parent_when_no_marker_and_no_workspace() {
        let root = resolve_root(
            Path::new("/tmp/scratch/foo.rs"),
            None,
            &["Cargo.toml"],
            |_| false,
        );
        assert_eq!(root, PathBuf::from("/tmp/scratch"));
    }

    #[test]
    fn walks_multiple_ancestors_to_find_a_marker() {
        let root = resolve_root(
            Path::new("/a/b/c/d/file.py"),
            None,
            &["pyproject.toml"],
            |p| p == Path::new("/a/b/pyproject.toml"),
        );
        assert_eq!(root, PathBuf::from("/a/b"));
    }

    #[cfg(unix)]
    #[test]
    fn workspace_root_matches_through_a_symlink() {
        // A file opened via a symlinked path (e.g. macOS's /tmp ->
        // /private/tmp) must still resolve to the workspace root — raw
        // PathBuf::starts_with would miss this and charge it against the
        // detached-root cap instead.
        let dir = tempfile::tempdir().unwrap();
        let real_root = dir.path().join("real_root");
        std::fs::create_dir(&real_root).unwrap();
        let src_dir = real_root.join("src");
        std::fs::create_dir(&src_dir).unwrap();
        let file_path = src_dir.join("main.rs");
        std::fs::write(&file_path, b"fn main() {}").unwrap();

        let link_root = dir.path().join("link_root");
        std::os::unix::fs::symlink(&real_root, &link_root).unwrap();
        let file_via_link = link_root.join("src").join("main.rs");

        let root = resolve_root(&file_via_link, Some(&real_root), &["Cargo.toml"], |_| false);
        assert_eq!(root, real_root);
    }

    // ---- command resolution ----

    #[test]
    fn resolve_in_dirs_finds_the_first_matching_extension() {
        let dirs = vec![PathBuf::from("/usr/bin"), PathBuf::from("/usr/local/bin")];
        let extensions = vec![".EXE".to_owned(), ".CMD".to_owned()];
        let found = resolve_in_dirs("pyright-langserver", dirs.into_iter(), &extensions, |p| {
            p == Path::new("/usr/local/bin/pyright-langserver.CMD")
        });
        assert_eq!(
            found,
            Some(PathBuf::from("/usr/local/bin/pyright-langserver.CMD"))
        );
    }

    #[test]
    fn resolve_in_dirs_returns_none_when_nothing_matches() {
        let dirs = vec![PathBuf::from("/usr/bin")];
        let extensions = vec![".EXE".to_owned()];
        let found = resolve_in_dirs("nope", dirs.into_iter(), &extensions, |_| false);
        assert!(found.is_none());
    }

    #[cfg(not(windows))]
    #[test]
    fn resolve_command_is_a_passthrough_off_windows() {
        assert_eq!(
            resolve_command("rust-analyzer"),
            PathBuf::from("rust-analyzer")
        );
    }

    // ---- integration: a real child process speaking the handshake ----

    /// A tiny in-process "fake server": reads one framed `initialize`
    /// request off stdin and writes back a minimal, well-formed
    /// response with the same id — proves `spawn_server`'s three
    /// threads (writer/reader/stderr) actually wire together against a
    /// real child process and drive the handshake to `Ready`, without
    /// needing a scripted fake LSP server binary on `PATH`.
    #[cfg(unix)]
    #[test]
    fn spawn_server_completes_the_handshake_against_a_real_child() {
        // `sh -c` running a small reader/writer pipeline: read headers
        // until the blank line (to find Content-Length), read the body,
        // then reply with a fixed response reusing whatever id was sent
        // — good enough since Phase 1 only ever sends `initialize` first.
        let script = r#"
IFS= read -r cl
cl=$(echo "$cl" | tr -d '\r' | sed 's/Content-Length: //')
IFS= read -r blank
body=$(dd bs=1 count="$cl" 2>/dev/null)
id=$(echo "$body" | grep -o '"id":[0-9]*' | head -1 | grep -o '[0-9]*')
resp='{"jsonrpc":"2.0","id":'"$id"',"result":{"capabilities":{}}}'
len=${#resp}
printf 'Content-Length: %d\r\n\r\n%s' "$len" "$resp"
"#;
        let (msg_tx, msg_rx) = std::sync::mpsc::channel();
        let dir = std::env::temp_dir();
        let mut handle = spawn_server(
            "sh",
            &["-c".to_owned(), script.to_owned()],
            &dir,
            LspServerId::from("fake-server"),
            msg_tx,
            None,
        )
        .expect("failed to spawn fake server");

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut saw_ready = false;
        while std::time::Instant::now() < deadline {
            match msg_rx.recv_timeout(Duration::from_millis(200)) {
                Ok(Msg::Lsp(LspMsg::ServerStateChanged {
                    state: ServerState::Ready,
                    ..
                })) => {
                    saw_ready = true;
                    break;
                }
                Ok(_) => {}
                Err(_) => {}
            }
        }
        assert!(saw_ready, "expected ServerStateChanged(Ready) within 5s");
        handle.kill();
    }

    // ---- publishDiagnostics ----

    #[test]
    fn classifies_publish_diagnostics_distinctly_from_log_and_progress() {
        assert!(matches!(
            classify_notification("textDocument/publishDiagnostics"),
            NotificationAction::PublishDiagnostics
        ));
        assert!(matches!(
            classify_notification("$/progress"),
            NotificationAction::Progress
        ));
        assert!(matches!(
            classify_notification("window/logMessage"),
            NotificationAction::Log
        ));
    }

    #[test]
    fn parses_publish_diagnostics_params_including_version_and_severity() {
        let params = json!({
            "uri": "file:///tmp/foo.rs",
            "version": 3,
            "diagnostics": [{
                "range": {
                    "start": { "line": 1, "character": 0 },
                    "end": { "line": 1, "character": 5 }
                },
                "severity": 1,
                "message": "unresolved import",
            }],
        });
        let (uri, version, diagnostics) =
            parse_publish_diagnostics(&params).expect("valid publishDiagnostics params");
        assert_eq!(uri.as_str(), "file:///tmp/foo.rs");
        assert_eq!(version, Some(3));
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, "unresolved import");
        assert_eq!(
            diagnostics[0].severity,
            Some(lsp_types::DiagnosticSeverity::ERROR)
        );
    }

    #[test]
    fn parses_publish_diagnostics_params_without_a_version() {
        let params = json!({
            "uri": "file:///tmp/foo.rs",
            "diagnostics": [],
        });
        let (_, version, diagnostics) =
            parse_publish_diagnostics(&params).expect("valid publishDiagnostics params");
        assert_eq!(version, None);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn rejects_malformed_publish_diagnostics_params() {
        assert!(parse_publish_diagnostics(&json!({ "not": "valid" })).is_none());
    }
}
