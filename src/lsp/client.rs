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
    ClientCapabilities, CodeActionClientCapabilities, CodeActionKind, CodeActionKindLiteralSupport,
    CodeActionLiteralSupport, CompletionClientCapabilities, CompletionItemCapability,
    CompletionItemCapabilityResolveSupport, DidChangeWatchedFilesClientCapabilities,
    DynamicRegistrationClientCapabilities, GeneralClientCapabilities, GotoCapability,
    HoverClientCapabilities, MarkupKind, ParameterInformationSettings, PositionEncodingKind,
    PublishDiagnosticsClientCapabilities, RenameClientCapabilities, ServerCapabilities,
    SignatureHelpClientCapabilities, SignatureInformationSettings, TagSupport,
    TextDocumentClientCapabilities, TextDocumentSyncCapability, TextDocumentSyncClientCapabilities,
    TextDocumentSyncKind, TextDocumentSyncSaveOptions, WindowClientCapabilities,
    WorkspaceClientCapabilities,
};
use serde_json::{json, Value};

use super::transport::{read_message, write_message};
use super::{LspServerId, ServerState};
use crate::messages::{LspMsg, Msg};
use crate::model::{SpanStyle, StyledText};

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
            references: Some(DynamicRegistrationClientCapabilities {
                dynamic_registration: Some(false),
            }),
            rename: Some(RenameClientCapabilities {
                dynamic_registration: Some(false),
                prepare_support: Some(true),
                prepare_support_default_behavior: None,
                honors_change_annotations: None,
            }),
            code_action: Some(CodeActionClientCapabilities {
                dynamic_registration: Some(false),
                code_action_literal_support: Some(CodeActionLiteralSupport {
                    code_action_kind: CodeActionKindLiteralSupport {
                        value_set: [
                            CodeActionKind::EMPTY,
                            CodeActionKind::QUICKFIX,
                            CodeActionKind::REFACTOR,
                            CodeActionKind::REFACTOR_EXTRACT,
                            CodeActionKind::REFACTOR_INLINE,
                            CodeActionKind::REFACTOR_REWRITE,
                            CodeActionKind::SOURCE,
                            CodeActionKind::SOURCE_ORGANIZE_IMPORTS,
                            CodeActionKind::SOURCE_FIX_ALL,
                        ]
                        .iter()
                        .map(|k| k.as_str().to_owned())
                        .collect(),
                    },
                }),
                is_preferred_support: Some(true),
                data_support: Some(false),
                ..Default::default()
            }),
            signature_help: Some(SignatureHelpClientCapabilities {
                dynamic_registration: Some(false),
                signature_information: Some(SignatureInformationSettings {
                    documentation_format: Some(vec![MarkupKind::PlainText, MarkupKind::Markdown]),
                    parameter_information: Some(ParameterInformationSettings {
                        label_offset_support: Some(true),
                    }),
                    active_parameter_support: Some(true),
                }),
                context_support: Some(true),
            }),
            completion: Some(CompletionClientCapabilities {
                dynamic_registration: Some(false),
                completion_item: Some(CompletionItemCapability {
                    snippet_support: Some(false),
                    commit_characters_support: Some(true),
                    preselect_support: Some(true),
                    label_details_support: Some(true),
                    resolve_support: Some(CompletionItemCapabilityResolveSupport {
                        properties: vec![
                            "documentation".to_owned(),
                            "detail".to_owned(),
                            "labelDetails".to_owned(),
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
            symbol: Some(lsp_types::WorkspaceSymbolClientCapabilities {
                dynamic_registration: Some(false),
                ..Default::default()
            }),
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

pub fn supports_references(caps: &ServerCapabilities) -> bool {
    caps.references_provider.is_some()
}

pub fn supports_workspace_symbols(caps: &ServerCapabilities) -> bool {
    matches!(
        caps.workspace_symbol_provider,
        Some(lsp_types::OneOf::Left(true) | lsp_types::OneOf::Right(_))
    )
}

pub fn supports_completion(caps: &ServerCapabilities) -> bool {
    caps.completion_provider.is_some()
}

pub fn supports_signature_help(caps: &ServerCapabilities) -> bool {
    caps.signature_help_provider.is_some()
}

pub fn supports_rename(caps: &ServerCapabilities) -> bool {
    caps.rename_provider.is_some()
}

/// `renameProvider.prepareProvider == true` — the server can validate the
/// symbol under the caret and suggest a placeholder before we prompt.
pub fn supports_prepare_rename(caps: &ServerCapabilities) -> bool {
    matches!(
        caps.rename_provider.as_ref(),
        Some(lsp_types::OneOf::Right(opts)) if opts.prepare_provider == Some(true)
    )
}

pub fn supports_code_action(caps: &ServerCapabilities) -> bool {
    caps.code_action_provider.is_some()
}

pub fn supports_formatting(caps: &ServerCapabilities) -> bool {
    caps.document_formatting_provider.is_some()
}

pub fn supports_range_formatting(caps: &ServerCapabilities) -> bool {
    caps.document_range_formatting_provider.is_some()
}

/// `(triggerCharacters, retriggerCharacters)` of the server's
/// `signatureHelpProvider` — both empty when absent.
pub fn signature_help_triggers(caps: &ServerCapabilities) -> (Vec<String>, Vec<String>) {
    let Some(p) = caps.signature_help_provider.as_ref() else {
        return (Vec::new(), Vec::new());
    };
    (
        p.trigger_characters.clone().unwrap_or_default(),
        p.retrigger_characters.clone().unwrap_or_default(),
    )
}

/// Whether the server advertised `completionProvider.resolveProvider` —
/// gates resolve-before-accept (ts-ls's auto-import `additionalTextEdits`
/// only exist after a resolve round trip).
pub fn supports_completion_resolve(caps: &ServerCapabilities) -> bool {
    caps.completion_provider
        .as_ref()
        .and_then(|p| p.resolve_provider)
        .unwrap_or(false)
}

/// The trigger characters a server advertised (e.g. `.` for member
/// access). Empty when none — typing them keeps the menu open and tags
/// the re-request, per lsp-integration.md Phase 5.
pub fn completion_trigger_characters(caps: &ServerCapabilities) -> Vec<String> {
    caps.completion_provider
        .as_ref()
        .and_then(|p| p.trigger_characters.clone())
        .unwrap_or_default()
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
/// reply must have one array entry per requested item: each item's dotted
/// `section` is looked up in this server's configured `settings`
/// (`lsp.servers.<id>.settings`), missing paths answering `null` — the
/// same reply an unconfigured server always got.
pub fn reply_for_server_request(
    method: &str,
    params: &Value,
    settings: &Value,
) -> Result<Value, JsonRpcError> {
    match method {
        "workspace/configuration" => {
            let items = params.get("items").and_then(Value::as_array);
            let results = items
                .map(|items| {
                    items
                        .iter()
                        .map(|item| {
                            configuration_section_value(
                                settings,
                                item.get("section").and_then(Value::as_str),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            Ok(Value::Array(results))
        }
        "client/registerCapability" | "client/unregisterCapability" => Ok(Value::Null),
        "window/workDoneProgress/create" => Ok(Value::Null),
        "workspace/applyEdit" => Ok(json!({ "applied": false })),
        "window/showMessageRequest" => Ok(Value::Null),
        _ => Err(JsonRpcError::method_not_found()),
    }
}

/// Looks up a dotted `section` path (`"rust-analyzer.cargo.allTargets"`)
/// in the configured settings object. A missing path, a section that
/// walks through a non-object, or no configured settings at all answers
/// `Null` — the spec's "null if the client doesn't have the setting", and
/// the reply every server got before per-server settings existed. No
/// section means "everything you have": the whole object (or null).
fn configuration_section_value(settings: &Value, section: Option<&str>) -> Value {
    let Some(section) = section else {
        return settings.clone();
    };
    let mut current = settings;
    for part in section.split('.') {
        match current {
            Value::Object(map) => match map.get(part) {
                Some(value) => current = value,
                None => return Value::Null,
            },
            _ => return Value::Null,
        }
    }
    current.clone()
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
    ///
    /// `shared_deadline` bounds *total* quit latency across every server
    /// being torn down (the caller computes one deadline and passes it to
    /// every call) — `timeout` still caps each phase (shutdown-ack,
    /// exit-wait) individually, but the two combine as a minimum: a
    /// server reached late in the teardown loop gets whatever's left of
    /// `shared_deadline`, never the full `timeout` again on top of what
    /// prior servers already spent. Without this, N servers pay up to
    /// `2 * timeout` sequentially instead of `2 * timeout` in total.
    pub fn graceful_shutdown(
        &mut self,
        msg_rx: &Receiver<Msg>,
        timeout: std::time::Duration,
        shared_deadline: std::time::Instant,
    ) -> bool {
        let _id = self.begin_request("shutdown", Value::Null);
        let deadline = (std::time::Instant::now() + timeout).min(shared_deadline);
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

        let exit_deadline = (std::time::Instant::now() + timeout).min(shared_deadline);
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
#[allow(clippy::too_many_arguments)]
pub fn spawn_server(
    command: &str,
    args: &[String],
    root: &Path,
    server_id: LspServerId,
    msg_tx: Sender<Msg>,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
    initialization_options: Value,
    settings: Value,
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
                writer_loop(
                    stdin,
                    outbound_rx,
                    init_id,
                    root_uri,
                    std::process::id(),
                    initialization_options,
                )
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
                    settings,
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
    initialization_options: Value,
) {
    let mut gate = HandshakeGate::new();

    let mut init_params = json!({
        "processId": process_id,
        "rootUri": root_uri.as_str(),
        "rootPath": uri_to_root_path(&root_uri),
        "workspaceFolders": [{ "uri": root_uri.as_str(), "name": root_uri.as_str() }],
        "capabilities": client_capabilities(),
    });
    if !initialization_options.is_null() {
        init_params["initializationOptions"] = initialization_options;
    }
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
    settings: Value,
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
                if method == "workspace/applyEdit" {
                    // Needs the model: forwarded to `update()`, which
                    // answers via `Cmd::LspRespondToServer`.
                    let result =
                        match serde_json::from_value::<lsp_types::ApplyWorkspaceEditParams>(params)
                        {
                            Ok(p) => {
                                let sent = msg_tx.send(Msg::Lsp(LspMsg::ApplyEditRequested {
                                    server_id: server_id.clone(),
                                    root: root.clone(),
                                    request_id: id,
                                    edit: Box::new(p.edit),
                                    label: p.label,
                                }));
                                if sent.is_ok() {
                                    if let Some(wake) = wake.as_deref() {
                                        wake();
                                    }
                                }
                                continue;
                            }
                            Err(e) => Ok(json!({
                                "applied": false,
                                "failureReason": format!("invalid params: {e}"),
                            })),
                        };
                    let _ = outbound_tx.send(WorkerCmd::ReplyToServer { id, result });
                    continue;
                }
                let result = reply_for_server_request(method, &params, &settings);
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
                    // it. Routed through the same `ServerExited` path a
                    // crash takes (rather than a one-off `Failed` and
                    // `continue`) so the runtime's normal backoff/kill/
                    // cleanup machinery applies: without this the handle
                    // stayed in `LspManager::servers` forever with
                    // capabilities permanently `None`, so every future
                    // sync send stayed queued in the writer's unbounded
                    // `VecDeque` behind a gate that could never open, and
                    // quit teardown blocked the full shutdown timeout on
                    // an ack that could never arrive.
                    tracing::warn!("[{}] initialize failed: {:?}", server_id.0, error);
                    send_state(
                        &server_id,
                        &root,
                        ServerState::Failed,
                        &msg_tx,
                        wake.as_deref(),
                    );
                    let _ = msg_tx.send(Msg::Lsp(LspMsg::ServerExited {
                        server_id: server_id.clone(),
                        generation,
                    }));
                    if let Some(wake) = wake.as_deref() {
                        wake();
                    }
                    break;
                }
                let result = message.get("result").cloned();
                let parsed: Option<lsp_types::ServerCapabilities> = result
                    .and_then(|r| r.get("capabilities").cloned())
                    .and_then(|c| serde_json::from_value(c).ok());
                // Completion trigger characters ride the same parse — sent
                // even when empty so a restart that loses them (server
                // swapped underneath the same id) clears the mirror.
                let characters = parsed
                    .as_ref()
                    .map(completion_trigger_characters)
                    .unwrap_or_default();
                let (sig_trigger, sig_retrigger) = parsed
                    .as_ref()
                    .map(signature_help_triggers)
                    .unwrap_or_default();
                *capabilities.lock().unwrap() = parsed;
                let _ = outbound_tx.send(WorkerCmd::HandshakeReady);
                send_state(
                    &server_id,
                    &root,
                    ServerState::Ready,
                    &msg_tx,
                    wake.as_deref(),
                );
                let _ = msg_tx.send(Msg::Lsp(LspMsg::ServerCompletionTriggers {
                    server_id: server_id.clone(),
                    characters,
                }));
                let _ = msg_tx.send(Msg::Lsp(LspMsg::ServerSignatureTriggers {
                    server_id: server_id.clone(),
                    trigger: sig_trigger,
                    retrigger: sig_retrigger,
                }));
                if let Some(wake) = wake.as_deref() {
                    wake();
                }
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
            } else if entry.method == "textDocument/definition" {
                // `entry.abandoned` (set by a superseding request via
                // `PendingRequests::abandon`) tells the runtime to
                // consume and discard this reply rather than act on it
                // (design doc's cancellation semantics: advisory, the
                // server still replies).
                let locations = parse_definition_result(message.get("result"));
                let _ = msg_tx.send(Msg::Lsp(LspMsg::DefinitionResponseFromServer {
                    server_id: server_id.clone(),
                    root: root.clone(),
                    request_id: id,
                    locations,
                    abandoned: entry.abandoned,
                }));
                if let Some(wake) = wake.as_deref() {
                    wake();
                }
            } else if entry.method == "textDocument/hover" {
                let content = parse_hover_result(message.get("result"));
                let _ = msg_tx.send(Msg::Lsp(LspMsg::HoverResponseFromServer {
                    server_id: server_id.clone(),
                    root: root.clone(),
                    request_id: id,
                    content,
                    abandoned: entry.abandoned,
                }));
                if let Some(wake) = wake.as_deref() {
                    wake();
                }
            } else if entry.method == "textDocument/signatureHelp" {
                // `null` / malformed -> `None`, same posture as hover.
                let help = message.get("result").and_then(|r| {
                    serde_json::from_value::<lsp_types::SignatureHelp>(r.clone())
                        .ok()
                        .map(Box::new)
                });
                let _ = msg_tx.send(Msg::Lsp(LspMsg::SignatureHelpResponseFromServer {
                    server_id: server_id.clone(),
                    root: root.clone(),
                    request_id: id,
                    help,
                    abandoned: entry.abandoned,
                }));
                if let Some(wake) = wake.as_deref() {
                    wake();
                }
            } else if entry.method == "textDocument/prepareRename" {
                // `null` (cannot rename here) and malformed both -> `None`.
                let response = message.get("result").and_then(|r| {
                    serde_json::from_value::<lsp_types::PrepareRenameResponse>(r.clone()).ok()
                });
                let _ = msg_tx.send(Msg::Lsp(LspMsg::PrepareRenameResponseFromServer {
                    server_id: server_id.clone(),
                    root: root.clone(),
                    request_id: id,
                    response,
                    abandoned: entry.abandoned,
                }));
                if let Some(wake) = wake.as_deref() {
                    wake();
                }
            } else if entry.method == "textDocument/rename" {
                let edit = message.get("result").and_then(|r| {
                    serde_json::from_value::<lsp_types::WorkspaceEdit>(r.clone())
                        .ok()
                        .map(Box::new)
                });
                let _ = msg_tx.send(Msg::Lsp(LspMsg::RenameResponseFromServer {
                    server_id: server_id.clone(),
                    root: root.clone(),
                    request_id: id,
                    edit,
                    abandoned: entry.abandoned,
                }));
                if let Some(wake) = wake.as_deref() {
                    wake();
                }
            } else if entry.method == "textDocument/codeAction" {
                let actions = parse_code_action_result(message.get("result"));
                let _ = msg_tx.send(Msg::Lsp(LspMsg::CodeActionsResponseFromServer {
                    server_id: server_id.clone(),
                    root: root.clone(),
                    request_id: id,
                    actions,
                    abandoned: entry.abandoned,
                }));
                if let Some(wake) = wake.as_deref() {
                    wake();
                }
            } else if entry.method == "textDocument/formatting"
                || entry.method == "textDocument/rangeFormatting"
            {
                // `null` / malformed -> no edits, same posture as hover.
                let edits = message
                    .get("result")
                    .and_then(|r| {
                        serde_json::from_value::<Vec<lsp_types::TextEdit>>(r.clone()).ok()
                    })
                    .unwrap_or_default()
                    .into_iter()
                    .map(|e| (e.range, e.new_text))
                    .collect();
                let _ = msg_tx.send(Msg::Lsp(LspMsg::FormattingResponseFromServer {
                    server_id: server_id.clone(),
                    root: root.clone(),
                    request_id: id,
                    edits,
                    abandoned: entry.abandoned,
                }));
                if let Some(wake) = wake.as_deref() {
                    wake();
                }
            } else if entry.method == "workspace/symbol" {
                let provider = super::workspace_symbols::SymbolProvider {
                    server_id: server_id.clone(),
                    root: root.clone(),
                    generation,
                };
                let result = super::workspace_symbols::parse_response(&message, provider);
                let _ = msg_tx.send(Msg::Lsp(LspMsg::WorkspaceSymbolsResponseFromServer {
                    server_id: server_id.clone(),
                    root: root.clone(),
                    generation,
                    request_id: id,
                    result,
                    abandoned: entry.abandoned,
                }));
                if let Some(wake) = wake.as_deref() {
                    wake();
                }
            } else if entry.method == "textDocument/references" {
                let locations = parse_references_result(message.get("result"));
                let _ = msg_tx.send(Msg::Lsp(LspMsg::ReferencesResponseFromServer {
                    server_id: server_id.clone(),
                    root: root.clone(),
                    request_id: id,
                    locations,
                    abandoned: entry.abandoned,
                }));
                if let Some(wake) = wake.as_deref() {
                    wake();
                }
            } else if entry.method == "textDocument/completion" {
                let (items, is_incomplete) = parse_completion_result(message.get("result"));
                let _ = msg_tx.send(Msg::Lsp(LspMsg::CompletionResponseFromServer {
                    server_id: server_id.clone(),
                    root: root.clone(),
                    request_id: id,
                    items,
                    is_incomplete,
                    abandoned: entry.abandoned,
                }));
                if let Some(wake) = wake.as_deref() {
                    wake();
                }
            } else if entry.method == "completionItem/resolve" {
                // A null/unparseable resolve result is `None` — the
                // deferred accept proceeds with what the original item
                // carried rather than being dropped.
                let item = message.get("result").and_then(|r| {
                    serde_json::from_value::<lsp_types::CompletionItem>(r.clone())
                        .ok()
                        .map(Box::new)
                });
                let _ = msg_tx.send(Msg::Lsp(LspMsg::ResolveResponseFromServer {
                    server_id: server_id.clone(),
                    root: root.clone(),
                    request_id: id,
                    item,
                    abandoned: entry.abandoned,
                }));
                if let Some(wake) = wake.as_deref() {
                    wake();
                }
            }
            // Other request kinds are routed above; anything unrecognized
            // is resolved-and-dropped by `PendingRequests::resolve` at the
            // top of this block.
        }
    }
}

/// Parses a `textDocument/definition` response's `result` into
/// `Location`s. We advertise `linkSupport: false`, but that's advisory —
/// a real server (`laravel-lsp`, observed live) sends `LocationLink[]`
/// regardless. `LocationLink` has no `uri`/`range` fields at all
/// (`targetUri`/`targetRange` instead), so `Location`'s deserializer
/// rejects it outright rather than ignoring extra fields — falling back
/// to `LocationLink` parsing is required, not optional. `null` (no
/// definition) and a malformed result both become an empty vec — the
/// caller can't tell "no result" from "unparseable result" here, which is
/// fine: both status transient to "no definition found".
fn parse_definition_result(result: Option<&Value>) -> Vec<lsp_types::Location> {
    let Some(result) = result else {
        return Vec::new();
    };
    if result.is_null() {
        return Vec::new();
    }
    if let Ok(location) = serde_json::from_value::<lsp_types::Location>(result.clone()) {
        return vec![location];
    }
    if let Ok(locations) = serde_json::from_value::<Vec<lsp_types::Location>>(result.clone()) {
        return locations;
    }
    if let Ok(links) = serde_json::from_value::<Vec<lsp_types::LocationLink>>(result.clone()) {
        return links
            .into_iter()
            .map(|link| lsp_types::Location {
                uri: link.target_uri,
                range: link.target_selection_range,
            })
            .collect();
    }
    Vec::new()
}

/// Parses a `textDocument/references` response's `result` into
/// `Location`s. Unlike `textDocument/definition`, the spec never allows
/// `LocationLink[]` here, so this is just the `null`-or-`Vec` half of
/// `parse_definition_result`.
fn parse_references_result(result: Option<&Value>) -> Vec<lsp_types::Location> {
    let Some(result) = result else {
        return Vec::new();
    };
    serde_json::from_value::<Vec<lsp_types::Location>>(result.clone()).unwrap_or_default()
}

/// Parses a `textDocument/completion` response's `result` into its items
/// plus `isIncomplete`. The three legal shapes: `CompletionItem[]`
/// (always complete), `CompletionList { items, isIncomplete }`, and
/// `null` (no completions). A malformed result collapses to empty-and-
/// complete — the menu keeps its offline items, exactly as if the server
/// had nothing to add.
fn parse_completion_result(result: Option<&Value>) -> (Vec<lsp_types::CompletionItem>, bool) {
    let Some(result) = result else {
        return (Vec::new(), false);
    };
    if result.is_null() {
        return (Vec::new(), false);
    }
    match serde_json::from_value::<lsp_types::CompletionResponse>(result.clone()) {
        Ok(lsp_types::CompletionResponse::List(list)) => (list.items, list.is_incomplete),
        Ok(lsp_types::CompletionResponse::Array(items)) => (items, false),
        Err(_) => (Vec::new(), false),
    }
}

/// Parses a `textDocument/hover` response's `result` into plaintext
/// (lsp-integration.md: "hover markdown lightly processed to plain text").
/// `null` (no hover at this position) and a malformed result both become
/// `None` — permissive-and-collapsed, same posture as
/// `parse_definition_result`.
fn parse_hover_result(result: Option<&Value>) -> Option<StyledText> {
    let result = result?;
    if result.is_null() {
        return None;
    }
    let hover: lsp_types::Hover = serde_json::from_value(result.clone()).ok()?;
    let text = hover_contents_to_styled(&hover.contents);
    (!text.text.trim().is_empty()).then_some(text)
}

/// Collapses the three legal shapes of `HoverContents` into one plaintext
/// string. `MarkupContent` respects its own `kind` (only markdown needs
/// stripping); the deprecated `MarkedString` shapes are always markdown per
/// the pre-3.0 spec.
pub(crate) fn hover_contents_to_styled(contents: &lsp_types::HoverContents) -> StyledText {
    match contents {
        lsp_types::HoverContents::Scalar(marked) => marked_string_to_styled(marked),
        lsp_types::HoverContents::Array(items) => {
            let mut out = StyledText::default();
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str("\n\n");
                }
                out.extend(&marked_string_to_styled(item));
            }
            out
        }
        lsp_types::HoverContents::Markup(markup) => match markup.kind {
            MarkupKind::PlainText => StyledText::plain(markup.value.clone()),
            MarkupKind::Markdown => markdown_to_styled(&markup.value),
        },
    }
}

/// Flattens a `textDocument/codeAction` reply (`(Command | CodeAction)[]
/// | null`) into popup rows; a bare `Command` keeps only `command`.
/// Disabled actions are dropped. `null` / malformed -> empty.
fn parse_code_action_result(result: Option<&Value>) -> Vec<crate::model::CodeActionItem> {
    use lsp_types::CodeActionOrCommand;
    let Some(parsed) =
        result.and_then(|r| serde_json::from_value::<Vec<CodeActionOrCommand>>(r.clone()).ok())
    else {
        return Vec::new();
    };
    parsed
        .into_iter()
        .filter_map(|entry| match entry {
            CodeActionOrCommand::Command(command) => Some(crate::model::CodeActionItem {
                title: command.title.clone(),
                kind: None,
                is_preferred: false,
                edit: None,
                command: Some(command),
            }),
            CodeActionOrCommand::CodeAction(action) if action.disabled.is_none() => {
                Some(crate::model::CodeActionItem {
                    title: action.title,
                    kind: action.kind.map(|k| k.as_str().to_owned()),
                    is_preferred: action.is_preferred.unwrap_or(false),
                    edit: action.edit.map(Box::new),
                    command: action.command,
                })
            }
            CodeActionOrCommand::CodeAction(_) => None,
        })
        .collect()
}

/// Flattens a `textDocument/signatureHelp` reply into the model's
/// plaintext view: the active signature/parameter resolved per spec
/// (`SignatureInformation.activeParameter` overrides the top-level one),
/// parameter label offsets (UTF-16) converted to char offsets into the
/// label, string labels located as a substring. `None` when there are no
/// signatures.
pub fn signature_help_state(
    help: &lsp_types::SignatureHelp,
) -> Option<crate::model::SignatureHelpState> {
    if help.signatures.is_empty() {
        return None;
    }
    let active = (help.active_signature.unwrap_or(0) as usize).min(help.signatures.len() - 1);
    let signatures = help
        .signatures
        .iter()
        .map(|sig| {
            let param = sig
                .active_parameter
                .or(help.active_parameter)
                .and_then(|i| sig.parameters.as_ref()?.get(i as usize));
            crate::model::SignatureView {
                label: sig.label.clone(),
                active_parameter_range: param.and_then(|p| match &p.label {
                    lsp_types::ParameterLabel::Simple(s) => {
                        let byte = sig.label.find(s.as_str())?;
                        let start = sig.label[..byte].chars().count();
                        Some((start, start + s.chars().count()))
                    }
                    lsp_types::ParameterLabel::LabelOffsets([start, end]) => Some((
                        utf16_to_char_offset(&sig.label, *start as usize),
                        utf16_to_char_offset(&sig.label, *end as usize),
                    )),
                }),
                doc: sig
                    .documentation
                    .as_ref()
                    .map(documentation_to_styled)
                    .filter(|d| !d.text.trim().is_empty()),
                parameter_doc: param
                    .and_then(|p| p.documentation.as_ref())
                    .map(documentation_to_styled)
                    .filter(|d| !d.text.trim().is_empty()),
            }
        })
        .collect();
    Some(crate::model::SignatureHelpState { signatures, active })
}

/// The char index whose UTF-16 offset within `s` is `utf16` (clamped to
/// the end).
fn utf16_to_char_offset(s: &str, utf16: usize) -> usize {
    let mut acc = 0;
    for (i, ch) in s.chars().enumerate() {
        if acc >= utf16 {
            return i;
        }
        acc += ch.len_utf16();
    }
    s.chars().count()
}

fn documentation_to_styled(doc: &lsp_types::Documentation) -> StyledText {
    match doc {
        lsp_types::Documentation::String(s) => StyledText::plain(s.clone()),
        lsp_types::Documentation::MarkupContent(markup) => match markup.kind {
            MarkupKind::PlainText => StyledText::plain(markup.value.clone()),
            MarkupKind::Markdown => markdown_to_styled(&markup.value),
        },
    }
}

fn marked_string_to_styled(marked: &lsp_types::MarkedString) -> StyledText {
    match marked {
        lsp_types::MarkedString::String(markdown) => markdown_to_styled(markdown),
        // A bare code block by construction: one `Code` run.
        lsp_types::MarkedString::LanguageString(ls) => {
            let mut out = StyledText::default();
            out.push_styled(&ls.value, SpanStyle::Code);
            out
        }
    }
}

#[cfg(test)]
use crate::lsp::markdown::markdown_to_plain_text;
pub(crate) use crate::lsp::markdown::markdown_to_styled;

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
/// doc comment). Each publish becomes its own `Msg`; successive publishes
/// for the same URI are coalesced (newest wins) in
/// `App::process_async_messages`'s drain loop, not here.
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
        assert_eq!(item.commit_characters_support, Some(true));
        assert_eq!(item.preselect_support, Some(true));
        assert_eq!(item.label_details_support, Some(true));
        assert!(item
            .resolve_support
            .unwrap()
            .properties
            .contains(&"labelDetails".to_owned()));
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
        assert!(!supports_references(&caps));

        let refs_caps = ServerCapabilities {
            references_provider: Some(lsp_types::OneOf::Left(true)),
            ..Default::default()
        };
        assert!(supports_references(&refs_caps));
    }

    // ---- server -> client reply table ----

    #[test]
    fn workspace_configuration_replies_with_one_null_per_item() {
        let params = json!({ "items": [{}, {}, {}] });
        let reply =
            reply_for_server_request("workspace/configuration", &params, &Value::Null).unwrap();
        assert_eq!(reply, json!([null, null, null]));
    }

    #[test]
    fn register_capability_replies_null() {
        assert_eq!(
            reply_for_server_request("client/registerCapability", &Value::Null, &Value::Null)
                .unwrap(),
            Value::Null
        );
        assert_eq!(
            reply_for_server_request("client/unregisterCapability", &Value::Null, &Value::Null)
                .unwrap(),
            Value::Null
        );
    }

    #[test]
    fn apply_edit_replies_not_applied() {
        let reply =
            reply_for_server_request("workspace/applyEdit", &Value::Null, &Value::Null).unwrap();
        assert_eq!(reply, json!({ "applied": false }));
    }

    #[test]
    fn unknown_request_is_method_not_found() {
        let err = reply_for_server_request("textDocument/foldingRange", &Value::Null, &Value::Null)
            .unwrap_err();
        assert_eq!(err.code, -32601);
    }

    #[test]
    fn workspace_configuration_answers_configured_sections() {
        let settings = json!({
            "python": { "analysis": { "typeCheckingMode": "strict" } },
            "bare": "top-level-value",
        });
        let params = json!({ "items": [
            { "section": "python.analysis.typeCheckingMode" },
            { "section": "python" },
            { "section": "missing.path" },
            {},
        ] });
        let reply =
            reply_for_server_request("workspace/configuration", &params, &settings).unwrap();
        assert_eq!(
            reply,
            json!([
                "strict",
                { "analysis": { "typeCheckingMode": "strict" } },
                null,
                // No section = "everything you have".
                settings,
            ])
        );
    }

    #[test]
    fn completion_response_parses_all_three_shapes() {
        use super::parse_completion_result;
        // Bare array: always complete.
        let (items, incomplete) = parse_completion_result(Some(&json!([{ "label": "foo" }])));
        assert_eq!(items.len(), 1);
        assert!(!incomplete);
        // CompletionList carries isIncomplete.
        let (items, incomplete) = parse_completion_result(Some(&json!({
            "isIncomplete": true,
            "items": [{ "label": "a" }, { "label": "b" }],
        })));
        assert_eq!(items.len(), 2);
        assert!(incomplete);
        // Null and malformed collapse to empty-and-complete.
        assert_eq!(parse_completion_result(None), (Vec::new(), false));
        assert_eq!(
            parse_completion_result(Some(&Value::Null)),
            (Vec::new(), false)
        );
        assert_eq!(
            parse_completion_result(Some(&json!({ "nonsense": true }))),
            (Vec::new(), false)
        );
    }

    #[test]
    fn trigger_characters_extract_from_capabilities() {
        use super::completion_trigger_characters;
        // A default server block advertises no completionProvider.
        let caps: lsp_types::ServerCapabilities = Default::default();
        assert!(completion_trigger_characters(&caps).is_empty());
        let caps: lsp_types::ServerCapabilities = serde_json::from_value(json!({
            "completionProvider": { "triggerCharacters": [".", ":"] }
        }))
        .unwrap();
        assert_eq!(completion_trigger_characters(&caps), vec![".", ":"]);
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

    /// `graceful_shutdown`'s two phases (shutdown-ack wait, exit wait) are
    /// each capped by `timeout`, but must also respect `shared_deadline`
    /// — the caller's *total* teardown budget across every server being
    /// torn down. A child that never acks `shutdown` and never exits on
    /// `exit` would normally pay the full `2 * timeout`; with an
    /// already-elapsed `shared_deadline` (as if prior servers in the
    /// teardown loop already spent the whole budget), both phases must
    /// return almost immediately instead.
    #[cfg(unix)]
    #[test]
    fn graceful_shutdown_is_bounded_by_the_shared_deadline_not_the_per_phase_cap() {
        let (msg_tx, msg_rx) = std::sync::mpsc::channel();
        let dir = std::env::temp_dir();
        let mut handle = spawn_server(
            "sh",
            &["-c".to_owned(), "sleep 30".to_owned()],
            &dir,
            LspServerId::from("fake-server"),
            msg_tx,
            None,
            Value::Null,
            Value::Null,
        )
        .expect("failed to spawn fake server");

        let shared_deadline = std::time::Instant::now();
        let started = std::time::Instant::now();
        let acked_and_exited =
            handle.graceful_shutdown(&msg_rx, Duration::from_secs(2), shared_deadline);
        let elapsed = started.elapsed();

        assert!(!acked_and_exited);
        // Generous margin against scheduling jitter under a loaded test
        // run — the point is distinguishing this from the *unbounded*
        // per-phase cap (2s ack-wait + 2s exit-wait = 4s minimum without
        // the shared deadline), not pinning an exact figure.
        assert!(
            elapsed < Duration::from_secs(3),
            "an already-elapsed shared deadline must short-circuit both phases instead of \
             paying the full per-phase cap (2s ack-wait + 2s exit-wait), took {elapsed:?}"
        );
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

    // ---- textDocument/definition response parsing ----

    fn location_json(path: &str) -> Value {
        json!({
            "uri": format!("file://{path}"),
            "range": {
                "start": { "line": 4, "character": 2 },
                "end": { "line": 4, "character": 8 },
            },
        })
    }

    #[test]
    fn definition_result_null_is_no_locations() {
        assert!(parse_definition_result(Some(&Value::Null)).is_empty());
        assert!(parse_definition_result(None).is_empty());
    }

    #[test]
    fn definition_result_accepts_a_single_location() {
        let result = location_json("/tmp/foo.rs");
        let locations = parse_definition_result(Some(&result));
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].uri.as_str(), "file:///tmp/foo.rs");
    }

    #[test]
    fn definition_result_accepts_a_location_array_multiple_locations_first_wins_at_the_caller() {
        let result = json!([location_json("/tmp/a.rs"), location_json("/tmp/b.rs")]);
        let locations = parse_definition_result(Some(&result));
        assert_eq!(locations.len(), 2);
        assert_eq!(locations[0].uri.as_str(), "file:///tmp/a.rs");
    }

    #[test]
    fn definition_result_malformed_is_no_locations_not_a_panic() {
        let result = json!({ "not": "a location" });
        assert!(parse_definition_result(Some(&result)).is_empty());
    }

    // ---- textDocument/references response parsing ----

    #[test]
    fn references_result_null_is_no_locations() {
        assert!(parse_references_result(Some(&Value::Null)).is_empty());
        assert!(parse_references_result(None).is_empty());
    }

    #[test]
    fn references_result_accepts_a_location_array() {
        let result = json!([location_json("/tmp/a.rs"), location_json("/tmp/b.rs")]);
        let locations = parse_references_result(Some(&result));
        assert_eq!(locations.len(), 2);
        assert_eq!(locations[1].uri.as_str(), "file:///tmp/b.rs");
    }

    #[test]
    fn references_result_malformed_is_no_locations_not_a_panic() {
        let result = json!({ "not": "a location" });
        assert!(parse_references_result(Some(&result)).is_empty());
    }

    #[test]
    fn definition_result_accepts_location_link_array_despite_advertised_link_support_false() {
        // laravel-lsp (observed live) ignores our `linkSupport: false` and
        // replies with `LocationLink[]` regardless — a non-conforming but
        // real server, not a hypothetical.
        let result = json!([{
            "originSelectionRange": {
                "start": { "line": 5, "character": 17 },
                "end": { "line": 5, "character": 24 },
            },
            "targetUri": "file:///tmp/welcome.blade.php",
            "targetRange": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 10, "character": 0 },
            },
            "targetSelectionRange": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 1 },
            },
        }]);
        let locations = parse_definition_result(Some(&result));
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].uri.as_str(), "file:///tmp/welcome.blade.php");
        assert_eq!(locations[0].range.start.line, 0);
        assert_eq!(locations[0].range.end.character, 1);
    }

    // ---- textDocument/hover response parsing ----

    #[test]
    fn hover_result_null_is_none() {
        assert!(parse_hover_result(Some(&Value::Null)).is_none());
        assert!(parse_hover_result(None).is_none());
    }

    #[test]
    fn hover_result_malformed_is_none_not_a_panic() {
        let result = json!({ "not": "a hover" });
        assert!(parse_hover_result(Some(&result)).is_none());
    }

    #[test]
    fn hover_result_plaintext_markup_passes_through_unchanged() {
        let result = json!({ "contents": { "kind": "plaintext", "value": "fn foo() -> i32" } });
        assert_eq!(
            parse_hover_result(Some(&result))
                .as_ref()
                .map(|t| t.text.as_str()),
            Some("fn foo() -> i32")
        );
    }

    #[test]
    fn hover_result_markdown_markup_strips_fences_and_emphasis() {
        let result = json!({ "contents": {
            "kind": "markdown",
            "value": "```rust\nfn foo() -> i32\n```\n**bold** and *italic* and `code`",
        }});
        let text = parse_hover_result(Some(&result)).unwrap();
        assert_eq!(text.text, "fn foo() -> i32\nbold and italic and code");
    }

    #[test]
    fn hover_result_scalar_marked_string_is_treated_as_markdown() {
        let result = json!({ "contents": "**bold**" });
        assert_eq!(
            parse_hover_result(Some(&result))
                .as_ref()
                .map(|t| t.text.as_str()),
            Some("bold")
        );
    }

    #[test]
    fn hover_result_array_of_marked_strings_joins_with_blank_line() {
        let result = json!({ "contents": ["one", { "language": "rust", "value": "two()" }] });
        assert_eq!(
            parse_hover_result(Some(&result))
                .as_ref()
                .map(|t| t.text.as_str()),
            Some("one\n\ntwo()")
        );
    }

    #[test]
    fn hover_result_blank_content_is_none() {
        let result = json!({ "contents": { "kind": "plaintext", "value": "   " } });
        assert!(parse_hover_result(Some(&result)).is_none());
    }

    #[test]
    fn markdown_to_plain_text_strips_headings() {
        assert_eq!(markdown_to_plain_text("## Signature"), "Signature");
        // Setext headings differ from thematic breaks, which need a blank
        // line after a paragraph when written with dashes.
        assert_eq!(markdown_to_plain_text("a\n---\nb"), "a\nb");
        assert_eq!(markdown_to_plain_text("a\n\n---\nb"), "a\n\nb");
        assert_eq!(markdown_to_plain_text("a\n* * *\nb"), "a\n\nb");
        // Inline links keep the text, drop the url; bare refs survive.
        assert_eq!(
            markdown_to_plain_text("see [docs](https://example.com) and [valid]"),
            "see docs and [valid]"
        );
    }

    #[test]
    fn markdown_to_plain_text_preserves_code_fence_identifiers() {
        // Code-fence contents must pass through untouched: `*`, `_`, and
        // backtick are legal in identifiers/operators/generics, not markdown
        // emphasis, once inside a fence.
        let markdown = "```rust\npub fn read_to_string(path: &Path)\n__init__\n5 * 3\n```";
        assert_eq!(
            markdown_to_plain_text(markdown),
            "pub fn read_to_string(path: &Path)\n__init__\n5 * 3"
        );
    }

    // ---- textDocument/signatureHelp flattening ----

    /// Label offsets are UTF-16; the model wants char offsets. `😀` is one
    /// char but two UTF-16 units, so offsets after it shift by one.
    #[test]
    fn signature_help_state_converts_utf16_label_offsets_to_char_offsets() {
        let help: lsp_types::SignatureHelp = serde_json::from_value(json!({
            "signatures": [{
                "label": "f(😀: A, b: B)",
                "parameters": [
                    { "label": [2, 6] },
                    { "label": [9, 13], "documentation": { "kind": "markdown", "value": "**second**" } }
                ]
            }],
            "activeSignature": 0,
            "activeParameter": 1
        }))
        .unwrap();
        let state = signature_help_state(&help).unwrap();
        let sig = &state.signatures[0];
        assert_eq!(sig.active_parameter_range, Some((8, 12)));
        assert_eq!(
            &sig.label.chars().skip(8).take(4).collect::<String>(),
            "b: B"
        );
        assert_eq!(
            sig.parameter_doc.as_ref().map(|t| t.text.as_str()),
            Some("second")
        );
    }

    #[test]
    fn signature_help_state_locates_a_string_parameter_label_in_the_signature() {
        let help: lsp_types::SignatureHelp = serde_json::from_value(json!({
            "signatures": [{ "label": "f(a: A, b: B)", "parameters": [{ "label": "b: B" }] }],
            "activeParameter": 0
        }))
        .unwrap();
        let state = signature_help_state(&help).unwrap();
        assert_eq!(state.signatures[0].active_parameter_range, Some((8, 12)));
    }
}
