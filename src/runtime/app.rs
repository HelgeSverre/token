use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str::FromStr;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::Result;
use softbuffer::Context;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
#[cfg(debug_assertions)]
use winit::keyboard::{Key, NamedKey};
use winit::keyboard::{KeyCode, PhysicalKey};
#[cfg(not(target_os = "macos"))]
use winit::window::Icon;
use winit::window::{CursorIcon, Window};

use token::cli::{StartupConfig, StartupMode};
use token::commands::{Cmd, Damage, DamageArea, ResolvePurpose};
use token::fs_watcher::{FileSystemEvent, FileSystemWatcher};
use token::keymap::{
    keystroke_from_winit, load_default_keymap, Command, KeyAction, KeyContext, Keymap,
};
use token::lsp::{self, client::ServerHandle, LspServerId, ServerState};
use token::messages::{
    AppMsg, CompletionMsg, DefinitionOutcome, EditorMsg, HoverOutcome, ImageMsg, LayoutMsg, LspMsg,
    ModalMsg, Msg, ReferencesOutcome, SyntaxMsg, UiMsg, WorkspaceMsg,
};
use token::model::editor::Position;
use token::model::{AppModel, JumpEntry};
use token::panel::DockPosition;
use token::syntax::{LanguageId, ParserState};
use token::update::update;

use super::input::{
    handle_cursor_overlay_key, handle_key, is_outline_dock_focused, is_problems_dock_focused,
    is_terminal_dock_focused, KeyModifiers, OptionKeyGesture,
};
use super::lsp_slot::{FeatureSlot, PendingRequest, RequestKey};
use super::mouse::{
    end_tab_drag, handle_mouse_press, handle_mouse_wheel, make_mouse_event, update_hover_target,
    update_tab_drag, ClickTracker, DragState,
};
use super::webview::WebviewManager;
use token::view::{Renderer, RendererPreparation};

use crate::automation::{self, AutomationEnvelope, AutomationRequest, AutomationResponse};
use token::perf::{PerfStage, PerfStats};

use winit::keyboard::ModifiersState;

/// Request sent to syntax worker thread
struct SyntaxParseRequest {
    document_id: token::model::editor_area::DocumentId,
    revision: u64,
    source: Arc<str>,
    language: LanguageId,
    snapshot_ms: f64,
    queued_at: Instant,
    extract_outline: bool,
}

enum SyntaxWorkerRequest {
    Parse(SyntaxParseRequest),
    ClearDocument(token::model::editor_area::DocumentId),
}

type TerminalSpawnReceiver = Receiver<Result<token::terminal::TerminalSpawnResult, String>>;

const POST_FIRST_FRAME_STARTUP_DELAY: Duration = Duration::from_millis(50);

fn should_skip_non_global_keymap(
    model: &AppModel,
    option_double_tapped: bool,
    alt_pressed: bool,
) -> bool {
    let sidebar_focused = matches!(
        model.ui.focus,
        token::model::FocusTarget::Dock(token::panel::DockPosition::Left)
    );
    model.ui.has_modal()
        || (option_double_tapped && alt_pressed)
        || sidebar_focused
        || is_outline_dock_focused(model)
        || is_problems_dock_focused(model)
        || is_terminal_dock_focused(model)
        || model.is_csv_editing()
}

struct PreparedApp {
    model: AppModel,
    keymap: Keymap,
    workspace_root: Option<PathBuf>,
}

/// Application state prepared in parallel with the platform event loop.
pub struct AppPreparation {
    handle: JoinHandle<PreparedApp>,
}

impl AppPreparation {
    pub fn start(
        window_width: u32,
        window_height: u32,
        startup_config: StartupConfig,
    ) -> std::io::Result<Self> {
        let handle = std::thread::Builder::new()
            .name("token-app-loader".to_owned())
            .spawn(move || prepare_app(window_width, window_height, startup_config))?;
        Ok(Self { handle })
    }

    fn finish(self) -> Option<PreparedApp> {
        match self.handle.join() {
            Ok(prepared) => Some(prepared),
            Err(_) => {
                tracing::warn!("Application preparation thread panicked; retrying synchronously");
                None
            }
        }
    }
}

fn prepare_app(
    window_width: u32,
    window_height: u32,
    startup_config: StartupConfig,
) -> PreparedApp {
    let keymap = Keymap::with_bindings(load_default_keymap());

    let (demo_mode, file_paths, workspace_root) = match startup_config.mode {
        StartupMode::Demo => (true, Vec::new(), None),
        StartupMode::Empty => (false, Vec::new(), None),
        StartupMode::SingleFile(path) => (false, vec![path], None),
        StartupMode::MultipleFiles(paths) => (false, paths, None),
        StartupMode::Workspace {
            root,
            initial_files,
        } => (false, initial_files, Some(root)),
    };

    let mut model = AppModel::new(window_width, window_height, 1.0, file_paths);
    if demo_mode {
        let document_id = model.document().id;
        *model.document_mut() = token::model::Document::with_text(DEMO_DOCUMENT);
        model.document_mut().id = document_id;
        model.document_mut().untitled_name = Some("Automation Demo.rs".to_owned());
        model.document_mut().language = LanguageId::Rust;
    }

    let cli_paths: Vec<_> = model
        .editor_area
        .documents
        .values()
        .filter_map(|doc| doc.file_path.clone())
        .collect();
    for path in cli_paths {
        model.record_file_opened(path);
    }

    if let Some(root) = &workspace_root {
        model.open_workspace(root.clone());
    }

    if let Some((line, column)) = startup_config.initial_position {
        let editor = model.editor_mut();
        editor.cursors[0].line = line;
        editor.cursors[0].column = column;
        editor.selections[0].anchor = Position::new(line, column);
        editor.selections[0].head = Position::new(line, column);
        model.ensure_cursor_visible();
    }

    PreparedApp {
        model,
        keymap,
        workspace_root,
    }
}

pub struct App {
    model: AppModel,
    keymap: Keymap,
    renderer: Option<Renderer>,
    renderer_preparation: Option<RendererPreparation>,
    window: Option<Rc<Window>>,
    context: Option<Context<Rc<Window>>>,
    last_tick: Instant,
    modifiers: ModifiersState,
    mouse_position: Option<(f64, f64)>,
    /// Mouse-dwell hover tracking (Zed-style `hover_on_mouse`): the pixel
    /// position + timestamp the pointer last settled at, cleared on any
    /// significant move, button press, or while a modal/cursor-overlay is
    /// open. `about_to_wait` fires `LspMsg::ShowHoverAt` once this has aged
    /// past `config.hover_delay_ms` — see `check_hover_dwell`.
    hover_dwell: Option<(f64, f64, Instant)>,
    /// Carries sub-line trackpad scroll remainders between wheel events.
    scroll_accumulator: ScrollAccumulator,
    option_gesture: OptionKeyGesture,
    drag: DragState,
    msg_tx: Sender<Msg>,
    msg_rx: Receiver<Msg>,
    perf: PerfStats,
    /// Channel to send parse requests to syntax worker
    syntax_tx: Sender<SyntaxWorkerRequest>,
    /// File system watcher for workspace directory (if workspace is open)
    fs_watcher: Option<FileSystemWatcher>,
    /// Workspace watcher initialization is deferred until after first paint.
    pending_fs_watcher_root: Option<PathBuf>,
    /// Deadline for nonessential startup work scheduled after first paint.
    deferred_startup_at: Option<Instant>,
    deferred_startup_complete: bool,
    /// Pending damage for the next render (accumulated from commands)
    pending_damage: Damage,
    /// Flag to request application exit (set by Cmd::Quit)
    should_quit: bool,
    /// Webview manager for markdown preview
    webview_manager: WebviewManager,
    /// Click tracker for unified mouse event handling
    click_tracker: ClickTracker,
    /// Syntax highlight debounce deadlines: document_id → (deadline, revision)
    syntax_deadlines: HashMap<token::model::editor_area::DocumentId, (Instant, u64)>,
    /// File paths queued for background loading after startup
    /// Receiver for background PTY spawn completion. Spawned asynchronously
    /// because `portable_pty` startup can block on shell initialization.
    terminal_spawn_rx: Option<(usize, TerminalSpawnReceiver)>,
    automation_rx: Receiver<AutomationEnvelope>,
    /// A sender into `automation_rx`: the macOS open-file hook feeds it,
    /// and tests push requests through the exact same
    /// `process_automation_requests` path the socket/MCP server feeds in
    /// production, without standing up a real socket.
    automation_tx: Sender<AutomationEnvelope>,
    automation_profile: Option<AutomationProfile>,
    /// `--wait` handoffs still waiting for their documents to close.
    document_waiters: Vec<DocumentWaiter>,
    /// Inline-suggestion debounces: document → (deadline, revision,
    /// explicit). Re-arming replaces the entry (autocomplete.md Phase 2).
    inline_deadlines: HashMap<token::model::editor_area::DocumentId, (Instant, u64, bool)>,
    /// Requests for the completion worker thread.
    inline_tx: Sender<token::completion::inline::InlineRequest>,
    /// When this window last gained focus (process start until then);
    /// automation clients pick the most recently focused instance.
    focused_at: std::time::SystemTime,
    syntax_scheduled: HashMap<(token::model::editor_area::DocumentId, u64), Instant>,
    syntax_present_pending: Vec<SyntaxPresentationPending>,
    automation_syntax_profile: Option<AutomationSyntaxProfile>,
    latest_syntax_performance: Option<crate::automation::SyntaxPerfSnapshot>,
    lsp: LspManager,
    /// Wakes the event loop from an LSP worker thread; `None` in tests
    /// that construct `App` without a real event loop (matches
    /// `automation_proxy`'s optionality).
    lsp_wake: Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
    /// Debounced `didChange` deadlines (max-wait capped), keyed by
    /// document — mirrors `syntax_deadlines`'s shape.
    lsp_change_deadlines: lsp::sync::DidChangeDeadlines<token::model::editor_area::DocumentId>,
}

struct AutomationProfile {
    remaining_frames: usize,
    response_tx: mpsc::SyncSender<AutomationResponse>,
}

/// One `OpenPaths { wait: true }` request: answered once every document
/// in `remaining` has been released (closed in every group), or on exit.
/// `exit_only` waiters (no files, or none resolvable) answer on exit only.
struct DocumentWaiter {
    remaining: HashSet<token::model::editor_area::DocumentId>,
    exit_only: bool,
    response_tx: mpsc::SyncSender<AutomationResponse>,
}

struct AutomationSyntaxProfile {
    document_id: token::model::editor_area::DocumentId,
    revision: u64,
    response_tx: mpsc::SyncSender<AutomationResponse>,
}

struct SyntaxPresentationPending {
    snapshot: crate::automation::SyntaxPerfSnapshot,
    started_at: Instant,
    response_tx: Option<mpsc::SyncSender<AutomationResponse>>,
}

/// Non-workspace roots (stdlib, `~/.cargo/registry`, ...) are capped per
/// session so opening files scattered across the filesystem doesn't
/// spawn an unbounded number of servers (see design doc's Root
/// resolution).
const MAX_DETACHED_ROOTS: usize = 4;

/// Crash-restart attempts before a server gives up and reports `Failed`.
const MAX_RESTART_ATTEMPTS: u8 = 3;

/// Exponential crash-restart backoff: `RESTART_BACKOFF_BASE_MS * 2^(attempt-1)`,
/// capped at `RESTART_BACKOFF_MAX_MS`.
const RESTART_BACKOFF_BASE_MS: u64 = 200;
const RESTART_BACKOFF_MAX_MS: u64 = 5_000;

/// UI-level abandonment timeout for `textDocument/definition` (design
/// doc's "~30 s for definition/hover (cold rust-analyzer legitimately
/// exceeds 10 s)"). Not a server-side cancellation guarantee — the
/// request is still marked abandoned and `$/cancelRequest` sent, but a
/// late reply is simply consumed and discarded (same as supersession).
const DEFINITION_TIMEOUT: Duration = Duration::from_secs(30);

/// UI-level abandonment timeout for `textDocument/hover` — same class as
/// `DEFINITION_TIMEOUT` per the design doc.
const HOVER_TIMEOUT: Duration = Duration::from_secs(30);

/// UI-level abandonment timeout for `textDocument/references` — same
/// class as `DEFINITION_TIMEOUT`/`HOVER_TIMEOUT`.
const REFERENCES_TIMEOUT: Duration = Duration::from_secs(30);

/// UI-level abandonment timeout for `textDocument/completion`. Shorter
/// than definition/hover's 30 s: a completion list arriving tens of
/// seconds after typing is worse than none, and the menu degrades
/// silently to words/snippets either way (completion never flashes a
/// status transient).
const COMPLETION_TIMEOUT: Duration = Duration::from_secs(10);
/// Signature help is typing-driven like completion — same silent, short
/// abandonment window.
const SIGNATURE_HELP_TIMEOUT: Duration = Duration::from_secs(10);
/// Rename is explicit and may touch the whole workspace — as patient as
/// references.
const RENAME_TIMEOUT: Duration = Duration::from_secs(30);
/// Code actions are user-invoked with a popup waiting on them — a
/// definition-class wait, but the "server did not answer" status makes a
/// shorter window acceptable.
const CODE_ACTIONS_TIMEOUT: Duration = Duration::from_secs(10);
/// Explicit Format Document/Selection waits this long for the server.
const FORMATTING_TIMEOUT: Duration = Duration::from_secs(10);
/// A `format_on_save` request must not hold the save hostage — past this
/// the file is written unformatted.
const FORMAT_ON_SAVE_TIMEOUT: Duration = Duration::from_secs(2);

/// UI-level abandonment timeout for a deferred accept's
/// `completionItem/resolve` round trip. On expiry the accept applies with
/// whatever the original item carried (auto-import edits are lost, but
/// Enter still works).
const RESOLVE_TIMEOUT: Duration = Duration::from_secs(3);

/// Per-document debounce between "the menu selection changed" and the
/// docs-purpose `completionItem/resolve` going out — arrowing through a
/// list coalesces into one request for the row the user lands on.
const RESOLVE_DEBOUNCE: Duration = Duration::from_millis(150);

/// Per-document debounce between "the completion query changed" and the
/// `textDocument/completion` request actually going out — typing-driven,
/// unlike definition/hover's single-shot requests. Coalesces a burst of
/// keystrokes into one request; flush-before-request still guarantees the
/// server sees current text when it fires.
const COMPLETION_DEBOUNCE: Duration = Duration::from_millis(120);

/// Show Usages caps the popup at this many locations — a status transient
/// reports the overflow count rather than rendering an unbounded list.
const MAX_REFERENCE_LOCATIONS: usize = 200;

/// Pointer movement (px) past which mouse-dwell hover tracking (`hover_dwell`)
/// resets — small jitter within this radius doesn't restart the delay.
const HOVER_DWELL_MOVE_THRESHOLD_PX: f64 = 3.0;

/// Owns every running language server's process handle. Authoritative —
/// `AppModel.lsp` (`LspUiState`) is a render-only mirror driven by
/// `Msg::Lsp(ServerStateChanged)`; this stays out of the model per the
/// design doc's Process Model (non-`Debug`/`Clone` handles must never
/// reach `AppModel`, which the automation layer snapshots wholesale).
struct LspManager {
    servers: HashMap<(LspServerId, PathBuf), ServerHandle>,
    detached_roots: Vec<PathBuf>,
    /// Crash count per `(server_id, root)` — keyed the same as `servers`
    /// so a crash loop at one root never counts against, or exhausts,
    /// another root running the same server binary. Cleared when that
    /// root reaches `Ready` (see `process_async_messages`'s `lsp_ready`
    /// handling) and on a manual `Cmd::LspRestartServer`.
    restart_attempts: HashMap<(LspServerId, PathBuf), u8>,
    /// Roots a server gave up on (`ServerState::Failed`), retained after
    /// its handle is removed so `Cmd::LspRestartServer` — which only
    /// knows the server id — has somewhere to respawn. Cleared once a
    /// restart is attempted for that id.
    failed_roots: HashMap<LspServerId, Vec<PathBuf>>,
    /// Crash-restart respawns scheduled after an exponential backoff
    /// delay (see `MAX_RESTART_ATTEMPTS`'s doc and
    /// `handle_lsp_server_exited`), fired by `check_lsp_restart_deadlines`.
    /// A manual `Cmd::LspRestartServer` bypasses this and respawns
    /// immediately.
    restart_deadlines: HashMap<(LspServerId, PathBuf), Instant>,
    /// Documents currently `didOpen`'d against a running server —
    /// authoritative record of "is this doc LSP-synced, and against
    /// which (server, root)". Populated by `lsp_open_document`, removed
    /// by `lsp_close_document`; survives a crash so a restart knows
    /// which documents to re-`didOpen`.
    open_documents: HashMap<token::model::editor_area::DocumentId, OpenDocState>,
    /// Set for the duration of `Cmd::Quit`'s teardown — suppresses the
    /// crash-restarter from reacting to the `ServerExited` a deliberate
    /// kill produces (design doc's `ShuttingDown` state).
    shutting_down: bool,
    /// `(server_id, root)` pairs whose next `Ready` should re-`didOpen`
    /// every tracked document — set by any respawn (crash-restart or
    /// manual `RestartLanguageServer`), since a fresh process has no
    /// memory of what the previous one had open.
    resync_pending: std::collections::HashSet<(LspServerId, PathBuf)>,
    /// Authoritative diagnostics store (lsp-integration.md Phase 2),
    /// keyed by canonical URI. Full replacement per publish; retains
    /// entries for URIs with no open document (rust-analyzer publishes
    /// workspace-wide from `cargo check`) so a later `didOpen` can pull
    /// them and a future Problems panel gets them for free.
    /// `Document.diagnostics` (model-side) is only ever a projection of
    /// this for currently-open documents.
    diagnostics: HashMap<lsp_types::Uri, Vec<lsp_types::Diagnostic>>,
    /// The last `version` seen per URI (when the server sends one) —
    /// used only to drop out-of-order publishes, never as a
    /// `Document.revision` equality guard (see the design doc's
    /// diagnostics exception).
    diagnostics_versions: HashMap<lsp_types::Uri, i64>,
    /// In-flight `textDocument/definition` requests — see `FeatureSlot`'s
    /// doc comment; drives `LspMsg::DefinitionResolved` via
    /// `process_async_messages`'s interception pass and is swept for
    /// abandonment by `check_lsp_definition_deadlines`.
    definition: FeatureSlot<PendingDefinition>,
    /// In-flight `textDocument/hover` requests, mirroring `definition`;
    /// swept for abandonment by `check_lsp_hover_deadlines`.
    hover: FeatureSlot<PendingHover>,
    /// In-flight `textDocument/references` (Show Usages / Find Usages)
    /// requests, mirroring `hover`; swept for abandonment by
    /// `check_lsp_references_deadlines`.
    references: FeatureSlot<PendingReferences>,
    /// In-flight `textDocument/codeAction` requests, mirroring `references`.
    code_actions: FeatureSlot<PendingCodeActions>,
    /// In-flight `textDocument/completion` requests, mirroring
    /// `references`; swept for abandonment by
    /// `check_lsp_completion_deadlines` (silently — completion never
    /// flashes a status transient).
    completion: FeatureSlot<PendingCompletion>,
    /// In-flight `textDocument/signatureHelp` requests, mirroring
    /// `completion` (silent sweep).
    signature_help: FeatureSlot<PendingSignatureHelp>,
    /// In-flight `textDocument/prepareRename` requests.
    prepare_rename: FeatureSlot<PendingPrepareRename>,
    /// In-flight `textDocument/rename` requests.
    rename: FeatureSlot<PendingRename>,
    /// In-flight `textDocument/formatting` / `rangeFormatting` requests.
    formatting: FeatureSlot<PendingFormatting>,
    /// In-flight `completionItem/resolve` requests; swept by
    /// `check_lsp_resolve_deadlines`, which emits an empty
    /// `CompletionItemResolved` for `Accept`-purpose ones so the blocked
    /// accept applies anyway (`Docs`-purpose ones just drop).
    resolve: FeatureSlot<PendingResolve>,
    /// Completion requests waiting out `COMPLETION_DEBOUNCE`, keyed by
    /// document. Fired by `check_lsp_completion_debounces`.
    completion_debounces: HashMap<token::model::editor_area::DocumentId, ScheduledCompletion>,
    /// Docs-purpose resolves waiting out `RESOLVE_DEBOUNCE`, keyed by
    /// document. Fired by `check_lsp_resolve_debounces`.
    resolve_debounces: HashMap<token::model::editor_area::DocumentId, ScheduledResolve>,
    /// `(server_id, root)` pairs whose spawn attempt already reported
    /// `ServerState::Missing` — `ensure_lsp_server` skips these outright
    /// (design doc's "one-time transient, no error spam"). Without this,
    /// every matching file-open re-attempts the spawn: two transients
    /// per open, and (worse) each attempt consumed a `detached_roots`
    /// slot *before* spawning, so a handful of opens for a missing
    /// server permanently exhausted `MAX_DETACHED_ROOTS`. Cleared by
    /// `Cmd::LspRestartServer` (the user's explicit "try again").
    missing_servers: std::collections::HashSet<(LspServerId, PathBuf)>,
}

/// What `LspManager` needs to turn a `textDocument/definition` response
/// into `LspMsg::DefinitionResolved` — captured at request time since the
/// worker thread that receives the response has no access to model state.
struct PendingDefinition {
    document_id: token::model::editor_area::DocumentId,
    revision: u64,
    origin: JumpEntry,
    /// The server that this request was sent to — carried through to
    /// `DefinitionResolved` so a location outside every root can be
    /// routed back to the *resolving* server instead of the generic
    /// open path re-deriving (and possibly spawning) its own root; see
    /// `LspUiState::route_hint`.
    server_id: LspServerId,
    root: PathBuf,
}

impl PendingRequest for PendingDefinition {
    fn document_id(&self) -> token::model::editor_area::DocumentId {
        self.document_id
    }
}

/// What `LspManager` needs to turn a `textDocument/hover` response into
/// `LspMsg::HoverResolved` — mirrors `PendingDefinition`.
struct PendingHover {
    document_id: token::model::editor_area::DocumentId,
    revision: u64,
    cursor: token::model::editor::Position,
}

impl PendingRequest for PendingHover {
    fn document_id(&self) -> token::model::editor_area::DocumentId {
        self.document_id
    }
}

/// What `LspManager` needs to turn a `textDocument/signatureHelp`
/// response into `LspMsg::SignatureHelpResolved` — same shape as
/// `PendingHover`.
struct PendingSignatureHelp {
    document_id: token::model::editor_area::DocumentId,
    revision: u64,
    cursor: token::model::editor::Position,
}

impl PendingRequest for PendingSignatureHelp {
    fn document_id(&self) -> token::model::editor_area::DocumentId {
        self.document_id
    }
}

/// What `LspManager` needs to turn a `textDocument/prepareRename`
/// response into `LspMsg::PrepareRenameResolved`. `fallback` is the word
/// under the caret, used for a `defaultBehavior` reply.
struct PendingPrepareRename {
    document_id: token::model::editor_area::DocumentId,
    revision: u64,
    cursor: token::model::editor::Position,
    fallback: String,
}

impl PendingRequest for PendingPrepareRename {
    fn document_id(&self) -> token::model::editor_area::DocumentId {
        self.document_id
    }
}

/// What `LspManager` needs to turn a formatting response into
/// `LspMsg::FormattingResolved`. `then_save` rides along so the gate /
/// timeout fallbacks still perform the `format_on_save` save.
struct PendingFormatting {
    document_id: token::model::editor_area::DocumentId,
    revision: u64,
    then_save: bool,
}

impl PendingRequest for PendingFormatting {
    fn document_id(&self) -> token::model::editor_area::DocumentId {
        self.document_id
    }
}

/// What `LspManager` needs to turn a `textDocument/rename` response into
/// `LspMsg::RenameResolved`.
struct PendingRename {
    document_id: token::model::editor_area::DocumentId,
    revision: u64,
}

impl PendingRequest for PendingRename {
    fn document_id(&self) -> token::model::editor_area::DocumentId {
        self.document_id
    }
}

/// Status transient for a rename request that never produced an edit.
fn rename_status_msg(text: &str) -> Msg {
    Msg::Ui(token::messages::UiMsg::SetTransientMessage {
        text: text.to_owned(),
        duration_ms: 3000,
    })
}

impl PendingFormatting {
    fn unavailable(self) -> Option<Msg> {
        Some(Msg::Lsp(LspMsg::FormattingResolved {
            document_id: self.document_id,
            revision: self.revision,
            edits: None,
            then_save: self.then_save,
        }))
    }
}

/// What `LspManager` needs to turn a `textDocument/references` response
/// into `LspMsg::ReferencesResolved` — same shape as `PendingHover`.
struct PendingReferences {
    document_id: token::model::editor_area::DocumentId,
    revision: u64,
    cursor: token::model::editor::Position,
}

impl PendingRequest for PendingReferences {
    fn document_id(&self) -> token::model::editor_area::DocumentId {
        self.document_id
    }
}

/// What `LspManager` needs to turn a `textDocument/codeAction` response
/// into `LspMsg::CodeActionsResolved` — same shape as `PendingReferences`.
struct PendingCodeActions {
    document_id: token::model::editor_area::DocumentId,
    revision: u64,
    cursor: token::model::editor::Position,
}

impl PendingRequest for PendingCodeActions {
    fn document_id(&self) -> token::model::editor_area::DocumentId {
        self.document_id
    }
}

impl PendingCodeActions {
    fn resolved(
        self,
        actions: Vec<token::model::CodeActionItem>,
        outcome: ReferencesOutcome,
    ) -> Msg {
        Msg::Lsp(LspMsg::CodeActionsResolved {
            document_id: self.document_id,
            revision: self.revision,
            cursor: self.cursor,
            actions,
            outcome,
        })
    }
}

/// What `LspManager` needs to turn a `textDocument/completion` response
/// into `LspMsg::CompletionResolved` — mirrors `PendingHover` minus the
/// cursor (the menu's own query/revision guards do the rest update-side).
struct PendingCompletion {
    document_id: token::model::editor_area::DocumentId,
    revision: u64,
}

impl PendingRequest for PendingCompletion {
    fn document_id(&self) -> token::model::editor_area::DocumentId {
        self.document_id
    }
}

/// What `LspManager` needs to turn a `completionItem/resolve` response
/// into `LspMsg::CompletionItemResolved` — the deferred accept's context.
/// `selected` echoes the menu selection the resolve was issued for so a
/// resolution whose selection has since moved is dropped update-side.
struct PendingResolve {
    document_id: token::model::editor_area::DocumentId,
    revision: u64,
    selected: usize,
    purpose: ResolvePurpose,
}

impl PendingRequest for PendingResolve {
    fn document_id(&self) -> token::model::editor_area::DocumentId {
        self.document_id
    }
}

/// A completion request waiting out `COMPLETION_DEBOUNCE`, armed by
/// `Cmd::LspScheduleCompletion`. Re-arming the same document resets the
/// deadline (a keystroke burst coalesces into one request); dismissal
/// drops it (`Cmd::LspCancelCompletion`).
struct ScheduledCompletion {
    position: lsp_types::Position,
    revision: u64,
    trigger_character: Option<String>,
    deadline: Instant,
}

/// A docs-purpose resolve waiting out `RESOLVE_DEBOUNCE`, armed by
/// `Cmd::LspScheduleResolve` — same lifecycle as `ScheduledCompletion`.
struct ScheduledResolve {
    revision: u64,
    server_id: LspServerId,
    root: PathBuf,
    raw_item: serde_json::Value,
    selected: usize,
    deadline: Instant,
}

/// Why `send_lsp_feature_request` couldn't put a request on the wire — the
/// caller maps this to its own outcome enum. `NoServer` and `Unsupported`
/// happen to map to the same outcome (`NotSupported`) for both definition
/// and hover today, but stay distinct here since a future feature might
/// tell them apart.
enum FeatureGateError {
    NoServer,
    NotReady,
    Unsupported,
}

/// Static description of one position-based LSP feature (definition,
/// hover, references, completion): the method, the capability gate, and
/// which slot owns its bookkeeping. Implemented via the `lsp_feature!`
/// macro below.
trait LspFeature: PendingRequest + Sized {
    const METHOD: &'static str;

    /// The slot this feature's in-flight requests live in.
    fn slot(lm: &mut LspManager) -> &mut FeatureSlot<Self>;

    fn supports(caps: &lsp_types::ServerCapabilities) -> bool;
}

/// The failure/success-adjacent messaging policies that differ per
/// feature, kept out of `LspFeature` so the mechanical parts stay
/// macro-generated. Both are functions on the *consumed* pending payload
/// rather than data: definition needs its captured jump-history `origin`
/// for the failure message, hover needs the cursor, and completion is
/// silent on every failure (`None`) — a "not supported" transient
/// flashing on each keystroke in an unsynced buffer would be noise.
///
/// `completionItem/resolve` is intentionally NOT covered by either
/// trait: it isn't position-based (no `send_lsp_feature_request` gating)
/// and its sweep is not silent — a blocked accept must unblock. It stays
/// bespoke.
trait LspOutcomePolicy: Sized {
    /// Message emitted when the request never reached the server, if the
    /// feature surfaces failures at all. Must not read the payload's
    /// key-derived fields (`server_id`/`root` on definition): the key
    /// doesn't exist yet when the gate fails.
    fn on_gate_error(self, err: FeatureGateError) -> Option<Msg>;

    /// Message emitted when this feature's UI-level abandonment deadline
    /// fires for a request the server never answered.
    fn on_timeout(self) -> Option<Msg>;
}

macro_rules! lsp_feature {
    ($ty:ty, $method:literal, $slot:ident, $supports:expr) => {
        impl LspFeature for $ty {
            const METHOD: &'static str = $method;
            fn slot(lm: &mut LspManager) -> &mut FeatureSlot<Self> {
                &mut lm.$slot
            }
            fn supports(caps: &lsp_types::ServerCapabilities) -> bool {
                $supports(caps)
            }
        }
    };
}

lsp_feature!(
    PendingDefinition,
    "textDocument/definition",
    definition,
    lsp::client::supports_definition
);
lsp_feature!(
    PendingHover,
    "textDocument/hover",
    hover,
    lsp::client::supports_hover
);
// References always sends `context.includeDeclaration: true`.
lsp_feature!(
    PendingReferences,
    "textDocument/references",
    references,
    lsp::client::supports_references
);
lsp_feature!(
    PendingCompletion,
    "textDocument/completion",
    completion,
    lsp::client::supports_completion
);
lsp_feature!(
    PendingCodeActions,
    "textDocument/codeAction",
    code_actions,
    lsp::client::supports_code_action
);

impl LspOutcomePolicy for PendingCodeActions {
    fn on_gate_error(self, err: FeatureGateError) -> Option<Msg> {
        let outcome = match err {
            FeatureGateError::NoServer | FeatureGateError::Unsupported => {
                ReferencesOutcome::NotSupported
            }
            FeatureGateError::NotReady => ReferencesOutcome::StillIndexing,
        };
        Some(self.resolved(Vec::new(), outcome))
    }

    fn on_timeout(self) -> Option<Msg> {
        Some(self.resolved(Vec::new(), ReferencesOutcome::NoResult))
    }
}

lsp_feature!(
    PendingSignatureHelp,
    "textDocument/signatureHelp",
    signature_help,
    lsp::client::supports_signature_help
);

lsp_feature!(
    PendingPrepareRename,
    "textDocument/prepareRename",
    prepare_rename,
    lsp::client::supports_prepare_rename
);
lsp_feature!(
    PendingRename,
    "textDocument/rename",
    rename,
    lsp::client::supports_rename
);

impl LspOutcomePolicy for PendingPrepareRename {
    fn on_gate_error(self, _err: FeatureGateError) -> Option<Msg> {
        Some(rename_status_msg("Rename not supported by this server"))
    }

    fn on_timeout(self) -> Option<Msg> {
        Some(rename_status_msg("Rename: server did not answer"))
    }
}

impl LspOutcomePolicy for PendingRename {
    fn on_gate_error(self, _err: FeatureGateError) -> Option<Msg> {
        Some(rename_status_msg("Rename not supported by this server"))
    }

    fn on_timeout(self) -> Option<Msg> {
        Some(rename_status_msg("Rename: server did not answer"))
    }
}

// The whole-document method; `request_lsp_formatting` swaps in
// `rangeFormatting` (+ its own capability gate) for a selection.
lsp_feature!(
    PendingFormatting,
    "textDocument/formatting",
    formatting,
    lsp::client::supports_formatting
);

impl LspOutcomePolicy for PendingFormatting {
    /// Every failure resolves with `edits: None` — update-side that is a
    /// status transient, and for `then_save` the unformatted save.
    fn on_gate_error(self, _err: FeatureGateError) -> Option<Msg> {
        self.unavailable()
    }

    fn on_timeout(self) -> Option<Msg> {
        self.unavailable()
    }
}

impl LspOutcomePolicy for PendingSignatureHelp {
    /// Silent like completion: a float that simply doesn't appear is the
    /// right failure mode for a typing-driven request.
    fn on_gate_error(self, _err: FeatureGateError) -> Option<Msg> {
        None
    }

    fn on_timeout(self) -> Option<Msg> {
        None
    }
}

impl LspOutcomePolicy for PendingDefinition {
    fn on_gate_error(self, err: FeatureGateError) -> Option<Msg> {
        let outcome = match err {
            FeatureGateError::NoServer | FeatureGateError::Unsupported => {
                DefinitionOutcome::NotSupported
            }
            FeatureGateError::NotReady => DefinitionOutcome::StillIndexing,
        };
        Some(Msg::Lsp(LspMsg::DefinitionResolved {
            document_id: self.document_id,
            revision: self.revision,
            origin: self.origin,
            outcome,
        }))
    }

    fn on_timeout(self) -> Option<Msg> {
        Some(Msg::Lsp(LspMsg::DefinitionResolved {
            document_id: self.document_id,
            revision: self.revision,
            origin: self.origin,
            outcome: DefinitionOutcome::NoResult,
        }))
    }
}

impl LspOutcomePolicy for PendingHover {
    fn on_gate_error(self, err: FeatureGateError) -> Option<Msg> {
        let outcome = match err {
            FeatureGateError::NoServer | FeatureGateError::Unsupported => {
                HoverOutcome::NotSupported
            }
            FeatureGateError::NotReady => HoverOutcome::StillIndexing,
        };
        Some(Msg::Lsp(LspMsg::HoverResolved {
            document_id: self.document_id,
            revision: self.revision,
            cursor: self.cursor,
            outcome,
        }))
    }

    fn on_timeout(self) -> Option<Msg> {
        // `HoverOutcome` has no "no result" variant distinct from
        // `Content(None)` — an abandoned request resolves the same way a
        // fast `null` reply would have.
        Some(Msg::Lsp(LspMsg::HoverResolved {
            document_id: self.document_id,
            revision: self.revision,
            cursor: self.cursor,
            outcome: HoverOutcome::Content(None),
        }))
    }
}

impl LspOutcomePolicy for PendingReferences {
    fn on_gate_error(self, err: FeatureGateError) -> Option<Msg> {
        let outcome = match err {
            FeatureGateError::NoServer | FeatureGateError::Unsupported => {
                ReferencesOutcome::NotSupported
            }
            FeatureGateError::NotReady => ReferencesOutcome::StillIndexing,
        };
        Some(Msg::Lsp(LspMsg::ReferencesResolved {
            document_id: self.document_id,
            revision: self.revision,
            cursor: self.cursor,
            items: Vec::new(),
            outcome,
        }))
    }

    fn on_timeout(self) -> Option<Msg> {
        Some(Msg::Lsp(LspMsg::ReferencesResolved {
            document_id: self.document_id,
            revision: self.revision,
            cursor: self.cursor,
            items: Vec::new(),
            outcome: ReferencesOutcome::NoResult,
        }))
    }
}

impl LspOutcomePolicy for PendingCompletion {
    /// Completion degrades silently to the menu's offline items — never a
    /// status transient, on gate failure or timeout alike.
    fn on_gate_error(self, _err: FeatureGateError) -> Option<Msg> {
        None
    }

    fn on_timeout(self) -> Option<Msg> {
        None
    }
}

/// What `LspManager` remembers about a `didOpen`'d document — enough to
/// send `didChange`/`didSave`/`didClose` without touching the model, and
/// to re-`didOpen` it against a freshly restarted server.
struct OpenDocState {
    server_id: LspServerId,
    root: PathBuf,
    uri: lsp_types::Uri,
    /// Revision most recently sent to the server (via `didOpen` or
    /// `didChange`) — lets `send_lsp_did_change` skip a no-op resend.
    synced_revision: u64,
}

impl LspManager {
    fn new() -> Self {
        Self {
            servers: HashMap::new(),
            detached_roots: Vec::new(),
            restart_attempts: HashMap::new(),
            failed_roots: HashMap::new(),
            restart_deadlines: HashMap::new(),
            open_documents: HashMap::new(),
            shutting_down: false,
            resync_pending: std::collections::HashSet::new(),
            diagnostics: HashMap::new(),
            diagnostics_versions: HashMap::new(),
            definition: FeatureSlot::new(DEFINITION_TIMEOUT),
            hover: FeatureSlot::new(HOVER_TIMEOUT),
            references: FeatureSlot::new(REFERENCES_TIMEOUT),
            code_actions: FeatureSlot::new(CODE_ACTIONS_TIMEOUT),
            completion: FeatureSlot::new(COMPLETION_TIMEOUT),
            signature_help: FeatureSlot::new(SIGNATURE_HELP_TIMEOUT),
            prepare_rename: FeatureSlot::new(RENAME_TIMEOUT),
            rename: FeatureSlot::new(RENAME_TIMEOUT),
            formatting: FeatureSlot::new(FORMATTING_TIMEOUT),
            resolve: FeatureSlot::new(RESOLVE_TIMEOUT),
            completion_debounces: HashMap::new(),
            resolve_debounces: HashMap::new(),
            missing_servers: std::collections::HashSet::new(),
        }
    }

    fn is_running(&self, id: &LspServerId, root: &Path) -> bool {
        self.servers.contains_key(&(id.clone(), root.to_path_buf()))
    }

    /// Roots a server id is currently running at (there can be more than
    /// one — a server is keyed by `(id, root)`, not just `id`).
    fn roots_for(&self, id: &LspServerId) -> Vec<PathBuf> {
        self.servers
            .keys()
            .filter(|(server_id, _)| server_id == id)
            .map(|(_, root)| root.clone())
            .collect()
    }
}

impl App {
    pub fn new(
        window_width: u32,
        window_height: u32,
        startup_config: StartupConfig,
        automation_proxy: Option<EventLoopProxy<()>>,
        renderer_preparation: Option<RendererPreparation>,
        app_preparation: Option<AppPreparation>,
    ) -> Self {
        let (msg_tx, msg_rx) = mpsc::channel();
        let (automation_tx, automation_rx) = mpsc::channel();
        if let Some(proxy) = automation_proxy.clone() {
            automation::start_server(automation_tx.clone(), proxy);
        }
        // LSP worker threads wake the event loop the same way the syntax
        // worker does; cloned before `automation_proxy` is moved below.
        let lsp_wake: Option<std::sync::Arc<dyn Fn() + Send + Sync>> =
            automation_proxy.clone().map(|proxy| {
                std::sync::Arc::new(move || {
                    let _ = proxy.send_event(());
                }) as std::sync::Arc<dyn Fn() + Send + Sync>
            });
        // Spawn the inline-suggestion worker (autocomplete.md Phase 2).
        let (inline_tx, inline_rx) = mpsc::channel();
        {
            let msg_tx_clone = msg_tx.clone();
            let proxy = automation_proxy.clone();
            std::thread::Builder::new()
                .name("inline-suggestions".into())
                .spawn(move || {
                    crate::runtime::inline_worker::inline_worker_loop(
                        inline_rx,
                        msg_tx_clone,
                        proxy,
                    )
                })
                .expect("spawn inline worker");
        }
        // Spawn syntax highlighting worker thread
        let (syntax_tx, syntax_rx) = mpsc::channel::<SyntaxWorkerRequest>();
        {
            let msg_tx_clone = msg_tx.clone();
            std::thread::spawn(move || {
                syntax_worker_loop(syntax_rx, msg_tx_clone, automation_proxy)
            });
        }

        let PreparedApp {
            model,
            keymap,
            workspace_root,
        } = app_preparation
            .and_then(AppPreparation::finish)
            .unwrap_or_else(|| prepare_app(window_width, window_height, startup_config));

        let mut app = Self {
            model,
            keymap,
            renderer: None,
            renderer_preparation,
            window: None,
            context: None,
            last_tick: Instant::now(),
            modifiers: ModifiersState::empty(),
            mouse_position: None,
            hover_dwell: None,
            scroll_accumulator: ScrollAccumulator::default(),
            option_gesture: OptionKeyGesture::default(),
            drag: DragState::default(),
            msg_tx,
            msg_rx,
            perf: PerfStats::default(),
            syntax_tx,
            fs_watcher: None,
            pending_fs_watcher_root: workspace_root,
            deferred_startup_at: None,
            deferred_startup_complete: false,
            pending_damage: Damage::Full, // Start with full render
            should_quit: false,
            webview_manager: WebviewManager::new(),
            click_tracker: ClickTracker::default(),
            syntax_deadlines: HashMap::new(),
            terminal_spawn_rx: None,
            automation_rx,
            automation_tx,
            automation_profile: None,
            document_waiters: Vec::new(),
            inline_deadlines: HashMap::new(),
            inline_tx,
            focused_at: std::time::SystemTime::now(),
            syntax_scheduled: HashMap::new(),
            syntax_present_pending: Vec::new(),
            automation_syntax_profile: None,
            latest_syntax_performance: None,
            lsp: LspManager::new(),
            lsp_wake,
            lsp_change_deadlines: lsp::sync::DidChangeDeadlines::new(),
        };

        // Trigger initial syntax parsing for all loaded documents
        app.trigger_initial_syntax_parsing();

        app
    }

    /// Trigger syntax parsing for all documents loaded at startup
    fn trigger_initial_syntax_parsing(&mut self) {
        // Collect document info first to avoid borrow issues
        let docs_to_parse: Vec<_> = self
            .model
            .editor_area
            .documents
            .iter()
            .filter(|(_, doc)| doc.language.has_highlighting())
            .map(|(&id, doc)| {
                let source: Arc<str> = doc.buffer.to_string().into();
                (id, doc.revision, source, doc.language)
            })
            .collect();

        // Send parse requests for each document
        for (doc_id, revision, source, language) in docs_to_parse {
            if let Err(e) = self
                .syntax_tx
                .send(SyntaxWorkerRequest::Parse(SyntaxParseRequest {
                    document_id: doc_id,
                    revision,
                    source,
                    language,
                    snapshot_ms: 0.0,
                    queued_at: Instant::now(),
                    extract_outline: false,
                }))
            {
                tracing::warn!("Failed to send initial syntax parse request: {}", e);
            }
        }

        // CLI-opened files (`token foo.rs`) never go through
        // `open_file_in_new_tab`'s didOpen wiring — catch them here so a
        // startup session is synced from the first frame.
        let docs_to_open: Vec<_> = self
            .model
            .editor_area
            .documents
            .iter()
            .filter_map(|(&id, doc)| doc.file_path.clone().map(|path| (id, path, doc.language)))
            .collect();
        for (document_id, file_path, language) in docs_to_open {
            self.ensure_lsp_server(language, &file_path);
            self.lsp_open_document(document_id, file_path, language);
        }
    }

    /// Dispatch a command through the update loop
    fn dispatch_command(&mut self, command: Command) -> Option<Cmd> {
        // `update()` must not do I/O, but this command's menu-enablement
        // (Paste) needs a live clipboard read — resolved here, the one
        // runtime-layer special case `Command::ShowContextMenu`'s doc
        // comment describes, then handed off to the same pure builder
        // `CommandId::ShowContextMenu` (the palette path) uses.
        if command == Command::ShowContextMenu {
            let clipboard_has_content = arboard::Clipboard::new()
                .and_then(|mut c| c.get_text())
                .is_ok_and(|text| !text.is_empty());
            return token::update::context_menu::open_editor_menu_at_caret(
                &mut self.model,
                clipboard_has_content,
            );
        }
        let mut result = None;
        for msg in command.to_msgs() {
            result = update(&mut self.model, msg).or(result);
        }
        result
    }

    /// Extract current context from the model for keybinding evaluation
    fn get_key_context(&self) -> KeyContext {
        use token::model::FocusTarget;

        let focus = self.model.ui.focus;

        KeyContext {
            has_selection: !self.model.editor().active_selection().is_empty(),
            has_multiple_cursors: self.model.editor().has_multiple_cursors(),
            modal_active: self.model.ui.has_modal(),
            editor_focused: matches!(focus, FocusTarget::Editor),
            sidebar_focused: matches!(focus, FocusTarget::Dock(DockPosition::Left)),
            overlay_routes_keys: self.model.ui.cursor_overlay.is_some(),
            inline_suggestion_visible: token::update::inline::visible(&self.model).is_some(),
        }
    }

    fn init_renderer(&mut self, window: Rc<Window>, context: &Context<Rc<Window>>) -> Result<()> {
        let renderer = match self.renderer_preparation.take() {
            Some(preparation) => Renderer::new_prepared(Rc::clone(&window), context, preparation)?,
            None => Renderer::new(Rc::clone(&window), context)?,
        };

        self.model.set_char_width(renderer.char_width());
        self.model.set_scale_factor(renderer.scale_factor());
        self.model.line_height = renderer.line_height();

        // Derive tab bar height from glyph metrics instead of hardcoded value
        self.model.recompute_tab_bar_height_from_line_height();
        let status_text_lh =
            renderer.status_text_line_height(self.model.config.status_bar_font_size_clamped());
        self.model.recompute_status_bar_height(status_text_lh);

        // Recompute viewport geometry with new metrics
        let size = window.inner_size();
        self.model.resize(size.width, size.height);

        self.renderer = Some(renderer);
        Ok(())
    }

    fn reinit_renderer(&mut self, scale_factor: f64) -> Result<()> {
        let Some(window) = &self.window else {
            return Ok(());
        };
        let Some(context) = &self.context else {
            return Ok(());
        };

        let renderer = Renderer::with_scale_factor(Rc::clone(window), context, scale_factor)?;

        self.model.set_char_width(renderer.char_width());
        self.model.line_height = renderer.line_height();

        // Recompute tab bar height from new font metrics
        self.model.recompute_tab_bar_height_from_line_height();
        let status_text_lh =
            renderer.status_text_line_height(self.model.config.status_bar_font_size_clamped());
        self.model.recompute_status_bar_height(status_text_lh);

        // Recompute viewport geometry for new char_width/line_height
        let size = window.inner_size();
        self.model.resize(size.width, size.height);

        self.renderer = Some(renderer);
        Ok(())
    }

    fn try_auto_scroll_for_drag(&mut self, y: f64) -> Option<Cmd> {
        let line_height = self.model.line_height as f64;
        let window_height = self.model.window_size.1 as f64;
        let status_bar_top = window_height - line_height;

        let direction = self.drag.try_auto_scroll(y, status_bar_top)?;
        update(&mut self.model, Msg::Editor(EditorMsg::Scroll(direction)))
    }

    /// Update both hover region tracking and cursor icon based on mouse position.
    /// Delegates to `hit_test_ui()` for unified hit-testing, then maps the result
    /// to the appropriate cursor icon and hover region.
    fn update_cursor_icon(&mut self, x: f64, y: f64) -> bool {
        use token::model::HoverRegion;
        use token::view::hit_test::{hit_test_ui, Point};

        let Some(window) = &self.window else {
            return false;
        };
        let Some(renderer) = &mut self.renderer else {
            return false;
        };

        // In-progress sidebar resize overrides all hit-testing
        if self.model.ui.sidebar_resize.is_some() {
            self.model.ui.hover = HoverRegion::SidebarResize;
            window.set_cursor(CursorIcon::ColResize);
            return false;
        }

        // In-progress dock resize overrides hit-testing
        if let Some(ref resize_state) = self.model.ui.dock_resize {
            self.model.ui.hover = HoverRegion::DockResize(resize_state.position);
            let icon = match resize_state.axis {
                token::model::ui::DockResizeAxis::Horizontal => CursorIcon::ColResize,
                token::model::ui::DockResizeAxis::Vertical => CursorIcon::RowResize,
            };
            window.set_cursor(icon);
            return false;
        }

        let pt = Point::new(x, y);
        let char_width = renderer.char_width();
        let target = {
            let mut painter = renderer.text_painter();
            let mut measure = token::layout::PainterMeasure::new(&mut painter);
            hit_test_ui(&self.model, pt, char_width, &mut measure)
        };
        window.set_cursor(
            target
                .as_ref()
                .map_or(CursorIcon::Default, |target| target.cursor_icon()),
        );
        update_hover_target(&mut self.model, target.as_ref())
    }

    /// Mouse-dwell hover bookkeeping for `CursorMoved` — call after
    /// `update_cursor_icon` so `self.model.ui.hover` already reflects the
    /// new position. `prev` is the mouse position before this move (`None`
    /// on the very first move, treated as significant).
    fn update_hover_dwell(&mut self, prev: Option<(f64, f64)>, x: f64, y: f64) {
        use token::model::{CursorOverlayKind, CursorOverlayState, HoverRegion};

        let moved_significantly = prev
            .map(|(px, py)| {
                let (dx, dy) = (x - px, y - py);
                (dx * dx + dy * dy).sqrt() > HOVER_DWELL_MOVE_THRESHOLD_PX
            })
            .unwrap_or(true);
        if !moved_significantly {
            return;
        }

        // A Hover card is dismissed by a significant move that lands
        // outside its own panel — moving inside the card (it's
        // scrollable/clickable, hit_test_ui claims it first) must not
        // dismiss it (overlay-surface.md Phase 5 pointer spec).
        let showing_hover_card = matches!(
            self.model.ui.cursor_overlay,
            Some(CursorOverlayState {
                kind: CursorOverlayKind::Hover,
                ..
            })
        );
        if showing_hover_card && self.model.ui.hover != HoverRegion::CursorOverlay {
            self.model.ui.cursor_overlay = None;
            self.model.ui.hover_card = None;
        }

        // No fresh dwell while a modal or any cursor-anchored popup is up
        // (including one that just got dismissed above) — moving within
        // the editor to a new position only re-arms dwell once nothing is
        // showing.
        self.hover_dwell = (!self.model.ui.has_modal() && self.model.ui.cursor_overlay.is_none())
            .then_some((x, y, Instant::now()));
    }

    /// Fires `LspMsg::ShowHoverAt` once the pointer has dwelled past
    /// `config.hover_delay_ms` over editor text — the mouse-driven
    /// counterpart to `CommandId::ShowHover`'s caret-anchored request.
    /// Returns whether a redraw is needed.
    fn check_hover_dwell(&mut self) -> bool {
        if !self.model.config.hover_on_mouse {
            return false;
        }
        let Some((x, y, started)) = self.hover_dwell else {
            return false;
        };
        if self.model.ui.has_modal() || self.model.ui.cursor_overlay.is_some() {
            return false;
        }
        if self.model.ui.hover != token::model::HoverRegion::EditorText {
            return false;
        }
        if started.elapsed() < Duration::from_millis(self.model.config.hover_delay_ms) {
            return false;
        }
        let Some(renderer) = &mut self.renderer else {
            return false;
        };
        // One-shot: don't refire every tick once the delay has elapsed —
        // a fresh dwell starts only after the next significant move.
        self.hover_dwell = None;
        let (line, col) = renderer.pixel_to_cursor(x, y, &self.model);
        let Some(cmd) = update(&mut self.model, Msg::Lsp(LspMsg::ShowHoverAt { line, col })) else {
            return false;
        };
        let needs_redraw = cmd.needs_redraw();
        self.pending_damage.merge(cmd.damage());
        self.process_cmd(cmd);
        needs_redraw
    }

    fn sync_text_input_rect(&self) {
        let Some(window) = &self.window else { return };
        let Some(rect) = token::view::caret::active_text_input_rect(
            &self.model,
            self.model.char_width,
            self.model.line_height,
        ) else {
            return;
        };

        window.set_ime_cursor_area(
            PhysicalPosition::new(rect.x as i32, rect.y as i32),
            PhysicalSize::new(rect.w as u32, rect.h as u32),
        );
    }

    fn handle_event(&mut self, event: &WindowEvent) -> Option<Cmd> {
        match event {
            WindowEvent::Resized(size) => update(
                &mut self.model,
                Msg::App(AppMsg::Resize(size.width, size.height)),
            ),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => update(
                &mut self.model,
                Msg::App(AppMsg::ScaleFactorChanged(*scale_factor)),
            ),
            WindowEvent::Focused(focused) => {
                if *focused {
                    self.focused_at = std::time::SystemTime::now();
                }
                // Window focus loss dismisses the completion popup — the
                // documented autocomplete.md gap. The popup claims
                // Up/Down/Enter/Tab pre-keymap, so leaving it open while
                // another app has focus leaves dead keys behind; the user's
                // next interaction with a refocused editor reopens it.
                if !focused && self.model.ui.completion_menu.is_some() {
                    update(&mut self.model, Msg::Completion(CompletionMsg::Dismiss))
                } else {
                    None
                }
            }
            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
                None
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let is_option_key = matches!(
                    event.physical_key,
                    PhysicalKey::Code(KeyCode::AltLeft) | PhysicalKey::Code(KeyCode::AltRight)
                );

                if is_option_key {
                    if event.state == ElementState::Pressed && !event.repeat {
                        self.option_gesture.on_press();
                    } else if event.state == ElementState::Released {
                        self.option_gesture.on_release();
                    }
                }

                if event.state == ElementState::Pressed {
                    #[cfg(debug_assertions)]
                    if event.logical_key == Key::Named(NamedKey::F2) {
                        self.perf.show_overlay = !self.perf.show_overlay;
                        return Some(Cmd::Redraw);
                    }

                    #[cfg(debug_assertions)]
                    if event.logical_key == Key::Named(NamedKey::F7) {
                        let dump = crate::debug_dump::StateDump::from_model(&self.model);
                        match dump.save_to_file() {
                            Ok(filename) => eprintln!("[DEBUG] State dumped to: {}", filename),
                            Err(e) => eprintln!("[DEBUG] Failed to dump state: {}", e),
                        }
                        return Some(Cmd::Redraw);
                    }

                    #[cfg(debug_assertions)]
                    if event.logical_key == Key::Named(NamedKey::F8) {
                        if let Some(ref mut overlay) = self.model.debug_overlay {
                            overlay.toggle();
                        }
                        return Some(Cmd::Redraw);
                    }

                    #[cfg(debug_assertions)]
                    if event.logical_key == Key::Named(NamedKey::F9) {
                        return token::update::execute_command(
                            &mut self.model,
                            token::commands::CommandId::CycleCursorOverlayDemo,
                        );
                    }

                    let ctrl = self.modifiers.control_key();
                    let shift = self.modifiers.shift_key();
                    let alt = self.modifiers.alt_key();
                    let logo = self.modifiers.super_key();
                    let modifiers = KeyModifiers {
                        ctrl,
                        shift,
                        alt,
                        logo,
                    };

                    // Convert the raw winit event to our Keystroke type once. This is a
                    // pure conversion of the event + modifiers and doesn't depend on
                    // keymap state, so the same value can be reused for both the
                    // global-command check below and the non-global check further down
                    // (previously this called keystroke_from_winit twice with identical
                    // arguments).
                    let keystroke = keystroke_from_winit(
                        &event.logical_key,
                        event.physical_key,
                        ctrl,
                        shift,
                        alt,
                        logo,
                    );

                    // Cursor-anchored popups aren't modals — they claim exactly
                    // Up/Down/Enter/Esc/Tab (+PageUp/PageDown,
                    // lsp-integration.md Phase 5) and must
                    // claim them *before* the keymap runs, or bindings like
                    // Up -> MoveCursorUp / Enter -> InsertNewline (both
                    // `is_simple()`, non-global) would dispatch and consume the
                    // key first. Every other key (Backspace, Delete, arrows with
                    // modifiers, Cmd+C/V/X/Z/A, ...) falls through to the normal
                    // keymap/handle_key path below unaffected.
                    if self.model.ui.cursor_overlay.is_some() {
                        if let Some(cmd) = handle_cursor_overlay_key(
                            &mut self.model,
                            &event.logical_key,
                            modifiers,
                        ) {
                            return cmd;
                        }
                    }

                    // Check for global commands first (work regardless of focus state)
                    // These include command palette, save, quit, etc.
                    if let Some(keystroke) = keystroke {
                        let context = self.get_key_context();
                        if let KeyAction::Execute(command) = self
                            .keymap
                            .handle_keystroke_with_context(keystroke, Some(&context))
                        {
                            if command.is_global() {
                                return self.dispatch_command(command);
                            }
                        }
                        // Reset keymap state after global check (we'll re-check below if needed)
                        self.keymap.reset();
                    }

                    // Try keymap for non-global commands, but only when:
                    // - No modal is active (modals handled by handle_modal_key in input.rs)
                    // - Not in option double-tap mode with alt pressed (multi-cursor gesture)
                    // - Sidebar is not focused (sidebar keys handled by handle_sidebar_key in input.rs)
                    // - Outline is not focused (outline keys handled by handle_outline_dock_key in input.rs)
                    // - Not editing a CSV cell (CSV cell editor handled by handle_csv_edit_key in input.rs)
                    let skip_keymap = should_skip_non_global_keymap(
                        &self.model,
                        self.option_gesture.double_tapped,
                        alt,
                    );

                    if !skip_keymap {
                        if let Some(keystroke) = keystroke {
                            let context = self.get_key_context();
                            match self
                                .keymap
                                .handle_keystroke_with_context(keystroke, Some(&context))
                            {
                                KeyAction::Execute(command) if command.is_simple() => {
                                    return self.dispatch_command(command);
                                }
                                KeyAction::AwaitMore => {
                                    // Chord in progress - don't fall through to handle_key
                                    return Some(Cmd::Redraw);
                                }
                                _ => {
                                    // NoMatch or complex command - fall through to handle_key
                                }
                            }
                        }
                    }

                    // Fall back to legacy handle_key for complex/context-dependent behavior
                    handle_key(
                        &mut self.model,
                        event.logical_key.clone(),
                        event.physical_key,
                        modifiers,
                        self.option_gesture.double_tapped,
                    )
                } else {
                    None
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.render() {
                    eprintln!("Render error: {}", e);
                }
                None
            }
            WindowEvent::CursorMoved { position, .. } => {
                let prev_mouse_position = self.mouse_position;
                self.mouse_position = Some((position.x, position.y));
                let hover_changed = self.update_cursor_icon(position.x, position.y);
                self.update_hover_dwell(prev_mouse_position, position.x, position.y);

                // A modal being open doesn't rule out a drag that started
                // before it opened (splitter/scrollbar/etc.), so those
                // branches still take priority below. Otherwise, row hover
                // wash (overlay-surface.md Pointer) only needs a repaint
                // when the hovered row actually changes, not on every move.
                if self.model.ui.has_modal()
                    && self.model.ui.splitter_drag.is_none()
                    && self.model.ui.scrollbar_drag.is_none()
                    && self.model.ui.sidebar_resize.is_none()
                    && self.model.ui.dock_resize.is_none()
                {
                    return hover_changed.then_some(Cmd::Redraw);
                }

                // Handle splitter drag first (highest priority)
                if self.model.ui.splitter_drag.is_some() {
                    return update(
                        &mut self.model,
                        Msg::Layout(LayoutMsg::UpdateSplitterDrag {
                            position: (position.x as f32, position.y as f32),
                        }),
                    );
                }

                // Handle scrollbar thumb drag
                if let Some(drag) = &self.model.ui.scrollbar_drag {
                    use token::model::ui::ScrollbarDragAxis;
                    let coord = match drag.axis {
                        ScrollbarDragAxis::Vertical => position.y as f32,
                        ScrollbarDragAxis::Horizontal => position.x as f32,
                    };
                    return update(
                        &mut self.model,
                        Msg::Ui(UiMsg::ScrollbarDragUpdate { mouse_coord: coord }),
                    );
                }

                // Handle sidebar resize drag
                if self.model.ui.sidebar_resize.is_some() {
                    return update(
                        &mut self.model,
                        Msg::Workspace(WorkspaceMsg::UpdateSidebarResize { x: position.x }),
                    );
                }

                // Handle dock resize drag
                if let Some(ref resize_state) = self.model.ui.dock_resize {
                    let coord = match resize_state.axis {
                        token::model::ui::DockResizeAxis::Horizontal => position.x,
                        token::model::ui::DockResizeAxis::Vertical => position.y,
                    };
                    return update(
                        &mut self.model,
                        Msg::Dock(token::messages::DockMsg::UpdateResize { coord }),
                    );
                }

                // Handle tab drag (armed on tab press, activates past threshold)
                if self.model.ui.tab_drag.is_some() {
                    return update_tab_drag(&mut self.model, position.x, position.y);
                }

                // Handle image panning and mouse tracking
                if let Some(editor) = self.model.editor_area.focused_editor() {
                    if editor.view_mode.is_image() {
                        let has_drag = editor
                            .view_mode
                            .as_image()
                            .map(|img| img.drag.is_some())
                            .unwrap_or(false);

                        if self.drag.is_down() && has_drag {
                            update(
                                &mut self.model,
                                Msg::Image(ImageMsg::UpdatePan {
                                    x: position.x,
                                    y: position.y,
                                }),
                            );
                        }

                        update(
                            &mut self.model,
                            Msg::Image(ImageMsg::MouseMove {
                                x: position.x,
                                y: position.y,
                            }),
                        );

                        return Some(Cmd::Redraw);
                    }
                }

                if self.model.editor().rectangle_selection.active {
                    if let Some(renderer) = &mut self.renderer {
                        // Use visual column (screen position) for rectangle selection
                        let (line, visual_col) = renderer.pixel_to_line_and_visual_column(
                            position.x,
                            position.y,
                            &self.model,
                        );
                        return update(
                            &mut self.model,
                            Msg::Editor(EditorMsg::UpdateRectangleSelection { line, visual_col }),
                        );
                    }
                } else if self.drag.is_down() {
                    if let Some(renderer) = &mut self.renderer {
                        // Check if drag threshold was just crossed
                        if let Some((start_x, start_y)) =
                            self.drag.check_threshold(position.x, position.y)
                        {
                            let (start_line, start_col) =
                                renderer.pixel_to_cursor(start_x, start_y, &self.model);
                            self.model.editor_mut().primary_selection_mut().anchor =
                                Position::new(start_line, start_col);
                        }

                        if self.drag.is_active() {
                            let (line, column) =
                                renderer.pixel_to_cursor(position.x, position.y, &self.model);

                            self.model.editor_mut().primary_cursor_mut().line = line;
                            self.model.editor_mut().primary_cursor_mut().column = column;
                            self.model.editor_mut().primary_selection_mut().head =
                                Position::new(line, column);

                            self.try_auto_scroll_for_drag(position.y);

                            return Some(Cmd::Redraw);
                        }
                    }
                }
                hover_changed.then_some(Cmd::Redraw)
            }
            WindowEvent::CursorLeft { .. } => {
                self.hover_dwell = None;
                update_hover_target(&mut self.model, None).then_some(Cmd::Redraw)
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                self.hover_dwell = None;
                if let Some((x, y)) = self.mouse_position {
                    if let Some(renderer) = &mut self.renderer {
                        let event = make_mouse_event(x, y, MouseButton::Left, self.modifiers);
                        let result = handle_mouse_press(
                            &mut self.model,
                            renderer,
                            event,
                            &mut self.click_tracker,
                        );

                        // Update drag tracking state
                        if result.start_drag_tracking {
                            self.drag.begin(x, y);
                        }

                        return result.cmd;
                    }
                }
                None
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                self.hover_dwell = None;
                if let Some((x, y)) = self.mouse_position {
                    if let Some(renderer) = &mut self.renderer {
                        let event = make_mouse_event(x, y, MouseButton::Right, self.modifiers);
                        let result = handle_mouse_press(
                            &mut self.model,
                            renderer,
                            event,
                            &mut self.click_tracker,
                        );
                        return result.cmd;
                    }
                }
                None
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                self.drag.end();

                // Finish tab drag if one is armed/active
                if self.model.ui.tab_drag.is_some() {
                    if let Some(cmd) = end_tab_drag(&mut self.model) {
                        return Some(cmd);
                    }
                }

                // End splitter drag if active
                if self.model.ui.splitter_drag.is_some() {
                    return update(&mut self.model, Msg::Layout(LayoutMsg::EndSplitterDrag));
                }

                // End scrollbar drag if active
                if self.model.ui.scrollbar_drag.is_some() {
                    return update(&mut self.model, Msg::Ui(UiMsg::ScrollbarDragEnd));
                }

                // End sidebar resize drag if active
                if self.model.ui.sidebar_resize.is_some() {
                    return update(
                        &mut self.model,
                        Msg::Workspace(WorkspaceMsg::EndSidebarResize),
                    );
                }

                // End dock resize drag if active
                if self.model.ui.dock_resize.is_some() {
                    return update(
                        &mut self.model,
                        Msg::Dock(token::messages::DockMsg::EndResize),
                    );
                }

                // End image pan if active
                if let Some(editor) = self.model.editor_area.focused_editor() {
                    if editor
                        .view_mode
                        .as_image()
                        .map(|img| img.drag.is_some())
                        .unwrap_or(false)
                    {
                        return update(&mut self.model, Msg::Image(ImageMsg::EndPan));
                    }
                }
                None
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Back,
                ..
            } => {
                self.hover_dwell = None;
                // Mouse "back" button navigates the jump history globally,
                // matching JetBrains.
                update(&mut self.model, Msg::Lsp(LspMsg::NavigateBack))
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Forward,
                ..
            } => {
                self.hover_dwell = None;
                update(&mut self.model, Msg::Lsp(LspMsg::NavigateForward))
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Middle,
                ..
            } => {
                self.hover_dwell = None;
                if let Some((x, y)) = self.mouse_position {
                    if let Some(renderer) = &mut self.renderer {
                        let event = make_mouse_event(x, y, MouseButton::Middle, self.modifiers);
                        let result = handle_mouse_press(
                            &mut self.model,
                            renderer,
                            event,
                            &mut self.click_tracker,
                        );
                        return result.cmd;
                    }
                }
                None
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Middle,
                ..
            } => {
                if self.model.editor().rectangle_selection.active {
                    return update(
                        &mut self.model,
                        Msg::Editor(EditorMsg::FinishRectangleSelection),
                    );
                }
                None
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (h_delta, v_delta) = self.scroll_accumulator.deltas(
                    *delta,
                    self.model.char_width as f64,
                    self.model.line_height as f64,
                );

                handle_mouse_wheel(&mut self.model, self.mouse_position, h_delta, v_delta)
            }
            WindowEvent::DroppedFile(path) => {
                // Clear hover state first
                self.model.ui.drop_state.cancel_hover();
                update(
                    &mut self.model,
                    Msg::Layout(LayoutMsg::OpenFileInNewTab(path.clone())),
                )
            }
            WindowEvent::HoveredFile(path) => {
                update(&mut self.model, Msg::Ui(UiMsg::FileHovered(path.clone())))
            }
            WindowEvent::HoveredFileCancelled => {
                update(&mut self.model, Msg::Ui(UiMsg::FileHoverCancelled))
            }
            _ => None,
        }
    }

    fn render(&mut self) -> Result<()> {
        self.perf.start_frame();

        if let Some(renderer) = &mut self.renderer {
            // Take pending damage and reset to empty for next frame
            let damage = std::mem::take(&mut self.pending_damage);
            renderer.render(&mut self.model, &mut self.perf, &damage)?;
        }

        self.sync_text_input_rect();

        // Sync webviews with preview panes.
        //
        // `sync_webviews` takes `&mut self` (the whole `App`), so it can't be
        // passed as a closure directly to `self.perf.measure_stage(...)` -
        // that would require borrowing `self.perf` and all of `self`
        // simultaneously. Swap `perf` out to a local for the duration of the
        // call instead of hand-rolling the timing with `Instant::now()`.
        let mut perf = std::mem::take(&mut self.perf);
        perf.measure_stage(PerfStage::WebviewSync, || self.sync_webviews());
        self.perf = perf;

        // Hide webviews when modals are active (so they don't render on top)
        let show_webviews = self.model.ui.active_modal.is_none();
        self.perf.measure_stage(PerfStage::WebviewVisibility, || {
            self.webview_manager.set_all_visible(show_webviews);
        });

        self.perf.record_frame_time();
        self.perf.record_render_history();
        if !self.deferred_startup_complete
            && self.deferred_startup_at.is_none()
            && self.has_deferred_startup_work()
        {
            self.deferred_startup_at = Some(Instant::now() + POST_FIRST_FRAME_STARTUP_DELAY);
        }
        for mut pending in std::mem::take(&mut self.syntax_present_pending) {
            pending.snapshot.edit_to_present_ms =
                pending.started_at.elapsed().as_secs_f64() * 1000.0;
            self.latest_syntax_performance = Some(pending.snapshot);
            if let Some(response_tx) = pending.response_tx {
                let _ = response_tx.send(self.automation_response("syntax profile complete"));
            }
        }
        if let Some(mut profile) = self.automation_profile.take() {
            profile.remaining_frames = profile.remaining_frames.saturating_sub(1);
            if profile.remaining_frames == 0 {
                let response = self.automation_response("profile complete");
                let _ = profile.response_tx.send(response);
            } else {
                self.pending_damage = Damage::Full;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                self.automation_profile = Some(profile);
            }
        }
        Ok(())
    }

    fn has_deferred_startup_work(&self) -> bool {
        self.pending_fs_watcher_root.is_some() || cfg!(target_os = "macos")
    }

    fn finish_deferred_startup(&mut self) {
        self.deferred_startup_at = None;
        self.deferred_startup_complete = true;

        #[cfg(target_os = "macos")]
        super::macos_menu::install();

        if let Some(root) = self.pending_fs_watcher_root.take() {
            match FileSystemWatcher::new(root) {
                Ok(watcher) => self.fs_watcher = Some(watcher),
                Err(error) => {
                    tracing::warn!("Failed to start file system watcher: {error}");
                }
            }
        }
    }

    /// Synchronize webview instances with preview panes in the model.
    /// Creates, updates, or destroys webviews as needed.
    fn sync_webviews(&mut self) {
        use super::webview::PreviewContent;
        use token::layout::editor::PreviewPaneLayout;
        use token::markdown::{content_to_preview_html, PreviewTheme};
        use token::model::editor_area::PreviewId;
        use token::syntax::LanguageId;

        let Some(window) = &self.window else {
            return;
        };

        // Get current preview IDs from the model
        let model_preview_ids: std::collections::HashSet<_> =
            self.model.editor_area.previews.keys().copied().collect();

        // Debug: log preview count
        if !model_preview_ids.is_empty() {
            tracing::debug!(
                "sync_webviews: {} preview(s) in model",
                model_preview_ids.len()
            );
        }

        // Get current webview preview IDs
        let webview_preview_ids: std::collections::HashSet<_> =
            self.webview_manager.active_previews().into_iter().collect();

        // Remove webviews for closed previews
        for preview_id in webview_preview_ids.difference(&model_preview_ids) {
            self.webview_manager.close_webview(*preview_id);
        }

        // Collect info about previews that need updates (to avoid borrow issues)
        let scale_factor = self.model.metrics.scale_factor;
        let theme = PreviewTheme::from_editor_theme(&self.model.theme);
        let metrics = &self.model.metrics;

        struct PreviewUpdate {
            preview_id: PreviewId,
            rect: token::model::editor_area::Rect,
            content: Option<PreviewContent>,
            doc_revision: u64,
            needs_content_update: bool,
            needs_create: bool,
        }

        let updates: Vec<PreviewUpdate> = self
            .model
            .editor_area
            .previews
            .iter()
            .filter_map(|(&preview_id, preview)| {
                let document = self.model.editor_area.documents.get(&preview.document_id)?;
                let needs_create = !self.webview_manager.has_webview(preview_id);
                let needs_content_update = preview.needs_refresh(document.revision);

                let webview_rect =
                    PreviewPaneLayout::new(preview_id, preview.rect, metrics).hosted_content_rect();

                // Only generate HTML when creating or updating content
                let content = if needs_create || needs_content_update {
                    let buffer_content = document.buffer.to_string();
                    let html = content_to_preview_html(&buffer_content, document.language, &theme)?;

                    // For HTML files with a file path, enable local resource loading
                    Some(if document.language == LanguageId::Html {
                        if let Some(file_path) = &document.file_path {
                            if let Some(base_dir) = file_path.parent() {
                                PreviewContent::HtmlFile {
                                    html,
                                    base_dir: base_dir.to_path_buf(),
                                }
                            } else {
                                PreviewContent::Html(html)
                            }
                        } else {
                            PreviewContent::Html(html)
                        }
                    } else {
                        PreviewContent::Html(html)
                    })
                } else {
                    None
                };

                Some(PreviewUpdate {
                    preview_id,
                    rect: webview_rect,
                    content,
                    doc_revision: document.revision,
                    needs_content_update,
                    needs_create,
                })
            })
            .collect();

        // Apply updates
        for update in updates {
            // Debug: log webview rect updates
            tracing::debug!(
                "Webview {:?}: rect=({:.0}, {:.0}, {:.0}x{:.0}) create={} content_update={}",
                update.preview_id,
                update.rect.x,
                update.rect.y,
                update.rect.width,
                update.rect.height,
                update.needs_create,
                update.needs_content_update
            );

            if update.needs_create {
                // Create new webview
                if let Some(content) = update.content {
                    if let Err(e) = self.webview_manager.create_webview(
                        update.preview_id,
                        window,
                        update.rect,
                        content,
                    ) {
                        tracing::error!("Failed to create webview for preview: {}", e);
                        continue;
                    }
                }
                // Update last_revision after successful creation
                if let Some(preview) = self.model.editor_area.preview_mut(update.preview_id) {
                    preview.last_revision = update.doc_revision;
                }
            } else {
                // Update existing webview bounds
                self.webview_manager
                    .update_bounds(update.preview_id, update.rect, scale_factor);

                // Update content if revision changed
                if update.needs_content_update {
                    if let Some(content) = update.content {
                        self.webview_manager
                            .update_content(update.preview_id, content);
                    }
                    // Update last_revision after content update
                    if let Some(preview) = self.model.editor_area.preview_mut(update.preview_id) {
                        preview.last_revision = update.doc_revision;
                    }
                }
            }
        }
    }

    fn tick(&mut self) -> Option<Cmd> {
        update(&mut self.model, Msg::Ui(UiMsg::BlinkCursor))
    }

    /// Quit-time teardown for every running server: the design doc's
    /// `shutdown` -> await response -> `exit` -> await process exit ->
    /// kill sequence, run per server (`ServerHandle::graceful_shutdown`).
    /// `shutting_down` is set first so the `ServerExited` each kill
    /// produces never triggers a restart.
    fn graceful_lsp_teardown(&mut self) {
        self.lsp.shutting_down = true;
        let keys: Vec<_> = self.lsp.servers.keys().cloned().collect();
        // One shared deadline for the *entire* teardown, not per server —
        // otherwise N running servers pay up to N * (2s + 2s) sequentially
        // (see `ServerHandle::graceful_shutdown`'s doc comment). Each
        // server still gets at most 2s per phase, but a server reached
        // late in the loop gets whatever's left of this budget instead of
        // a fresh 4s on top of what earlier servers already spent.
        let shared_deadline = std::time::Instant::now() + Duration::from_secs(4);
        for key in keys {
            self.set_lsp_server_state(key.0.clone(), &key.1, ServerState::ShuttingDown);
            if let Some(mut handle) = self.lsp.servers.remove(&key) {
                // `shutdown`/`exit` both pass through the handshake gate —
                // if `initialize` hasn't answered yet (or answered with an
                // error, which never opens the gate at all — see the
                // reader's `initialize` error arm), they'd sit queued
                // forever and this would block the full 2s+2s waiting for
                // acks that can never arrive. Skip straight to `kill` in
                // that case; there is no live handshake to shut down
                // gracefully.
                if handle.capabilities_snapshot().is_some() {
                    handle.graceful_shutdown(&self.msg_rx, Duration::from_secs(2), shared_deadline);
                } else {
                    handle.kill();
                }
            }
        }
    }

    fn process_cmd(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::None => {}
            Cmd::Redraw => {}
            Cmd::RedrawAreas(_) => {} // Partial redraw - handled by damage tracking in render()
            Cmd::SyncStatusBarMetrics => {
                if let Some(renderer) = &self.renderer {
                    let status_text_lh = renderer
                        .status_text_line_height(self.model.config.status_bar_font_size_clamped());
                    self.model.recompute_status_bar_height(status_text_lh);
                    let (w, h) = self.model.window_size;
                    self.model.resize(w, h);
                }
            }
            Cmd::ReinitializeRenderer => {
                let scale_factor = self.model.metrics.scale_factor;
                if let Err(e) = self.reinit_renderer(scale_factor) {
                    tracing::error!("Failed to reinitialize renderer: {}", e);
                }
            }
            Cmd::SaveFile { path, content } => {
                let tx = self.msg_tx.clone();
                std::thread::spawn(move || {
                    let result = std::fs::write(&path, content).map_err(|e| e.to_string());
                    if let Err(e) = tx.send(Msg::App(AppMsg::SaveCompleted(result))) {
                        tracing::warn!("Failed to send save completion to main thread: {}", e);
                    }
                });
            }
            Cmd::SaveFileAs {
                document_id,
                old_path,
                new_path,
                content,
            } => {
                let tx = self.msg_tx.clone();
                std::thread::spawn(move || {
                    let result = std::fs::write(&new_path, content).map_err(|e| e.to_string());
                    let msg = Msg::App(AppMsg::SaveAsCompleted {
                        document_id,
                        old_path,
                        new_path,
                        result,
                    });
                    if let Err(e) = tx.send(msg) {
                        tracing::warn!("Failed to send save-as completion to main thread: {}", e);
                    }
                });
            }
            Cmd::LoadFile { path } => {
                let tx = self.msg_tx.clone();
                std::thread::spawn(move || {
                    let result = std::fs::read_to_string(&path).map_err(|e| e.to_string());
                    if let Err(e) = tx.send(Msg::App(AppMsg::FileLoaded { path, result })) {
                        tracing::warn!("Failed to send file load result to main thread: {}", e);
                    }
                });
            }
            Cmd::OpenInExplorer { path } => {
                #[cfg(target_os = "macos")]
                {
                    if let Err(e) = std::process::Command::new("open").arg(&path).spawn() {
                        tracing::warn!("Failed to open file in default app: {}", e);
                    }
                }
                #[cfg(target_os = "windows")]
                {
                    if let Err(e) = std::process::Command::new("explorer").arg(&path).spawn() {
                        tracing::warn!("Failed to open file in explorer: {}", e);
                    }
                }
                #[cfg(target_os = "linux")]
                {
                    if let Err(e) = std::process::Command::new("xdg-open").arg(&path).spawn() {
                        tracing::warn!("Failed to open file with xdg-open: {}", e);
                    }
                }
            }
            Cmd::RevealFileInFinder { path } => {
                #[cfg(target_os = "macos")]
                {
                    if let Err(e) = std::process::Command::new("open")
                        .arg("-R")
                        .arg(&path)
                        .spawn()
                    {
                        tracing::warn!("Failed to reveal file in Finder: {}", e);
                    }
                }
                #[cfg(target_os = "linux")]
                {
                    if let Some(parent) = path.parent() {
                        if let Err(e) = std::process::Command::new("xdg-open").arg(parent).spawn() {
                            tracing::warn!("Failed to reveal file in file manager: {}", e);
                        }
                    }
                }
                #[cfg(target_os = "windows")]
                {
                    if let Err(e) = std::process::Command::new("explorer")
                        .arg("/select,")
                        .arg(&path)
                        .spawn()
                    {
                        tracing::warn!("Failed to reveal file in Explorer: {}", e);
                    }
                }
            }
            Cmd::OpenFileInEditor { path } => {
                let tx = self.msg_tx.clone();
                std::thread::spawn(move || {
                    let result = std::fs::read_to_string(&path).map_err(|e| e.to_string());
                    if let Err(e) = tx.send(Msg::App(AppMsg::FileLoaded { path, result })) {
                        tracing::warn!("Failed to send file load result to main thread: {}", e);
                    }
                });
            }
            Cmd::SaveConfiguration { config } => {
                if let Err(error) = config.save() {
                    self.model
                        .ui
                        .set_status(format!("Could not save settings: {error}"));
                }
            }
            Cmd::SaveRecentFiles { recent } => {
                std::thread::spawn(move || {
                    if let Err(e) = recent.save() {
                        tracing::warn!("Failed to save recent files: {}", e);
                    }
                });
            }
            Cmd::SaveCommandHistory { history } => {
                std::thread::spawn(move || {
                    if let Err(e) = history.save() {
                        tracing::warn!("Failed to save command history: {}", e);
                    }
                });
            }
            Cmd::CopyToClipboard(text) => {
                std::thread::spawn(move || {
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        if let Err(e) = clipboard.set_text(&text) {
                            tracing::warn!("Failed to copy to clipboard: {}", e);
                        }
                    } else {
                        tracing::warn!("Failed to initialize clipboard");
                    }
                });
            }
            Cmd::RequestClipboardPaste => {
                let tx = self.msg_tx.clone();
                std::thread::spawn(move || {
                    let clipboard_text = if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        clipboard.get_text().unwrap_or_default()
                    } else {
                        tracing::warn!("Failed to open clipboard for pasting");
                        String::new()
                    };
                    if let Err(e) = tx.send(Msg::App(AppMsg::PasteFromClipboard(clipboard_text))) {
                        tracing::warn!(
                            "Failed to send clipboard paste message to main thread: {}",
                            e
                        );
                    }
                });
            }
            Cmd::CreateDefaultKeymapFile { path } => {
                let tx = self.msg_tx.clone();
                std::thread::spawn(move || {
                    let result = token::update::create_default_keymap_file(&path);
                    if let Err(e) = tx.send(Msg::App(AppMsg::KeymapCreated { path, result })) {
                        tracing::warn!(
                            "Failed to send keymap created message to main thread: {}",
                            e
                        );
                    }
                });
            }
            Cmd::SpawnTerminal {
                session_id,
                rows,
                cols,
            } => {
                if let Some((pending_session_id, _)) = self.terminal_spawn_rx.as_ref() {
                    tracing::debug!(
                        "Ignoring terminal spawn for session {session_id}; spawn for session {pending_session_id} is pending"
                    );
                    if *pending_session_id != session_id {
                        self.model.terminal.clear_spawn_pending(session_id);
                    }
                    return;
                }

                self.model.terminal.mark_spawn_pending(session_id);
                let cwd = self
                    .model
                    .workspace
                    .as_ref()
                    .map(|workspace| workspace.root.clone())
                    .or_else(|| std::env::current_dir().ok())
                    .unwrap_or_else(std::env::temp_dir);

                let (spawn_tx, spawn_rx) = mpsc::channel();
                self.terminal_spawn_rx = Some((session_id, spawn_rx));
                let msg_tx = self.msg_tx.clone();
                std::thread::spawn(move || {
                    let result = token::terminal::spawn_pty(&cwd, rows, cols, msg_tx, session_id)
                        .map(|pty| token::terminal::TerminalSpawnResult {
                            session_id,
                            rows: rows as usize,
                            cols: cols as usize,
                            pty,
                        })
                        .map_err(|e| e.to_string());
                    if let Err(e) = spawn_tx.send(result) {
                        tracing::warn!("Failed to send terminal spawn result: {e:?}");
                    }
                });
            }
            Cmd::Batch(cmds) => {
                for cmd in cmds {
                    self.process_cmd(cmd);
                }
            }

            // =====================================================================
            // File Dialogs (using rfd)
            // =====================================================================
            Cmd::ShowOpenFileDialog {
                allow_multi,
                start_dir,
            } => {
                let tx = self.msg_tx.clone();
                std::thread::spawn(move || {
                    let mut dlg = rfd::FileDialog::new();
                    if let Some(dir) = start_dir {
                        dlg = dlg.set_directory(dir);
                    }

                    let paths = if allow_multi {
                        dlg.pick_files().unwrap_or_default()
                    } else {
                        dlg.pick_file().into_iter().collect()
                    };

                    if let Err(e) = tx.send(Msg::App(AppMsg::OpenFileDialogResult { paths })) {
                        tracing::warn!(
                            "Failed to send open file dialog result to main thread: {}",
                            e
                        );
                    }
                });
            }

            Cmd::ShowSaveFileDialog { suggested_path } => {
                let tx = self.msg_tx.clone();
                std::thread::spawn(move || {
                    let mut dlg = rfd::FileDialog::new();
                    if let Some(ref path) = suggested_path {
                        if let Some(dir) = path.parent() {
                            dlg = dlg.set_directory(dir);
                        }
                        if let Some(name) = path.file_name() {
                            dlg = dlg.set_file_name(name.to_string_lossy());
                        }
                    }

                    let path = dlg.save_file();
                    if let Err(e) = tx.send(Msg::App(AppMsg::SaveFileAsDialogResult { path })) {
                        tracing::warn!(
                            "Failed to send save file dialog result to main thread: {}",
                            e
                        );
                    }
                });
            }

            Cmd::ShowOpenFolderDialog { start_dir } => {
                let tx = self.msg_tx.clone();
                std::thread::spawn(move || {
                    let mut dlg = rfd::FileDialog::new();
                    if let Some(dir) = start_dir {
                        dlg = dlg.set_directory(dir);
                    }

                    let folder = dlg.pick_folder();
                    if let Err(e) = tx.send(Msg::App(AppMsg::OpenFolderDialogResult { folder })) {
                        tracing::warn!(
                            "Failed to send open folder dialog result to main thread: {}",
                            e
                        );
                    }
                });
            }

            // =====================================================================
            // Syntax Highlighting
            // =====================================================================
            Cmd::DebouncedSyntaxParse {
                document_id,
                revision,
                delay_ms,
            } => {
                self.syntax_scheduled
                    .retain(|(scheduled_document, _), _| *scheduled_document != document_id);
                self.syntax_scheduled
                    .insert((document_id, revision), Instant::now());
                tracing::debug!(
                    "DebouncedSyntaxParse: doc={} rev={} delay={}ms",
                    document_id.0,
                    revision,
                    delay_ms
                );
                let deadline = if delay_ms > 0 {
                    Instant::now() + Duration::from_millis(delay_ms)
                } else {
                    Instant::now() // Immediate
                };
                self.syntax_deadlines
                    .insert(document_id, (deadline, revision));
            }

            Cmd::RunSyntaxParse {
                document_id,
                revision,
                source,
                language,
                snapshot_ms,
            } => {
                tracing::debug!(
                    "RunSyntaxParse: doc={} rev={} lang={:?} len={}",
                    document_id.0,
                    revision,
                    language,
                    source.len()
                );
                let syntax_tx = self.syntax_tx.clone();
                if let Err(e) = syntax_tx.send(SyntaxWorkerRequest::Parse(SyntaxParseRequest {
                    document_id,
                    revision,
                    source,
                    language,
                    snapshot_ms,
                    queued_at: Instant::now(),
                    extract_outline: is_outline_panel_open(&self.model)
                        && self.model.document().id == Some(document_id),
                })) {
                    tracing::warn!("Failed to send syntax parse request: {}", e);
                }
            }

            Cmd::ClearSyntaxState { document_id } => {
                tracing::debug!("ClearSyntaxState: doc={}", document_id.0);
                self.syntax_deadlines.remove(&document_id);
                if let Err(e) = self
                    .syntax_tx
                    .send(SyntaxWorkerRequest::ClearDocument(document_id))
                {
                    tracing::warn!("Failed to send syntax clear request: {}", e);
                }
            }

            // =====================================================================
            // Language Server Commands
            // =====================================================================
            Cmd::LspEnsureServer {
                language,
                file_path,
            } => {
                self.ensure_lsp_server(language, &file_path);
            }
            Cmd::LspRestartServer { server_id } => {
                // Manual restart resets backoff: a user/automation-driven
                // restart is a deliberate retry, not another crash.
                self.lsp
                    .restart_attempts
                    .retain(|(id, _), _| *id != server_id);
                self.lsp
                    .restart_deadlines
                    .retain(|(id, _), _| *id != server_id);
                // Clear the missing-server memo too — a manual restart is
                // the user's explicit "try again" (e.g. after installing
                // the binary), so `ensure_lsp_server` must be allowed to
                // re-attempt the spawn instead of skipping it forever.
                self.lsp.missing_servers.retain(|(id, _)| *id != server_id);
                self.restart_lsp_server(&server_id);
            }
            Cmd::LspDidOpen {
                document_id,
                file_path,
                language,
            } => {
                self.lsp_open_document(document_id, file_path, language);
            }
            Cmd::LspScheduleDidChange {
                document_id,
                revision,
            } => {
                // No-op unless a server actually has this document open —
                // otherwise every keystroke in a zero-server session, or
                // in a plaintext/markdown/untitled buffer, arms a 30ms
                // deadline that will only ever no-op when it fires.
                if !self.lsp.open_documents.contains_key(&document_id) {
                    return;
                }
                self.lsp_change_deadlines.record_edit(
                    document_id,
                    revision,
                    Instant::now(),
                    Duration::from_millis(lsp::sync::DID_CHANGE_DEBOUNCE_MS),
                    Duration::from_millis(lsp::sync::DID_CHANGE_MAX_WAIT_MS),
                );
            }
            Cmd::LspDidSave { document_id } => {
                self.flush_lsp_did_change(document_id);
                self.lsp_save_document(document_id);
            }
            Cmd::LspDidClose { document_id } => {
                self.lsp_close_document(document_id);
            }
            Cmd::LspClearDiagnostics { document_id } => {
                if let Some(file_path) = self
                    .model
                    .editor_area
                    .documents
                    .get(&document_id)
                    .and_then(|doc| doc.file_path.clone())
                {
                    let uri = lsp::path_to_uri(&file_path);
                    self.lsp.diagnostics.remove(&uri);
                    self.lsp.diagnostics_versions.remove(&uri);
                    // The mirror is keyed by `uri_to_path(published_uri)`
                    // (`DiagnosticsPublished`'s insertion point), which can
                    // differ from `file_path` once a server canonicalizes
                    // (e.g. /tmp -> /private/tmp) — round-trip through the
                    // same URI this removal already built, not `file_path`
                    // directly, or the mirror row survives.
                    if let Some(mirror_path) = lsp::uri_to_path(&uri) {
                        self.model.lsp.diagnostics.remove(&mirror_path);
                    }
                    token::update::problems::clamp_problems_selection(&mut self.model);
                    // Same reasoning as `clear_diagnostics_for_roots`: the
                    // Problems panel has no dedicated damage area, so a
                    // clear while it's open needs a full repaint or the
                    // stale row stays painted until an unrelated event
                    // damages the window.
                    let problems_panel_open = self
                        .model
                        .dock_layout
                        .active_panel_position(token::panel::PanelId::PROBLEMS)
                        .is_some();
                    if problems_panel_open {
                        self.pending_damage.merge(Damage::Full);
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                    }
                }
            }
            Cmd::LspRequestDefinition {
                document_id,
                position,
                revision,
                origin,
            } => {
                self.request_lsp_definition(document_id, position, revision, origin);
            }
            Cmd::LspRequestHover {
                document_id,
                position,
                cursor,
                revision,
            } => {
                self.request_lsp_hover(document_id, position, cursor, revision);
            }
            Cmd::LspRequestSignatureHelp {
                document_id,
                position,
                cursor,
                revision,
                trigger,
                is_retrigger,
            } => {
                self.request_lsp_signature_help(
                    document_id,
                    position,
                    cursor,
                    revision,
                    trigger,
                    is_retrigger,
                );
            }
            Cmd::LspRequestPrepareRename {
                document_id,
                position,
                cursor,
                revision,
                fallback,
            } => {
                self.request_lsp_prepare_rename(document_id, position, cursor, revision, fallback);
            }
            Cmd::LspRequestRename {
                document_id,
                position,
                revision,
                new_name,
            } => {
                self.gated_lsp_request::<PendingRename>(
                    document_id,
                    Some(position),
                    Some(serde_json::json!({ "newName": new_name })),
                    |_| PendingRename {
                        document_id,
                        revision,
                    },
                );
            }
            Cmd::LspRequestFormatting {
                document_id,
                revision,
                range,
                options,
                then_save,
            } => {
                self.request_lsp_formatting(document_id, revision, range, options, then_save);
            }
            Cmd::LspRequestReferences {
                document_id,
                position,
                cursor,
                revision,
            } => {
                self.request_lsp_references(document_id, position, cursor, revision);
            }
            Cmd::LspRequestCodeActions {
                document_id,
                position,
                range,
                cursor,
                revision,
                diagnostics,
            } => {
                self.gated_lsp_request::<PendingCodeActions>(
                    document_id,
                    Some(position),
                    Some(serde_json::json!({
                        "range": range,
                        "context": { "diagnostics": diagnostics, "triggerKind": 1 },
                    })),
                    |_| PendingCodeActions {
                        document_id,
                        revision,
                        cursor,
                    },
                );
            }
            Cmd::LspExecuteCommand {
                document_id,
                command,
                arguments,
            } => {
                self.execute_lsp_command(document_id, command, arguments);
            }
            Cmd::LspScheduleCompletion {
                document_id,
                position,
                revision,
                trigger_character,
            } => {
                // Re-arming resets the deadline: a keystroke burst
                // coalesces into one request fired after the last char.
                self.lsp.completion_debounces.insert(
                    document_id,
                    ScheduledCompletion {
                        position,
                        revision,
                        trigger_character,
                        deadline: Instant::now() + COMPLETION_DEBOUNCE,
                    },
                );
            }
            Cmd::ScheduleInlineRequest {
                document_id,
                revision,
                delay_ms,
                explicit,
            } => {
                self.inline_deadlines.insert(
                    document_id,
                    (
                        Instant::now() + Duration::from_millis(delay_ms),
                        revision,
                        explicit,
                    ),
                );
            }
            Cmd::RunInlineRequest(request) => {
                if self.inline_tx.send(*request).is_err() {
                    tracing::warn!("inline suggestion worker is gone");
                    self.model.ui.inline_in_flight = false;
                }
            }
            Cmd::LspCancelCompletion { document_id } => {
                // Drop the pending debounce and supersede any in-flight
                // request (its late reply is consumed and discarded by the
                // interception pass; the menu it was for is gone).
                self.lsp.completion_debounces.remove(&document_id);
                self.lsp.resolve_debounces.remove(&document_id);
                if let Some(old_key) = self.lsp.completion.supersede(document_id) {
                    self.cancel_lsp_request(&old_key);
                }
            }
            Cmd::LspResolveCompletionItem {
                document_id,
                revision,
                server_id,
                root,
                raw_item,
                selected,
                purpose,
            } => {
                self.request_lsp_resolve(
                    document_id,
                    revision,
                    server_id,
                    root,
                    raw_item,
                    selected,
                    purpose,
                );
            }
            Cmd::LspScheduleResolve {
                document_id,
                revision,
                server_id,
                root,
                raw_item,
                selected,
            } => {
                self.lsp.resolve_debounces.insert(
                    document_id,
                    ScheduledResolve {
                        revision,
                        server_id,
                        root,
                        raw_item,
                        selected,
                        deadline: Instant::now() + RESOLVE_DEBOUNCE,
                    },
                );
            }
            Cmd::LspDidOpenOnServer {
                document_id,
                file_path,
                server_id,
                root,
            } => {
                let language = self
                    .model
                    .editor_area
                    .documents
                    .get(&document_id)
                    .map(|doc| doc.language);
                if let Some(language_id) = language.and_then(lsp::sync::language_id_str) {
                    self.lsp_open_document_on(document_id, file_path, server_id, root, language_id);
                }
            }
            Cmd::LspSetEnabled { enabled } => {
                if enabled {
                    self.lsp.missing_servers.clear();
                } else {
                    self.teardown_all_lsp_servers();
                }
            }
            Cmd::LspRespondToServer {
                server_id,
                root,
                request_id,
                result,
            } => {
                if let Some(handle) = self.lsp.servers.get(&(server_id, root)) {
                    let _ = handle
                        .outbound_tx
                        .send(lsp::client::WorkerCmd::ReplyToServer {
                            id: request_id,
                            result: Ok(result),
                        });
                }
            }
            Cmd::LspSetServerEnabled { server_id, enabled } => {
                if enabled {
                    self.lsp.missing_servers.retain(|(id, _)| *id != server_id);
                } else {
                    self.teardown_lsp_server(&server_id);
                }
            }

            // =====================================================================
            // Application Commands
            // =====================================================================
            Cmd::Quit => {
                // No quit-time teardown existed anywhere in the runtime
                // before this (not even for PTY children); this is the
                // first one. Runs the design doc's shutdown sequence per
                // server: `shutdown` request -> await response (2s) ->
                // `exit` notification -> await process exit (2s) -> kill.
                self.graceful_lsp_teardown();
                self.should_quit = true;
            }

            // =====================================================================
            // Debug Commands
            // =====================================================================
            #[cfg(debug_assertions)]
            Cmd::TogglePerfOverlay => {
                self.perf.show_overlay = !self.perf.show_overlay;
            }
        }
    }

    fn process_async_messages(&mut self) -> bool {
        let mut needs_redraw = self.process_terminal_spawn_results();
        let mut messages: Vec<Msg> = Vec::new();
        while let Ok(msg) = self.msg_rx.try_recv() {
            messages.push(msg);
        }
        messages = self.intercept_definition_replies(messages);
        messages = self.intercept_hover_replies(messages);
        messages = self.intercept_signature_help_replies(messages);
        messages = self.intercept_rename_replies(messages);
        messages = self.intercept_formatting_replies(messages);
        messages = self.intercept_references_replies(messages);
        messages = self.intercept_code_action_replies(messages);
        messages = self.intercept_completion_replies(messages);
        messages = self.intercept_resolve_replies(messages);
        // Coalesce successive `publishDiagnostics` for the same URI within
        // this drain, newest wins — each publish is a full replacement, so
        // dropping the superseded ones here converges on the same end
        // state as processing every one, but without the redundant
        // store-insert/`find_document_by_uri`/projection/redraw work a
        // flood of publishes for one URI would otherwise cost
        // (lsp-integration.md's "successive publishes ... coalesce before
        // drain"). "Newest" is by version order (mirroring
        // `is_stale_diagnostics_publish`), not literal batch position —
        // a server can (and the fake-server harness deliberately does,
        // to test staleness) enqueue an older-versioned publish after a
        // newer one within the same drain.
        // `lsp_types::Uri` (fluent-uri) trips `mutable_key_type`: it caches
        // parsed auth data in a `Cell` the way `Uri`'s own `Hash`/`Eq`
        // never reads, same false positive the existing `diagnostics`/
        // `diagnostics_versions` fields on this struct are exempt from
        // only because clippy doesn't lint struct fields.
        #[allow(clippy::mutable_key_type)]
        let mut running_version: std::collections::HashMap<lsp_types::Uri, i64> =
            std::collections::HashMap::new();
        #[allow(clippy::mutable_key_type)]
        let mut winner_idx: std::collections::HashMap<lsp_types::Uri, usize> =
            std::collections::HashMap::new();
        for (idx, msg) in messages.iter().enumerate() {
            if let Msg::Lsp(LspMsg::DiagnosticsPublished { uri, version, .. }) = msg {
                let baseline = running_version
                    .get(uri)
                    .copied()
                    .or_else(|| self.lsp.diagnostics_versions.get(uri).copied());
                let stale = matches!((version, baseline), (Some(v), Some(last)) if *v < last);
                if stale {
                    continue;
                }
                if let Some(v) = version {
                    running_version.insert(uri.clone(), *v);
                }
                winner_idx.insert(uri.clone(), idx);
            }
        }
        let mut idx = 0usize;
        messages.retain(|msg| {
            let this_idx = idx;
            idx += 1;
            match msg {
                Msg::Lsp(LspMsg::DiagnosticsPublished { uri, .. }) => {
                    winner_idx.get(uri) == Some(&this_idx)
                }
                _ => true,
            }
        });
        for msg in messages {
            if let Msg::Lsp(LspMsg::DiagnosticsPublished {
                ref uri,
                version,
                ref diagnostics,
            }) = msg
            {
                if self.is_stale_diagnostics_publish(uri, version) {
                    // Out-of-order publish for a URI we've already seen a
                    // newer version of — dropped before it touches the
                    // authoritative store or the model (design doc's
                    // diagnostics-version-ordering rule).
                    continue;
                }
                self.lsp
                    .diagnostics
                    .insert(uri.clone(), diagnostics.clone());
                if let Some(version) = version {
                    self.lsp.diagnostics_versions.insert(uri.clone(), version);
                }
            }
            let syntax_completion = match &msg {
                Msg::Syntax(SyntaxMsg::ParseCompleted {
                    document_id,
                    revision,
                    timing,
                    ..
                }) => Some((*document_id, *revision, **timing, Instant::now())),
                _ => None,
            };
            let lsp_exited = match &msg {
                Msg::Lsp(LspMsg::ServerExited {
                    server_id,
                    generation,
                }) => Some((server_id.clone(), *generation)),
                _ => None,
            };
            // A root that reaches `Ready` has proven it isn't crash-looping;
            // clear its restart-attempt count so a later, unrelated crash
            // doesn't inherit a stale streak (see `restart_attempts`' doc
            // comment).
            let lsp_ready = match &msg {
                Msg::Lsp(LspMsg::ServerStateChanged {
                    server_id,
                    root,
                    state: ServerState::Ready,
                }) => Some((server_id.clone(), root.clone())),
                _ => None,
            };
            // Log syntax-related messages for debugging
            if let Msg::Syntax(ref syntax_msg) = msg {
                tracing::debug!("Received async syntax message: {:?}", syntax_msg);
            }

            if let Some(cmd) = update(&mut self.model, msg) {
                if cmd.needs_redraw() {
                    needs_redraw = true;
                }
                // Accumulate damage from async message
                self.pending_damage.merge(cmd.damage());
                self.process_cmd(cmd);
            }
            if let Some((server_id, generation)) = lsp_exited {
                self.handle_lsp_server_exited(&server_id, generation);
            }
            if let Some((server_id, root)) = lsp_ready {
                self.lsp
                    .restart_attempts
                    .remove(&(server_id.clone(), root.clone()));
                if self
                    .lsp
                    .resync_pending
                    .remove(&(server_id.clone(), root.clone()))
                {
                    // A fresh process has no memory of documents opened
                    // against the one that crashed/was restarted —
                    // re-`didOpen` them (design doc's "after any restart"
                    // rule).
                    self.resync_open_documents(&server_id, &root);
                }
            }
            if let Some((document_id, revision, timing, apply_started)) = syntax_completion {
                let applied = self
                    .model
                    .editor_area
                    .documents
                    .get(&document_id)
                    .and_then(|document| document.syntax_highlights.as_ref())
                    .is_some_and(|highlights| highlights.revision == revision);
                if !applied {
                    continue;
                }
                let started_at = self
                    .syntax_scheduled
                    .remove(&(document_id, revision))
                    .unwrap_or(apply_started);
                let response_tx = self
                    .automation_syntax_profile
                    .take_if(|profile| {
                        profile.document_id == document_id && profile.revision == revision
                    })
                    .map(|profile| profile.response_tx);
                self.syntax_present_pending.push(SyntaxPresentationPending {
                    snapshot: crate::automation::SyntaxPerfSnapshot {
                        revision,
                        snapshot_ms: timing.snapshot_ms,
                        queue_ms: timing.queue_ms,
                        parse_highlight_ms: timing.parse_highlight_ms,
                        parse_ms: timing.parse_ms,
                        highlight_ms: timing.highlight_ms,
                        outline_ms: timing.outline_ms,
                        worker_total_ms: timing.worker_total_ms,
                        outline_extracted: timing.outline_extracted,
                        highlighted_line_count: timing.highlighted_line_count,
                        replaced_range_count: timing.replaced_range_count,
                        apply_ms: apply_started.elapsed().as_secs_f64() * 1000.0,
                        edit_to_present_ms: 0.0,
                    },
                    started_at,
                    response_tx,
                });
            }
        }
        needs_redraw
    }

    // Translate raw `textDocument/definition` worker replies into
    // `LspMsg::DefinitionResolved` before anything else sees them:
    // only `LspManager::definition_requests` (runtime-only state) has
    // the `(document_id, revision, origin)` context the response
    // needs, so this can't wait for `update()`. A superseded
    // request's late reply (`abandoned`) or an unknown id is dropped
    // here — "consumed and discarded" per the design doc.
    fn intercept_definition_replies(&mut self, messages: Vec<Msg>) -> Vec<Msg> {
        messages
            .into_iter()
            .filter_map(|msg| {
                let Msg::Lsp(LspMsg::DefinitionResponseFromServer {
                    server_id,
                    root,
                    request_id,
                    locations,
                    abandoned,
                }) = msg
                else {
                    return Some(msg);
                };
                let key = (server_id, root, request_id);
                let pending = self.lsp.definition.take_response(&key)?;
                if abandoned {
                    return None;
                }
                let outcome = if !locations.is_empty() {
                    DefinitionOutcome::Locations {
                        locations,
                        resolving_server: pending.server_id,
                        resolving_root: pending.root,
                    }
                } else if self.is_lsp_indexing(&pending.server_id) {
                    // An empty reply while the server is still `Starting`/
                    // `Indexing` means it hasn't finished analyzing the
                    // workspace, not that the symbol doesn't exist (design
                    // doc lines 101/212: never "not found" before `Ready`).
                    DefinitionOutcome::StillIndexing
                } else {
                    DefinitionOutcome::NoResult
                };
                Some(Msg::Lsp(LspMsg::DefinitionResolved {
                    document_id: pending.document_id,
                    revision: pending.revision,
                    origin: pending.origin,
                    outcome,
                }))
            })
            .collect()
    }

    // Same interception for `textDocument/hover` replies, mirroring
    // the definition pass above.
    fn intercept_hover_replies(&mut self, messages: Vec<Msg>) -> Vec<Msg> {
        messages
            .into_iter()
            .filter_map(|msg| {
                let Msg::Lsp(LspMsg::HoverResponseFromServer {
                    server_id,
                    root,
                    request_id,
                    content,
                    abandoned,
                }) = msg
                else {
                    return Some(msg);
                };
                let key = (server_id, root, request_id);
                let pending = self.lsp.hover.take_response(&key)?;
                if abandoned {
                    return None;
                }
                // A null/empty hover reply while the server is still
                // `Starting`/`Indexing` means it hasn't finished analyzing
                // yet, not that there's genuinely nothing to show (design
                // doc lines 101/212) — mirrors the definition arm above.
                let outcome = if content.is_some() || !self.is_lsp_indexing(&key.0) {
                    HoverOutcome::Content(content)
                } else {
                    HoverOutcome::StillIndexing
                };
                Some(Msg::Lsp(LspMsg::HoverResolved {
                    document_id: pending.document_id,
                    revision: pending.revision,
                    cursor: pending.cursor,
                    outcome,
                }))
            })
            .collect()
    }

    // Same interception for `textDocument/signatureHelp` replies; the
    // flattening to `SignatureHelpState` happens here so `update()` stays
    // cheap.
    fn intercept_signature_help_replies(&mut self, messages: Vec<Msg>) -> Vec<Msg> {
        messages
            .into_iter()
            .filter_map(|msg| {
                let Msg::Lsp(LspMsg::SignatureHelpResponseFromServer {
                    server_id,
                    root,
                    request_id,
                    help,
                    abandoned,
                }) = msg
                else {
                    return Some(msg);
                };
                let pending = self
                    .lsp
                    .signature_help
                    .take_response(&(server_id, root, request_id))?;
                if abandoned {
                    return None;
                }
                Some(Msg::Lsp(LspMsg::SignatureHelpResolved {
                    document_id: pending.document_id,
                    revision: pending.revision,
                    cursor: pending.cursor,
                    help: help.and_then(|h| lsp::client::signature_help_state(&h)),
                }))
            })
            .collect()
    }

    // Same interception for `textDocument/prepareRename` and
    // `textDocument/rename` replies; a `Range` placeholder is read from the
    // document buffer here so `update()` stays cheap.
    fn intercept_rename_replies(&mut self, messages: Vec<Msg>) -> Vec<Msg> {
        messages
            .into_iter()
            .filter_map(|msg| match msg {
                Msg::Lsp(LspMsg::PrepareRenameResponseFromServer {
                    server_id,
                    root,
                    request_id,
                    response,
                    abandoned,
                }) => {
                    let pending = self
                        .lsp
                        .prepare_rename
                        .take_response(&(server_id, root, request_id))?;
                    if abandoned {
                        return None;
                    }
                    let placeholder = response.and_then(|response| match response {
                        lsp_types::PrepareRenameResponse::RangeWithPlaceholder {
                            placeholder,
                            ..
                        } => Some(placeholder),
                        lsp_types::PrepareRenameResponse::DefaultBehavior { .. } => {
                            Some(pending.fallback.clone())
                        }
                        lsp_types::PrepareRenameResponse::Range(range) => {
                            let doc = self.model.editor_area.documents.get(&pending.document_id)?;
                            let start = lsp::lsp_to_position(doc, range.start);
                            let end = lsp::lsp_to_position(doc, range.end);
                            Some(
                                doc.buffer
                                    .slice(
                                        doc.cursor_to_offset(start.line, start.column)
                                            ..doc.cursor_to_offset(end.line, end.column),
                                    )
                                    .to_string(),
                            )
                        }
                    });
                    Some(Msg::Lsp(LspMsg::PrepareRenameResolved {
                        document_id: pending.document_id,
                        revision: pending.revision,
                        cursor: pending.cursor,
                        placeholder,
                    }))
                }
                Msg::Lsp(LspMsg::RenameResponseFromServer {
                    server_id,
                    root,
                    request_id,
                    edit,
                    abandoned,
                }) => {
                    let pending = self
                        .lsp
                        .rename
                        .take_response(&(server_id, root, request_id))?;
                    if abandoned {
                        return None;
                    }
                    Some(Msg::Lsp(LspMsg::RenameResolved {
                        document_id: pending.document_id,
                        revision: pending.revision,
                        edit,
                    }))
                }
                other => Some(other),
            })
            .collect()
    }

    // Same interception for `textDocument/codeAction` replies (already
    // flattened by the reader).
    fn intercept_code_action_replies(&mut self, messages: Vec<Msg>) -> Vec<Msg> {
        messages
            .into_iter()
            .filter_map(|msg| {
                let Msg::Lsp(LspMsg::CodeActionsResponseFromServer {
                    server_id,
                    root,
                    request_id,
                    actions,
                    abandoned,
                }) = msg
                else {
                    return Some(msg);
                };
                let pending = self
                    .lsp
                    .code_actions
                    .take_response(&(server_id, root, request_id))?;
                if abandoned {
                    return None;
                }
                Some(pending.resolved(actions, ReferencesOutcome::Found))
            })
            .collect()
    }

    // Same interception for formatting replies.
    fn intercept_formatting_replies(&mut self, messages: Vec<Msg>) -> Vec<Msg> {
        messages
            .into_iter()
            .filter_map(|msg| {
                let Msg::Lsp(LspMsg::FormattingResponseFromServer {
                    server_id,
                    root,
                    request_id,
                    edits,
                    abandoned,
                }) = msg
                else {
                    return Some(msg);
                };
                let pending = self
                    .lsp
                    .formatting
                    .take_response(&(server_id, root, request_id))?;
                if abandoned {
                    return None;
                }
                Some(Msg::Lsp(LspMsg::FormattingResolved {
                    document_id: pending.document_id,
                    revision: pending.revision,
                    edits: Some(edits),
                    then_save: pending.then_save,
                }))
            })
            .collect()
    }

    // Same interception for `textDocument/references` replies. Unlike
    // definition/hover, this one does real work: previews may require
    // reading unopened files off disk (`build_reference_items`), which
    // `update()` must never do — so it happens here, before the
    // message reaches it.
    fn intercept_references_replies(&mut self, messages: Vec<Msg>) -> Vec<Msg> {
        messages
            .into_iter()
            .filter_map(|msg| {
                let Msg::Lsp(LspMsg::ReferencesResponseFromServer {
                    server_id,
                    root,
                    request_id,
                    locations,
                    abandoned,
                }) = msg
                else {
                    return Some(msg);
                };
                let key = (server_id, root, request_id);
                let pending = self.lsp.references.take_response(&key)?;
                if abandoned {
                    return None;
                }
                let items = self.build_reference_items(locations, &key.0, &key.1);
                let outcome = if !items.is_empty() {
                    ReferencesOutcome::Found
                } else if self.is_lsp_indexing(&key.0) {
                    ReferencesOutcome::StillIndexing
                } else {
                    ReferencesOutcome::NoResult
                };
                Some(Msg::Lsp(LspMsg::ReferencesResolved {
                    document_id: pending.document_id,
                    revision: pending.revision,
                    cursor: pending.cursor,
                    items,
                    outcome,
                }))
            })
            .collect()
    }

    // Same interception for `textDocument/completion` replies. The
    // conversion to menu items happens here (mirroring
    // `build_reference_items`'s "real work before update()" rule):
    // `can_resolve` comes from the responding server's capability
    // snapshot, which only the runtime can consult.
    fn intercept_completion_replies(&mut self, messages: Vec<Msg>) -> Vec<Msg> {
        messages
            .into_iter()
            .filter_map(|msg| {
                let Msg::Lsp(LspMsg::CompletionResponseFromServer {
                    server_id,
                    root,
                    request_id,
                    items,
                    is_incomplete,
                    abandoned,
                }) = msg
                else {
                    return Some(msg);
                };
                let key = (server_id, root, request_id);
                let pending = self.lsp.completion.take_response(&key)?;
                if abandoned {
                    return None;
                }
                // An empty reply while the server is still indexing is
                // forwarded as-is (empty): the menu keeps its offline
                // items and the next keystroke re-requests — there is no
                // "still indexing" transient for completion.
                let can_resolve = self
                    .lsp
                    .servers
                    .get(&(key.0.clone(), key.1.clone()))
                    .and_then(|handle| handle.capabilities_snapshot())
                    .is_some_and(|caps| lsp::client::supports_completion_resolve(&caps));
                let menu_items =
                    token::completion::lsp::items_to_menu_items(items, &key.0, &key.1, can_resolve);
                Some(Msg::Lsp(LspMsg::CompletionResolved {
                    document_id: pending.document_id,
                    revision: pending.revision,
                    items: menu_items,
                    is_incomplete,
                }))
            })
            .collect()
    }

    // Same interception for `completionItem/resolve` replies: fold the
    // resolved item's accept-relevant fields into
    // `CompletionItemResolved`. A null/unparseable result forwards
    // empty extras — the deferred accept applies with what was known.
    fn intercept_resolve_replies(&mut self, messages: Vec<Msg>) -> Vec<Msg> {
        messages
            .into_iter()
            .filter_map(|msg| {
                let Msg::Lsp(LspMsg::ResolveResponseFromServer {
                    server_id,
                    root,
                    request_id,
                    item,
                    abandoned,
                }) = msg
                else {
                    return Some(msg);
                };
                let key = (server_id, root, request_id);
                let pending = self.lsp.resolve.take_response(&key)?;
                if abandoned {
                    return None;
                }
                let additional_text_edits = item
                    .as_ref()
                    .and_then(|resolved| resolved.additional_text_edits.as_ref())
                    .map(|edits| {
                        edits
                            .iter()
                            .map(|edit| (edit.range, edit.new_text.clone()))
                            .collect()
                    })
                    .unwrap_or_default();
                let documentation = item.as_ref().and_then(|resolved| {
                    resolved
                        .documentation
                        .as_ref()
                        .and_then(token::completion::lsp::documentation_to_styled)
                });
                let detail = item.and_then(|resolved| resolved.detail);
                Some(Msg::Lsp(LspMsg::CompletionItemResolved {
                    document_id: pending.document_id,
                    revision: pending.revision,
                    selected: pending.selected,
                    detail,
                    documentation,
                    additional_text_edits,
                }))
            })
            .collect()
    }

    /// Spawns a server for `language` rooted for `file_path`, if one is
    /// registered, enabled, and not already running for that root
    /// (`Cmd::LspEnsureServer`; see design doc's Process Model — lazy
    /// spawn on first matching `didOpen`, wired by the next unit).
    fn ensure_lsp_server(&mut self, language: token::syntax::LanguageId, file_path: &Path) {
        let Some((resolved, root)) = self.resolved_server_and_root(language, file_path) else {
            return;
        };

        if self.lsp.is_running(&resolved.id, &root) {
            return;
        }
        // Already know this `(server_id, root)` has no binary — skip the
        // re-attempt entirely (see `LspManager::missing_servers`'s doc
        // comment). Without this a matching file-open re-flashes the
        // transient and re-consumes a detached-root slot every time.
        if self
            .lsp
            .missing_servers
            .contains(&(resolved.id.clone(), root.clone()))
        {
            return;
        }

        let workspace_root = self.model.workspace.as_ref().map(|w| w.root.as_path());
        let is_detached = workspace_root.is_none_or(|ws| !root.starts_with(ws));
        let already_detached = self.lsp.detached_roots.contains(&root);
        if is_detached && !already_detached && self.lsp.detached_roots.len() >= MAX_DETACHED_ROOTS {
            tracing::debug!(
                "Detached LSP root cap ({MAX_DETACHED_ROOTS}) reached; not spawning {} for {}",
                resolved.id,
                root.display()
            );
            return;
        }

        // The slot is only spent once the spawn actually succeeds — a
        // failed spawn (`Missing`) must not permanently claim one of the
        // limited detached-root slots (see `LspManager::missing_servers`).
        if self.spawn_lsp_server_at(&resolved, &root) && is_detached && !already_detached {
            self.lsp.detached_roots.push(root);
        }
    }

    /// Kills every running instance of `server_id` and respawns it at
    /// the same root(s) — used both for manual restart and, capped at
    /// `MAX_RESTART_ATTEMPTS`, crash backoff.
    fn restart_lsp_server(&mut self, server_id: &LspServerId) {
        let Some(def) = lsp::server_def_by_id(&server_id.0) else {
            return;
        };
        let Some(resolved) = lsp::resolve_server(def, &self.model.config.lsp) else {
            return;
        };
        // `roots_for` only sees roots with a live handle — a `Failed`
        // server has none (`handle_lsp_server_exited` removes it before
        // reporting `Failed`), so fall back to the roots remembered at
        // the point it gave up. Without this, `RestartLanguageServer`
        // silently does nothing for exactly the state it exists to fix.
        let mut roots = self.lsp.roots_for(server_id);
        if roots.is_empty() {
            roots = self.lsp.failed_roots.remove(server_id).unwrap_or_default();
        }
        self.clear_diagnostics_for_roots(server_id, &roots);
        self.clear_pending_requests_for_roots(server_id, &roots);
        for root in roots {
            if let Some(mut handle) = self.lsp.servers.remove(&(server_id.clone(), root.clone())) {
                handle.kill();
            }
            self.lsp
                .resync_pending
                .insert((server_id.clone(), root.clone()));
            self.spawn_lsp_server_at(&resolved, &root);
        }
    }

    /// Kills every running instance of `server_id` and clears its
    /// diagnostics, without respawning — `Cmd::LspSetServerEnabled { enabled:
    /// false }` (Language Servers picker). Mirrors `restart_lsp_server`
    /// minus the respawn half.
    fn teardown_lsp_server(&mut self, server_id: &LspServerId) {
        let roots = self.lsp.roots_for(server_id);
        self.clear_diagnostics_for_roots(server_id, &roots);
        self.clear_pending_requests_for_roots(server_id, &roots);
        for root in roots {
            self.set_lsp_server_state(server_id.clone(), &root, ServerState::ShuttingDown);
            if let Some(mut handle) = self.lsp.servers.remove(&(server_id.clone(), root)) {
                handle.kill();
            }
        }
        // A disabled server that previously gave up shouldn't keep
        // `RestartLanguageServer` pointed at roots it's not allowed to run
        // at anymore.
        self.lsp.failed_roots.remove(server_id);
    }

    /// Tears down every running language server without quitting the app —
    /// `Cmd::LspSetEnabled { enabled: false }` (`CommandId::ToggleLsp`
    /// disabling the master switch). Mirrors `graceful_lsp_teardown`'s
    /// shutdown sequence and shared grace budget, but restores
    /// `lsp.shutting_down` afterward instead of leaving quit's teardown
    /// flag set, and clears every torn-down server's diagnostics — a
    /// disabled server has no process left to ever refresh them.
    fn teardown_all_lsp_servers(&mut self) {
        let was_shutting_down = self.lsp.shutting_down;
        self.lsp.shutting_down = true;
        let keys: Vec<(LspServerId, PathBuf)> = self.lsp.servers.keys().cloned().collect();
        let shared_deadline = std::time::Instant::now() + Duration::from_secs(4);
        for (server_id, root) in &keys {
            self.set_lsp_server_state(server_id.clone(), root, ServerState::ShuttingDown);
            if let Some(mut handle) = self.lsp.servers.remove(&(server_id.clone(), root.clone())) {
                if handle.capabilities_snapshot().is_some() {
                    handle.graceful_shutdown(&self.msg_rx, Duration::from_secs(2), shared_deadline);
                } else {
                    handle.kill();
                }
            }
        }
        self.lsp.shutting_down = was_shutting_down;

        let mut roots_by_server: HashMap<LspServerId, Vec<PathBuf>> = HashMap::new();
        for (server_id, root) in keys {
            roots_by_server.entry(server_id).or_default().push(root);
        }
        for (server_id, roots) in roots_by_server {
            self.clear_diagnostics_for_roots(&server_id, &roots);
            self.clear_pending_requests_for_roots(&server_id, &roots);
            self.lsp.failed_roots.remove(&server_id);
        }
    }

    /// Drops definition/hover request bookkeeping for `(server_id, root)`
    /// pairs in `roots` — shared by `handle_lsp_server_exited` (the old
    /// worker will never answer) and `restart_lsp_server` (same reasoning,
    /// plus: a fresh `ServerHandle` restarts request-id allocation at 1,
    /// so a stale entry left behind can collide with a new request's id
    /// and cause `check_lsp_definition_deadlines`/hover's deadline sweep
    /// to abandon a live request that happens to reuse the old id).
    fn clear_pending_requests_for_roots(&mut self, server_id: &LspServerId, roots: &[PathBuf]) {
        self.lsp.definition.clear_for_roots(server_id, roots);
        self.lsp.hover.clear_for_roots(server_id, roots);
        self.lsp.references.clear_for_roots(server_id, roots);
        self.lsp.code_actions.clear_for_roots(server_id, roots);
        self.lsp.completion.clear_for_roots(server_id, roots);
        self.lsp.signature_help.clear_for_roots(server_id, roots);
        self.lsp.prepare_rename.clear_for_roots(server_id, roots);
        self.lsp.rename.clear_for_roots(server_id, roots);
        self.lsp.formatting.clear_for_roots(server_id, roots);
        self.lsp.resolve.clear_for_roots(server_id, roots);
    }

    /// Advisory `$/cancelRequest` + local abandonment for a superseded or
    /// timed-out feature request — the shared half of the four identical
    /// blocks `request_lsp_definition`/`request_lsp_hover`/the deadline
    /// sweeps used to hand-roll. A no-op if the server has since exited.
    fn cancel_lsp_request(&self, key: &RequestKey) {
        if let Some(handle) = self.lsp.servers.get(&(key.0.clone(), key.1.clone())) {
            handle.pending.lock().unwrap().abandon(key.2);
            let _ = handle.outbound_tx.send(lsp::client::WorkerCmd::Notify {
                method: "$/cancelRequest".to_owned(),
                params: serde_json::json!({ "id": key.2 }),
            });
        }
    }

    /// Returns whether the spawn succeeded — callers that reserve a
    /// limited resource on the strength of "a server is now running here"
    /// (the detached-roots cap) must only do so after this returns `true`.
    fn spawn_lsp_server_at(&mut self, resolved: &lsp::ResolvedServer, root: &Path) -> bool {
        // Funnel guard against the backoff-window duplicate-spawn race: a
        // crash removes the dead handle and arms a backoff deadline; a
        // file-open during that window can reach here via
        // `ensure_lsp_server` (which checked `is_running` before the
        // backoff fired) and then `check_lsp_restart_deadlines` fires the
        // same respawn again. `restart_lsp_server` always removes the old
        // handle first, so a live server is never mistaken for stale here.
        if self.lsp.is_running(&resolved.id, root) {
            return true;
        }
        // A restart (manual or crash-backoff) re-attempts a spawn for a
        // root previously memoized as missing — e.g. the user installed
        // the binary and ran `RestartLanguageServer`, which clears the
        // memo. Stale entries left behind would otherwise wedge
        // `ensure_lsp_server` even after a successful respawn here.
        self.lsp
            .missing_servers
            .remove(&(resolved.id.clone(), root.to_path_buf()));
        self.set_lsp_server_state(resolved.id.clone(), root, ServerState::Starting);
        match lsp::client::spawn_server(
            &resolved.command,
            &resolved.args,
            root,
            resolved.id.clone(),
            self.msg_tx.clone(),
            self.lsp_wake.clone(),
            resolved.initialization_options.clone(),
            resolved.settings.clone(),
        ) {
            Ok(handle) => {
                self.lsp
                    .servers
                    .insert((resolved.id.clone(), root.to_path_buf()), handle);
                true
            }
            Err(e) => {
                tracing::warn!("Failed to spawn LSP server {}: {}", resolved.id, e);
                self.set_lsp_server_state(resolved.id.clone(), root, ServerState::Missing);
                self.lsp
                    .missing_servers
                    .insert((resolved.id.clone(), root.to_path_buf()));
                false
            }
        }
    }

    /// Resolves which server (config-overridden) and root a file's
    /// language maps to, or `None` if no server is registered/enabled
    /// for it. Shared by `ensure_lsp_server` and `lsp_open_document` so
    /// they never disagree about which `(server_id, root)` a document
    /// belongs to.
    fn resolved_server_and_root(
        &self,
        language: token::syntax::LanguageId,
        file_path: &Path,
    ) -> Option<(lsp::ResolvedServer, PathBuf)> {
        let def = lsp::lsp_server_def(language)?;
        let resolved = lsp::resolve_server(def, &self.model.config.lsp)?;
        let workspace_root = self.model.workspace.as_ref().map(|w| w.root.as_path());
        let root = lsp::client::resolve_root(file_path, workspace_root, def.project_markers, |p| {
            p.is_file()
        });
        Some((resolved, root))
    }

    /// `textDocument/didOpen` — sends the document's current full text
    /// if a server is registered/ready for it; otherwise a silent no-op
    /// (no server, or `ensure_lsp_server` hasn't produced a handle yet —
    /// the notification is dropped since nothing sent it will ever ask
    /// again; a later edit's `didChange` would target a document the
    /// server never opened, so `lsp_change_deadlines`/`send_lsp_did_change`
    /// guard on `open_documents` containing this id).
    fn lsp_open_document(
        &mut self,
        document_id: token::model::editor_area::DocumentId,
        file_path: PathBuf,
        language: token::syntax::LanguageId,
    ) {
        let Some((resolved, root)) = self.resolved_server_and_root(language, &file_path) else {
            return;
        };
        let Some(language_id) = lsp::sync::language_id_str(language) else {
            return;
        };
        self.lsp_open_document_on(document_id, file_path, resolved.id, root, language_id);
    }

    /// The actual `didOpen` send + `open_documents` bookkeeping, against
    /// an already-known `(server_id, root)` — shared by `lsp_open_document`
    /// (which resolves the pair from `language`/`file_path`) and
    /// `Cmd::LspDidOpenOnServer` (which is handed the pair directly, for a
    /// definition-jump target outside every root).
    fn lsp_open_document_on(
        &mut self,
        document_id: token::model::editor_area::DocumentId,
        file_path: PathBuf,
        server_id: LspServerId,
        root: PathBuf,
        language_id: &'static str,
    ) {
        let Some(handle) = self.lsp.servers.get(&(server_id.clone(), root.clone())) else {
            return;
        };
        // Gate on `textDocumentSync` once capabilities are known — the
        // design doc's "absent: no sync messages at all". Every other
        // sync send (`didChange`/`didClose`/`didSave`) is a no-op when
        // `document_id` isn't in `open_documents`, so gating here (the
        // one place that inserts into it) is enough to keep the rest of
        // the pipeline honest without repeating the check at every send.
        // Unknown capabilities (handshake still in flight) fall through —
        // the outbound `didOpen` still queues correctly behind the
        // transport's own handshake gate.
        if let Some(caps) = handle.capabilities_snapshot() {
            if lsp::client::sync_mode(&caps) == lsp::client::SyncMode::None {
                return;
            }
        }
        let Some(doc) = self.model.editor_area.documents.get(&document_id) else {
            return;
        };
        let text = doc.buffer.to_string();
        let revision = doc.revision;
        let uri = lsp::path_to_uri(&file_path);

        let params = serde_json::json!({
            "textDocument": {
                "uri": uri.as_str(),
                "languageId": language_id,
                "version": revision as i64,
                "text": text,
            }
        });
        let _ = handle.outbound_tx.send(lsp::client::WorkerCmd::Notify {
            method: "textDocument/didOpen".to_owned(),
            params,
        });
        // Pull any diagnostics the store already retained for this URI
        // (a publish that arrived before this document was open) — the
        // design doc's "retains publishes for unopened files" rule.
        if let Some(diagnostics) = self.lsp.diagnostics.get(&uri) {
            let has_marks = !diagnostics.is_empty();
            if let Some(doc) = self.model.editor_area.documents.get_mut(&document_id) {
                doc.diagnostics = diagnostics.clone();
            }
            // Marks-lane activation changes gutter width — see
            // `AppModel::resync_viewports`'s doc comment.
            if has_marks {
                self.model.resync_viewports();
            }
        }
        self.lsp.open_documents.insert(
            document_id,
            OpenDocState {
                server_id,
                root,
                uri,
                synced_revision: revision,
            },
        );
    }

    /// Whether a `publishDiagnostics` `version` for `uri` is older than
    /// the last one applied — the design doc's "version used only to
    /// discard out-of-order publishes" rule. A publish with no version
    /// (or the first ever seen for `uri`) is never stale.
    fn is_stale_diagnostics_publish(&self, uri: &lsp_types::Uri, version: Option<i64>) -> bool {
        match (version, self.lsp.diagnostics_versions.get(uri)) {
            (Some(incoming), Some(&last)) => incoming < last,
            _ => false,
        }
    }

    /// Drops the diagnostics store entry and any open document's
    /// projection for every URI under `(server_id, root)` in `roots` —
    /// open or not — called on crash-exit and on a manual restart, since
    /// diagnostics from a server that's gone (or about to be replaced)
    /// are stale (design doc's "cleared on ... server exit"). Retained
    /// entries for unopened files must be swept too, or a crashed
    /// server's stale publish survives to be pulled by a later `didOpen`.
    fn clear_diagnostics_for_roots(&mut self, server_id: &LspServerId, roots: &[PathBuf]) {
        let affected_docs: Vec<_> = self
            .lsp
            .open_documents
            .iter()
            .filter(|(_, state)| &state.server_id == server_id && roots.contains(&state.root))
            .map(|(doc_id, state)| (*doc_id, state.uri.clone()))
            .collect();
        // See the `mutable_key_type` note in `process_async_messages` —
        // same `lsp_types::Uri` false positive.
        #[allow(clippy::mutable_key_type)]
        let mut stale_uris: std::collections::HashSet<_> =
            affected_docs.iter().map(|(_, uri)| uri.clone()).collect();
        stale_uris.extend(self.lsp.diagnostics.keys().filter_map(|uri| {
            let path = lsp::uri_to_path(uri)?;
            roots
                .iter()
                .any(|root| path.starts_with(root))
                .then(|| uri.clone())
        }));
        for uri in &stale_uris {
            self.lsp.diagnostics.remove(uri);
            self.lsp.diagnostics_versions.remove(uri);
        }
        for uri in &stale_uris {
            if let Some(path) = lsp::uri_to_path(uri) {
                self.model.lsp.diagnostics.remove(&path);
            }
        }
        // Bypasses `update()`, so the Problems panel's selection needs the
        // same clamp `DiagnosticsPublished` gives it on the publish side —
        // a clear can shrink the mirror out from under a stored selection.
        token::update::problems::clamp_problems_selection(&mut self.model);
        let mut any_had_marks = false;
        for (doc_id, _) in affected_docs {
            if let Some(doc) = self.model.editor_area.documents.get_mut(&doc_id) {
                any_had_marks |= !doc.diagnostics.is_empty();
                doc.diagnostics.clear();
            }
        }
        // Marks-lane deactivation changes gutter width — see
        // `AppModel::resync_viewports`'s doc comment. The only damage a
        // caller of this helper otherwise merges is `redraw_status_bar`
        // (via `set_lsp_server_state`), which `Renderer::render` doesn't
        // treat as covering the editor area — without merging editor
        // damage directly here, cleared marks/underlines stay painted
        // until an unrelated event happens to damage the editor.
        // The Problems panel has no dedicated damage area (it isn't a text
        // buffer), so a clear while it's open needs a full repaint to drop
        // the stale rows — same reasoning as the editor-marks merge below.
        let problems_panel_open = self
            .model
            .dock_layout
            .active_panel_position(token::panel::PanelId::PROBLEMS)
            .is_some();
        if any_had_marks || problems_panel_open {
            self.model.resync_viewports();
            self.pending_damage.merge(if problems_panel_open {
                Damage::Full
            } else {
                Cmd::redraw_editor().damage()
            });
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }

    /// `textDocument/didChange`, full-text sync. `text` reuses an
    /// already-snapshotted buffer (shared with a coincident syntax parse
    /// — see `check_syntax_and_lsp_deadlines`) when given, otherwise
    /// snapshots the buffer itself. A no-op if `document_id` isn't
    /// currently open on a server, or already at `revision` (flushing a
    /// deadline that another flush already fired).
    fn send_lsp_did_change(
        &mut self,
        document_id: token::model::editor_area::DocumentId,
        revision: u64,
        text: Option<Arc<str>>,
    ) {
        let Some(state) = self.lsp.open_documents.get_mut(&document_id) else {
            return;
        };
        if state.synced_revision == revision {
            return;
        }
        let Some(handle) = self
            .lsp
            .servers
            .get(&(state.server_id.clone(), state.root.clone()))
        else {
            return;
        };
        let text: Arc<str> = match text {
            Some(text) => text,
            None => match self.model.editor_area.documents.get(&document_id) {
                Some(doc) => doc.buffer.to_string().into(),
                None => return,
            },
        };
        let params = serde_json::json!({
            "textDocument": { "uri": state.uri.as_str(), "version": revision as i64 },
            "contentChanges": [{ "text": text.as_ref() }],
        });
        let _ = handle.outbound_tx.send(lsp::client::WorkerCmd::Notify {
            method: "textDocument/didChange".to_owned(),
            params,
        });
        state.synced_revision = revision;
    }

    /// The flush-before-request invariant
    /// (docs/feature/lsp-integration.md "Document Synchronization"): if
    /// there's a debounced `didChange` still pending for `document_id`,
    /// send it now, ahead of whatever the caller sends next. A no-op if
    /// nothing is pending.
    fn flush_lsp_did_change(&mut self, document_id: token::model::editor_area::DocumentId) {
        if let Some(revision) = self.lsp_change_deadlines.take(document_id) {
            self.send_lsp_did_change(document_id, revision, None);
        }
    }

    /// `textDocument/didSave` — with text iff the server's capabilities
    /// asked for it (`save: { includeText: true }`).
    fn lsp_save_document(&mut self, document_id: token::model::editor_area::DocumentId) {
        let Some(state) = self.lsp.open_documents.get(&document_id) else {
            return;
        };
        let Some(handle) = self
            .lsp
            .servers
            .get(&(state.server_id.clone(), state.root.clone()))
        else {
            return;
        };
        let caps = handle.capabilities_snapshot().unwrap_or_default();
        if !lsp::client::wants_did_save(&caps) {
            return;
        }
        let mut params = serde_json::json!({ "textDocument": { "uri": state.uri.as_str() } });
        if lsp::client::save_includes_text(&caps) {
            if let Some(doc) = self.model.editor_area.documents.get(&document_id) {
                params["text"] = serde_json::json!(doc.buffer.to_string());
            }
        }
        let _ = handle.outbound_tx.send(lsp::client::WorkerCmd::Notify {
            method: "textDocument/didSave".to_owned(),
            params,
        });
    }

    /// `textDocument/didClose` — call only when the document is released
    /// (never on tab close alone). Drops any still-pending `didChange`
    /// for it without sending: the server is about to be told the
    /// document is gone, so a `didChange` for it afterward would be
    /// invalid.
    fn lsp_close_document(&mut self, document_id: token::model::editor_area::DocumentId) {
        self.lsp_change_deadlines.take(document_id);
        // A closed document's menu is gone; a pending completion debounce
        // or in-flight request for it would be answered into nothing.
        self.lsp.completion_debounces.remove(&document_id);
        self.lsp.resolve_debounces.remove(&document_id);
        let _ = self.lsp.completion.supersede(document_id);
        let _ = self.lsp.resolve.supersede(document_id);
        let Some(state) = self.lsp.open_documents.remove(&document_id) else {
            return;
        };
        let Some(handle) = self.lsp.servers.get(&(state.server_id, state.root)) else {
            return;
        };
        let params = serde_json::json!({ "textDocument": { "uri": state.uri.as_str() } });
        let _ = handle.outbound_tx.send(lsp::client::WorkerCmd::Notify {
            method: "textDocument/didClose".to_owned(),
            params,
        });
    }

    /// Re-sends `didOpen` for every document tracked as open against
    /// `(server_id, root)` — the design doc's "after any restart,
    /// `didOpen` is re-sent for every currently-open matching document"
    /// (a fresh process has no memory of documents opened against the
    /// one that crashed).
    ///
    /// Reuses each document's *stored* `(server_id, root, uri)`
    /// (`OpenDocState`) rather than re-resolving them from
    /// `language`/`file_path` — `lsp_open_document`'s generic resolution
    /// re-derives the root from the file's own project markers, which
    /// silently disagrees with a document that was opened via the
    /// out-of-root route hint (e.g. a registry file reached through a
    /// definition jump, tracked under the *resolving* server/root, not
    /// its own crate's). Re-resolving on resync would pick that crate's
    /// own root, find no handle there, and return without ever
    /// re-`didOpen`ing — leaving `didChange`/hover/definition silently
    /// targeting a server that never saw the document.
    fn resync_open_documents(&mut self, server_id: &LspServerId, root: &Path) {
        let document_ids: Vec<_> = self
            .lsp
            .open_documents
            .iter()
            .filter(|(_, state)| &state.server_id == server_id && state.root == root)
            .map(|(&doc_id, _)| doc_id)
            .collect();
        for document_id in document_ids {
            let Some(doc) = self.model.editor_area.documents.get(&document_id) else {
                continue;
            };
            let Some(file_path) = doc.file_path.clone() else {
                continue;
            };
            let Some(language_id) = lsp::sync::language_id_str(doc.language) else {
                continue;
            };
            self.lsp_open_document_on(
                document_id,
                file_path,
                server_id.clone(),
                root.to_path_buf(),
                language_id,
            );
        }
    }

    /// The shared gate/flush/send skeleton every LSP feature request
    /// (definition, hover, ...) goes through: `open_documents` lookup →
    /// handle lookup → capability check → flush-before-request → re-lookup
    /// the handle (flush can drop it) → send. Gates on the same
    /// information `didOpen` already established (`open_documents`)
    /// rather than re-resolving the server/root, so this and
    /// `lsp_open_document` never disagree about which server a document
    /// belongs to. Returns the request's key once it's on the wire; the
    /// caller still owns supersede/insert/deadline-arming since those are
    /// feature-specific (different pending-payload shapes).
    fn send_lsp_feature_request(
        &mut self,
        document_id: token::model::editor_area::DocumentId,
        method: &'static str,
        position: Option<lsp_types::Position>,
        supports: fn(&lsp_types::ServerCapabilities) -> bool,
        extra_params: Option<serde_json::Value>,
    ) -> Result<RequestKey, FeatureGateError> {
        let Some(state) = self.lsp.open_documents.get(&document_id) else {
            // No server registered/synced for this document at all —
            // untitled buffer, or a language with no registered server.
            return Err(FeatureGateError::NoServer);
        };
        let server_id = state.server_id.clone();
        let root = state.root.clone();
        let uri = state.uri.clone();

        let Some(handle) = self.lsp.servers.get(&(server_id.clone(), root.clone())) else {
            return Err(FeatureGateError::NotReady);
        };
        let Some(caps) = handle.capabilities_snapshot() else {
            // Handshake hasn't completed yet — indistinguishable from
            // "still indexing" from the user's point of view.
            return Err(FeatureGateError::NotReady);
        };
        if !supports(&caps) {
            return Err(FeatureGateError::Unsupported);
        }

        // Flush-before-request invariant: a request issued inside the
        // debounce window must not be answered against stale text.
        self.flush_lsp_did_change(document_id);

        let Some(handle) = self.lsp.servers.get(&(server_id.clone(), root.clone())) else {
            return Err(FeatureGateError::NotReady);
        };
        let mut params = serde_json::json!({
            "textDocument": { "uri": uri.as_str() },
        });
        if let Some(position) = position {
            params["position"] =
                serde_json::json!({ "line": position.line, "character": position.character });
        }
        if let Some(extra) = extra_params {
            if let (Some(dst), Some(src)) = (params.as_object_mut(), extra.as_object()) {
                dst.extend(src.clone());
            }
        }
        let request_id = handle.begin_request(method, params);
        Ok((server_id, root, request_id))
    }

    /// The shared supersede → gate → insert → arm skeleton of every
    /// position-based `request_lsp_*` (see `LspFeature`). Gate failures
    /// route through `F::on_gate_error` and are emitted synchronously via
    /// the same path the worker threads use, so status-bar damage is never
    /// skipped; a feature with no failure messaging (`None`) just returns.
    /// `build` receives the on-the-wire request's key so payloads that
    /// echo the resolving server/root back (definition's route hints) can
    /// capture it.
    fn gated_lsp_request<F: LspFeature + LspOutcomePolicy>(
        &mut self,
        document_id: token::model::editor_area::DocumentId,
        position: Option<lsp_types::Position>,
        extra_params: Option<serde_json::Value>,
        build: impl FnOnce(RequestKey) -> F,
    ) {
        self.gated_lsp_request_as::<F>(
            document_id,
            F::METHOD,
            F::supports,
            position,
            extra_params,
            build,
        )
    }

    /// `gated_lsp_request` with the method/capability gate supplied by the
    /// caller — for a feature whose slot serves two wire methods
    /// (formatting vs. rangeFormatting).
    fn gated_lsp_request_as<F: LspFeature + LspOutcomePolicy>(
        &mut self,
        document_id: token::model::editor_area::DocumentId,
        method: &'static str,
        supports: fn(&lsp_types::ServerCapabilities) -> bool,
        position: Option<lsp_types::Position>,
        extra_params: Option<serde_json::Value>,
        build: impl FnOnce(RequestKey) -> F,
    ) {
        if let Some(old_key) = F::slot(&mut self.lsp).supersede(document_id) {
            self.cancel_lsp_request(&old_key);
        }
        let key = match self.send_lsp_feature_request(
            document_id,
            method,
            position,
            supports,
            extra_params,
        ) {
            Ok(key) => key,
            Err(err) => {
                // Build from a zeroed key: the failure messages never read
                // server/root, only document context captured earlier.
                if let Some(msg) =
                    build((LspServerId::from(""), PathBuf::new(), 0)).on_gate_error(err)
                {
                    self.emit_lsp_msg(msg);
                }
                return;
            }
        };
        let pending = build(key.clone());
        F::slot(&mut self.lsp).insert(key.clone(), document_id, pending);
        F::slot(&mut self.lsp).arm_deadline(key);
    }

    /// The shared UI-level abandonment sweep for every feature slot —
    /// cancels each expired request server-side (`$/cancelRequest`) and
    /// emits its timeout outcome, if the feature has one. Superseded
    /// leftovers are silently cleaned up by `take_due`.
    fn sweep_lsp_feature_deadlines<F: LspFeature + LspOutcomePolicy>(&mut self) {
        let due = {
            let slot = F::slot(&mut self.lsp);
            if slot.is_empty_deadlines() {
                return;
            }
            slot.take_due(Instant::now())
        };
        for (key, pending) in due {
            self.cancel_lsp_request(&key);
            if let Some(msg) = pending.on_timeout() {
                self.emit_lsp_msg(msg);
            }
        }
    }

    /// Routes an LSP outcome message through `process_automation_msg` —
    /// the same path the worker threads use — instead of poking model
    /// state directly, so the mirror stays "driven only by messages"
    /// (design doc's Process Model) and redraw damage is never skipped.
    /// Called from inside `process_cmd`, where nothing downstream would
    /// otherwise notice status-bar damage.
    fn emit_lsp_msg(&mut self, msg: Msg) {
        self.process_automation_msg(msg);
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    /// `textDocument/definition` (lsp-integration.md Phase 3). Gates on
    /// the same information `didOpen` already established
    /// (`open_documents`) rather than re-resolving the server/root, so
    /// this and `lsp_open_document` never disagree about which server a
    /// document belongs to. Supersedes any request already in flight for
    /// `document_id` via `$/cancelRequest` first.
    fn request_lsp_definition(
        &mut self,
        document_id: token::model::editor_area::DocumentId,
        position: lsp_types::Position,
        revision: u64,
        origin: JumpEntry,
    ) {
        self.gated_lsp_request::<PendingDefinition>(
            document_id,
            Some(position),
            None,
            |(server_id, root, _)| PendingDefinition {
                document_id,
                revision,
                origin,
                server_id,
                root,
            },
        );
    }

    /// `textDocument/hover` (lsp-integration.md Phase 4). Mirrors
    /// `request_lsp_definition` exactly — same gating, flush-before-
    /// request, and supersede/cancel handling — with `cursor` (the
    /// char-column position at request time) threaded through instead of
    /// a jump-history `origin`.
    fn request_lsp_hover(
        &mut self,
        document_id: token::model::editor_area::DocumentId,
        position: lsp_types::Position,
        cursor: token::model::editor::Position,
        revision: u64,
    ) {
        self.gated_lsp_request::<PendingHover>(document_id, Some(position), None, |_| {
            PendingHover {
                document_id,
                revision,
                cursor,
            }
        });
    }

    /// `textDocument/signatureHelp` — `request_lsp_hover` plus the
    /// `SignatureHelpContext` params (`triggerKind` 2 for a typed
    /// trigger/retrigger character, 1 for an explicit invoke).
    fn request_lsp_signature_help(
        &mut self,
        document_id: token::model::editor_area::DocumentId,
        position: lsp_types::Position,
        cursor: token::model::editor::Position,
        revision: u64,
        trigger: Option<String>,
        is_retrigger: bool,
    ) {
        let mut context = serde_json::json!({
            "triggerKind": if trigger.is_some() { 2 } else { 1 },
            "isRetrigger": is_retrigger,
        });
        if let Some(ch) = trigger {
            context["triggerCharacter"] = serde_json::Value::String(ch);
        }
        self.gated_lsp_request::<PendingSignatureHelp>(
            document_id,
            Some(position),
            Some(serde_json::json!({ "context": context })),
            |_| PendingSignatureHelp {
                document_id,
                revision,
                cursor,
            },
        );
    }

    /// Rename Symbol entry point. A server with `renameProvider` but no
    /// `prepareProvider` skips the round trip: the prompt opens right away
    /// with `fallback`. Everything else (no server, not ready, no rename
    /// support, prepare supported) goes through the gated
    /// `textDocument/prepareRename` request.
    fn request_lsp_prepare_rename(
        &mut self,
        document_id: token::model::editor_area::DocumentId,
        position: lsp_types::Position,
        cursor: token::model::editor::Position,
        revision: u64,
        fallback: String,
    ) {
        let caps = self.lsp.open_documents.get(&document_id).and_then(|state| {
            self.lsp
                .servers
                .get(&(state.server_id.clone(), state.root.clone()))?
                .capabilities_snapshot()
        });
        if caps.is_some_and(|c| {
            lsp::client::supports_rename(&c) && !lsp::client::supports_prepare_rename(&c)
        }) {
            self.emit_lsp_msg(Msg::Lsp(LspMsg::PrepareRenameResolved {
                document_id,
                revision,
                cursor,
                placeholder: Some(fallback),
            }));
            return;
        }
        self.gated_lsp_request::<PendingPrepareRename>(document_id, Some(position), None, |_| {
            PendingPrepareRename {
                document_id,
                revision,
                cursor,
                fallback,
            }
        });
    }

    /// `textDocument/formatting` (`range: None`) or `rangeFormatting`,
    /// each behind its own capability gate. A `then_save` request gets
    /// the short `FORMAT_ON_SAVE_TIMEOUT` instead of the slot default.
    fn request_lsp_formatting(
        &mut self,
        document_id: token::model::editor_area::DocumentId,
        revision: u64,
        range: Option<lsp_types::Range>,
        options: lsp_types::FormattingOptions,
        then_save: bool,
    ) {
        let mut params = serde_json::json!({ "options": options });
        let (method, supports): (&'static str, fn(&lsp_types::ServerCapabilities) -> bool) =
            match range {
                Some(range) => {
                    params["range"] = serde_json::json!(range);
                    (
                        "textDocument/rangeFormatting",
                        lsp::client::supports_range_formatting,
                    )
                }
                None => ("textDocument/formatting", lsp::client::supports_formatting),
            };
        self.gated_lsp_request_as::<PendingFormatting>(
            document_id,
            method,
            supports,
            None,
            Some(params),
            |_| PendingFormatting {
                document_id,
                revision,
                then_save,
            },
        );
        if then_save {
            let slot = &mut self.lsp.formatting;
            if let Some(key) = slot.by_doc.get(&document_id).cloned() {
                slot.deadlines
                    .insert(key, Instant::now() + FORMAT_ON_SAVE_TIMEOUT);
            }
        }
    }

    /// Builds `LocationItem` previews for a `textDocument/references`
    /// reply — called from the interception pass (not `update()`) because
    /// a location in a file that isn't currently open needs a disk read
    /// to preview. Sorted by `(path, line)` and capped at
    /// `MAX_REFERENCE_LOCATIONS` before the (possibly expensive) preview
    /// reads, so the cap bounds I/O too, not just the popup's row count.
    fn build_reference_items(
        &self,
        locations: Vec<lsp_types::Location>,
        server_id: &LspServerId,
        root: &std::path::Path,
    ) -> Vec<token::update::navigation::LocationItem> {
        let mut resolved: Vec<(std::path::PathBuf, lsp_types::Position)> = locations
            .iter()
            .filter_map(|location| {
                let path = lsp::uri_to_path(&location.uri)?;
                Some((path, location.range.start))
            })
            .collect();
        resolved.sort_by(|a, b| (&a.0, a.1.line).cmp(&(&b.0, b.1.line)));
        resolved.truncate(MAX_REFERENCE_LOCATIONS);

        // ponytail: one `read_to_string` per distinct unopened file,
        // cached only for this batch (bounded by `MAX_REFERENCE_LOCATIONS`
        // distinct files at worst) — fine for a single references reply;
        // revisit with a real cache if a future caller does this more
        // than once per user action.
        let mut file_cache: HashMap<std::path::PathBuf, Vec<String>> = HashMap::new();
        resolved
            .into_iter()
            .map(|(path, position)| {
                let line = position.line as usize;
                let preview = self
                    .model
                    .editor_area
                    .find_open_file(&path)
                    .and_then(|(doc_id, _, _)| self.model.editor_area.documents.get(&doc_id))
                    .and_then(|doc| doc.get_line_cow(line).map(|c| c.into_owned()))
                    .or_else(|| {
                        let lines = file_cache.entry(path.clone()).or_insert_with(|| {
                            std::fs::read_to_string(&path)
                                .map(|s| s.lines().map(str::to_owned).collect())
                                .unwrap_or_default()
                        });
                        lines.get(line).cloned()
                    })
                    .map(|s| s.trim().to_owned())
                    .unwrap_or_default();
                // Same route-hint rule as `DefinitionResolved`'s
                // out-of-workspace branch: only set when this location
                // isn't under any workspace root, so an in-workspace jump
                // still lets the generic open path derive its root.
                let outside_every_root = self
                    .model
                    .workspace
                    .as_ref()
                    .is_none_or(|ws| !path.starts_with(&ws.root));
                let route_hint =
                    outside_every_root.then(|| (server_id.clone(), root.to_path_buf()));
                token::update::navigation::LocationItem {
                    path,
                    position,
                    preview,
                    route_hint,
                }
            })
            .collect()
    }

    /// `textDocument/references` (Show Usages / Find Usages). Mirrors
    /// `request_lsp_hover` exactly — same gating, flush-before-request,
    /// supersede/cancel handling — with `context.includeDeclaration: true`
    /// sent via `send_lsp_feature_request`'s `extra_params` hook.
    fn request_lsp_references(
        &mut self,
        document_id: token::model::editor_area::DocumentId,
        position: lsp_types::Position,
        cursor: token::model::editor::Position,
        revision: u64,
    ) {
        self.gated_lsp_request::<PendingReferences>(
            document_id,
            Some(position),
            Some(serde_json::json!({ "context": { "includeDeclaration": true } })),
            |_| PendingReferences {
                document_id,
                revision,
                cursor,
            },
        );
    }

    /// `workspace/executeCommand` on the server that owns `document_id`
    /// (looked up like `send_lsp_feature_request`). Fire-and-forget: the
    /// reply is resolved-and-dropped by the reader; result edits arrive
    /// as `workspace/applyEdit`.
    fn execute_lsp_command(
        &mut self,
        document_id: token::model::editor_area::DocumentId,
        command: String,
        arguments: Option<Vec<serde_json::Value>>,
    ) {
        let Some(state) = self.lsp.open_documents.get(&document_id) else {
            return;
        };
        let key = (state.server_id.clone(), state.root.clone());
        if let Some(handle) = self.lsp.servers.get(&key) {
            handle.begin_request(
                "workspace/executeCommand",
                serde_json::json!({ "command": command, "arguments": arguments }),
            );
        }
    }

    /// `textDocument/completion` (lsp-integration.md Phase 5) — fired by
    /// `check_lsp_completion_debounces` after `COMPLETION_DEBOUNCE` quiets.
    /// Mirrors `request_lsp_references`'s gating and flush-before-request,
    /// but *silent* on every gate failure: completion degrades to the
    /// menu's offline items, never a status transient (a "not supported"
    /// flash on every keystroke in an unsynced buffer would be noise).
    fn request_lsp_completion(
        &mut self,
        document_id: token::model::editor_area::DocumentId,
        position: lsp_types::Position,
        revision: u64,
        trigger_character: Option<String>,
    ) {
        // `triggerKind`: 2 (`TriggerCharacter`) when the keystroke that
        // scheduled this request was one of the server's trigger
        // characters; 1 (`Invoked`) otherwise — explicit Ctrl+Space or
        // plain typing. `TriggerForIncompleteCompletions` (3) is not used:
        // we always re-request the full list, which every server accepts.
        let context = match &trigger_character {
            Some(ch) => serde_json::json!({
                "triggerKind": 2,
                "triggerCharacter": ch,
            }),
            None => serde_json::json!({ "triggerKind": 1 }),
        };
        self.gated_lsp_request::<PendingCompletion>(
            document_id,
            Some(position),
            Some(serde_json::json!({ "context": context })),
            |_| PendingCompletion {
                document_id,
                revision,
            },
        );
    }

    /// `completionItem/resolve`. Unlike the other feature requests this
    /// isn't position-based, so it gates manually (server still running +
    /// resolve advertised) instead of through `send_lsp_feature_request`.
    /// For an `Accept` purpose every failure emits an empty
    /// `CompletionItemResolved` so the blocked accept applies immediately
    /// rather than hanging until `RESOLVE_TIMEOUT`; a `Docs` purpose fails
    /// silently. Any pending docs debounce for the document is dropped —
    /// this request supersedes it.
    #[allow(clippy::too_many_arguments)]
    fn request_lsp_resolve(
        &mut self,
        document_id: token::model::editor_area::DocumentId,
        revision: u64,
        server_id: LspServerId,
        root: PathBuf,
        raw_item: serde_json::Value,
        selected: usize,
        purpose: ResolvePurpose,
    ) {
        self.lsp.resolve_debounces.remove(&document_id);
        if let Some(old_key) = self.lsp.resolve.supersede(document_id) {
            self.cancel_lsp_request(&old_key);
        }
        let can_resolve = self
            .lsp
            .servers
            .get(&(server_id.clone(), root.clone()))
            .and_then(|handle| handle.capabilities_snapshot())
            .is_some_and(|caps| lsp::client::supports_completion_resolve(&caps));
        // Flush-before-request doesn't apply (resolve sees no text), but
        // the handle re-lookup pattern does — the flush-free path can't
        // drop the handle, so one lookup suffices.
        let handle = self
            .lsp
            .servers
            .get(&(server_id.clone(), root.clone()))
            .filter(|_| can_resolve);
        let Some(handle) = handle else {
            if purpose == ResolvePurpose::Accept {
                self.emit_lsp_msg(Msg::Lsp(LspMsg::CompletionItemResolved {
                    document_id,
                    revision,
                    selected,
                    detail: None,
                    documentation: None,
                    additional_text_edits: Vec::new(),
                }));
            }
            return;
        };
        let request_id = handle.begin_request("completionItem/resolve", raw_item);
        let key = (server_id, root, request_id);
        self.lsp.resolve.insert(
            key.clone(),
            document_id,
            PendingResolve {
                document_id,
                revision,
                selected,
                purpose,
            },
        );
        self.lsp.resolve.arm_deadline(key);
    }

    /// Whether the model mirror has `server_id` in `Starting` or
    /// `Indexing` — used to tell "hasn't finished analyzing yet" apart
    /// from a genuine empty/not-found result for definition/hover replies
    /// (design doc lines 101/212: "never 'not found' before `Ready`").
    fn is_lsp_indexing(&self, server_id: &LspServerId) -> bool {
        matches!(
            self.model.lsp.servers.get(server_id),
            Some(ServerState::Starting | ServerState::Indexing)
        )
    }

    /// Routes a server-state change through `Msg::Lsp(ServerStateChanged)`
    /// — the same path the worker threads use — instead of poking
    /// `model.lsp.servers` directly, so the mirror stays "driven only by
    /// messages" (design doc's Process Model) and redraw damage is never
    /// skipped.
    fn set_lsp_server_state(&mut self, server_id: LspServerId, root: &Path, state: ServerState) {
        self.process_automation_msg(Msg::Lsp(LspMsg::ServerStateChanged {
            server_id,
            root: root.to_path_buf(),
            state,
        }));
        // Unlike worker-originated `ServerStateChanged` (routed through
        // `process_async_messages`, which returns `needs_redraw` to its
        // caller), this is called synchronously from inside `process_cmd`
        // — nothing downstream will otherwise notice the status-bar
        // damage `process_automation_msg` merged into `pending_damage`.
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    /// A server's worker thread hit EOF/error on stdout — the child
    /// exited (crash or otherwise). Removes the dead handle and schedules
    /// a respawn after an exponential backoff, capped at
    /// `MAX_RESTART_ATTEMPTS`; giving up reports `Failed`.
    /// `LspManager::shutting_down` (quit teardown) suppresses this
    /// entirely — a deliberate kill's EOF must never trigger a restart.
    fn handle_lsp_server_exited(&mut self, server_id: &LspServerId, generation: u64) {
        if self.lsp.shutting_down {
            return;
        }
        // Only remove/restart handles whose generation matches the one
        // that actually exited — a deliberate kill+respawn (restart,
        // quit) can race this message against a replacement handle
        // already being installed at the same (id, root) key. See
        // `ServerHandle::generation`'s doc comment.
        let roots: Vec<PathBuf> = self
            .lsp
            .servers
            .iter()
            .filter(|((id, _), handle)| id == server_id && handle.generation == generation)
            .map(|((_, root), _)| root.clone())
            .collect();
        if roots.is_empty() {
            return;
        }
        for root in &roots {
            if let Some(mut handle) = self.lsp.servers.remove(&(server_id.clone(), root.clone())) {
                // The reader thread only observed EOF/exit; nothing has
                // `wait()`ed on this child yet. `kill()` is safe to call
                // on an already-dead process (it just errors) and its
                // `wait()` is what actually reaps it — without this every
                // server that exits on its own (or whose `initialize`
                // failed, which routes through here too — see the
                // reader's `initialize` error arm) becomes an unreaped
                // zombie for the rest of the session.
                handle.kill();
            }
        }
        self.clear_diagnostics_for_roots(server_id, &roots);
        self.clear_pending_requests_for_roots(server_id, &roots);

        for root in roots {
            let key = (server_id.clone(), root.clone());
            let entry = self.lsp.restart_attempts.entry(key.clone()).or_insert(0);
            *entry += 1;
            let attempts = *entry;
            if attempts > MAX_RESTART_ATTEMPTS {
                self.lsp
                    .failed_roots
                    .entry(server_id.clone())
                    .or_default()
                    .push(root.clone());
                self.set_lsp_server_state(server_id.clone(), &root, ServerState::Failed);
                continue;
            }
            self.set_lsp_server_state(
                server_id.clone(),
                &root,
                ServerState::Restarting { attempt: attempts },
            );
            // Exponential backoff: 200ms, 400ms, 800ms, ... capped so a
            // flapping server doesn't spin the event loop.
            let delay_ms = RESTART_BACKOFF_BASE_MS.saturating_mul(1u64 << (attempts - 1));
            let delay_ms = delay_ms.min(RESTART_BACKOFF_MAX_MS);
            self.lsp
                .restart_deadlines
                .insert(key, Instant::now() + Duration::from_millis(delay_ms));
        }
    }

    /// Fires respawns scheduled by `handle_lsp_server_exited`'s backoff.
    fn check_lsp_restart_deadlines(&mut self) {
        if self.lsp.restart_deadlines.is_empty() {
            return;
        }
        let now = Instant::now();
        let due: Vec<(LspServerId, PathBuf)> = self
            .lsp
            .restart_deadlines
            .iter()
            .filter(|(_, deadline)| now >= **deadline)
            .map(|(key, _)| key.clone())
            .collect();
        for (server_id, root) in due {
            self.lsp
                .restart_deadlines
                .remove(&(server_id.clone(), root.clone()));
            let Some(def) = lsp::server_def_by_id(&server_id.0) else {
                continue;
            };
            let Some(resolved) = lsp::resolve_server(def, &self.model.config.lsp) else {
                continue;
            };
            self.lsp
                .resync_pending
                .insert((server_id.clone(), root.clone()));
            self.spawn_lsp_server_at(&resolved, &root);
        }
    }

    /// Fires `DEFINITION_TIMEOUT` UI-level abandonment for definition
    /// requests a server never answered (design doc's "hung server
    /// degrades to abandoned requests with honest status messages").
    /// A deadline for a request that's since been superseded (no longer
    /// the doc's *current* request in `definition_request_by_doc`) is
    /// dropped silently — the newer request owns whatever outcome the
    /// user eventually sees.
    fn check_lsp_definition_deadlines(&mut self) {
        self.sweep_lsp_feature_deadlines::<PendingDefinition>();
    }

    /// Fires `HOVER_TIMEOUT` UI-level abandonment for hover requests a
    /// server never answered — mirrors `check_lsp_definition_deadlines`.
    /// Since `HoverOutcome` has no "no result" variant distinct from
    /// `Content(None)`, an abandoned request resolves as `Content(None)`
    /// (the same "the server had nothing to say" outcome a fast `null`
    /// reply would have produced).
    fn check_lsp_hover_deadlines(&mut self) {
        self.sweep_lsp_feature_deadlines::<PendingHover>();
    }

    /// Silent `SIGNATURE_HELP_TIMEOUT` abandonment sweep.
    fn check_lsp_signature_help_deadlines(&mut self) {
        self.sweep_lsp_feature_deadlines::<PendingSignatureHelp>();
    }

    /// `RENAME_TIMEOUT` sweep for both rename slots — flashes "server did
    /// not answer".
    fn check_lsp_rename_deadlines(&mut self) {
        self.sweep_lsp_feature_deadlines::<PendingPrepareRename>();
        self.sweep_lsp_feature_deadlines::<PendingRename>();
    }

    /// Formatting abandonment sweep — resolves with `edits: None` so a
    /// `then_save` request still saves.
    fn check_lsp_formatting_deadlines(&mut self) {
        self.sweep_lsp_feature_deadlines::<PendingFormatting>();
    }

    /// Fires `REFERENCES_TIMEOUT` UI-level abandonment for references
    /// requests a server never answered — mirrors
    /// `check_lsp_definition_deadlines`.
    fn check_lsp_references_deadlines(&mut self) {
        self.sweep_lsp_feature_deadlines::<PendingReferences>();
    }

    fn check_lsp_code_action_deadlines(&mut self) {
        self.sweep_lsp_feature_deadlines::<PendingCodeActions>();
    }

    /// Fires due completion debounces into real requests. Silent on gate
    /// failure (`request_lsp_completion`'s own policy) — a debounce armed
    /// for a document that lost its server between schedule and fire just
    /// evaporates.
    /// Replay elapsed inline-suggestion debounces into the update layer,
    /// which re-checks the revision and snapshots the request.
    fn check_inline_deadlines(&mut self) {
        if self.inline_deadlines.is_empty() {
            return;
        }
        let now = Instant::now();
        let due: Vec<_> = self
            .inline_deadlines
            .iter()
            .filter(|(_, (deadline, _, _))| now >= *deadline)
            .map(|(document_id, (_, revision, explicit))| (*document_id, *revision, *explicit))
            .collect();
        for (document_id, revision, explicit) in due {
            self.inline_deadlines.remove(&document_id);
            self.process_automation_msg(Msg::Completion(CompletionMsg::InlineDeadlineFired {
                document_id,
                revision,
                explicit,
            }));
        }
    }

    fn check_lsp_completion_debounces(&mut self) {
        if self.lsp.completion_debounces.is_empty() {
            return;
        }
        let now = Instant::now();
        let due: Vec<token::model::editor_area::DocumentId> = self
            .lsp
            .completion_debounces
            .iter()
            .filter(|(_, scheduled)| now >= scheduled.deadline)
            .map(|(doc, _)| *doc)
            .collect();
        for document_id in due {
            let Some(scheduled) = self.lsp.completion_debounces.remove(&document_id) else {
                continue;
            };
            self.request_lsp_completion(
                document_id,
                scheduled.position,
                scheduled.revision,
                scheduled.trigger_character,
            );
        }
    }

    /// Completion's UI-level abandonment sweep — silent, unlike its
    /// definition/hover/references siblings: the menu keeps whatever
    /// offline items it has, and no status transient ever fires.
    fn check_lsp_completion_deadlines(&mut self) {
        self.sweep_lsp_feature_deadlines::<PendingCompletion>();
    }

    /// Fires due docs-purpose resolves — `check_lsp_completion_debounces`'s
    /// twin.
    fn check_lsp_resolve_debounces(&mut self) {
        if self.lsp.resolve_debounces.is_empty() {
            return;
        }
        let now = Instant::now();
        let due: Vec<token::model::editor_area::DocumentId> = self
            .lsp
            .resolve_debounces
            .iter()
            .filter(|(_, scheduled)| now >= scheduled.deadline)
            .map(|(doc, _)| *doc)
            .collect();
        for document_id in due {
            let Some(scheduled) = self.lsp.resolve_debounces.remove(&document_id) else {
                continue;
            };
            self.request_lsp_resolve(
                document_id,
                scheduled.revision,
                scheduled.server_id,
                scheduled.root,
                scheduled.raw_item,
                scheduled.selected,
                ResolvePurpose::Docs,
            );
        }
    }

    /// Resolve's abandonment sweep. Unlike completion, an `Accept`-purpose
    /// expiry MUST emit: an accept is blocked on the round trip, and
    /// letting the deadline pass silently would leave Enter dead until
    /// Escape. The empty outcome unblocks the accept with what the item
    /// already carried. A `Docs`-purpose expiry just drops.
    fn check_lsp_resolve_deadlines(&mut self) {
        if self.lsp.resolve.is_empty_deadlines() {
            return;
        }
        for (key, pending) in self.lsp.resolve.take_due(Instant::now()) {
            self.cancel_lsp_request(&key);
            if pending.purpose != ResolvePurpose::Accept {
                continue;
            }
            self.emit_lsp_msg(Msg::Lsp(LspMsg::CompletionItemResolved {
                document_id: pending.document_id,
                revision: pending.revision,
                selected: pending.selected,
                detail: None,
                documentation: None,
                additional_text_edits: Vec::new(),
            }));
        }
    }

    /// Drain any pending background PTY spawn results and create the
    /// corresponding `TerminalSession` on the main thread.
    fn process_terminal_spawn_results(&mut self) -> bool {
        let mut needs_redraw = false;
        let mut clear_receiver = false;

        {
            let Some((spawn_session_id, rx)) = self.terminal_spawn_rx.as_ref() else {
                return false;
            };
            let spawn_session_id = *spawn_session_id;

            loop {
                match rx.try_recv() {
                    Ok(Ok(mut result)) => {
                        clear_receiver = true;
                        let should_keep = self.should_keep_terminal_spawn_result(result.session_id);
                        self.model.terminal.clear_spawn_pending(result.session_id);
                        if result.session_id != spawn_session_id {
                            self.model.terminal.clear_spawn_pending(spawn_session_id);
                        }

                        if should_keep {
                            let session = token::terminal::TerminalSession::new(
                                result.session_id,
                                result.rows,
                                result.cols,
                                result.pty,
                                self.msg_tx.clone(),
                            );
                            self.model.terminal.sessions.push(session);
                            self.model.terminal.active =
                                self.model.terminal.sessions.len().saturating_sub(1);
                            self.pending_damage.merge(Cmd::Redraw.damage());
                            needs_redraw = true;
                        } else {
                            result.pty.kill();
                        }
                    }
                    Ok(Err(e)) => {
                        clear_receiver = true;
                        self.model.terminal.clear_spawn_pending(spawn_session_id);
                        tracing::warn!("Failed to spawn terminal: {}", e);
                        self.model
                            .ui
                            .set_status(format!("Failed to spawn terminal: {e}"));
                        self.pending_damage.merge(Cmd::redraw_status_bar().damage());
                        needs_redraw = true;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        clear_receiver = true;
                        self.model.terminal.clear_spawn_pending(spawn_session_id);
                        break;
                    }
                }
            }
        }

        if clear_receiver {
            self.terminal_spawn_rx = None;
        }

        needs_redraw
    }

    fn should_keep_terminal_spawn_result(&self, session_id: usize) -> bool {
        self.model.terminal.is_spawn_pending(session_id)
            && self
                .model
                .dock_layout
                .active_panel_position(token::panel::PanelId::TERMINAL)
                .is_some()
    }
}

const DEMO_DOCUMENT: &str = r#"use std::time::Instant;

fn render_frame(lines: &[&str]) -> usize {
    let started = Instant::now();
    let glyphs = lines.iter().map(|line| line.chars().count()).sum();
    println!("rendered {glyphs} glyphs in {:?}", started.elapsed());
    glyphs
}

fn main() {
    let lines = ["Token", "deterministic", "automation demo"];
    assert_eq!(render_frame(&lines), 34);
}
"#;

/// Create window icon from embedded PNG
#[cfg(not(target_os = "macos"))]
fn create_window_icon() -> Option<Icon> {
    let icon_bytes = include_bytes!("../../assets/icon.png");
    let icon_image = image::load_from_memory(icon_bytes).ok()?.to_rgba8();
    let (width, height) = icon_image.dimensions();
    Icon::from_rgba(icon_image.into_raw(), width, height).ok()
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window_attributes = Window::default_attributes()
                .with_title(token::product::DISPLAY_NAME)
                .with_inner_size(LogicalSize::new(800, 600)); // TODO: Persist window size/position/monitor on exit/boot
            #[cfg(not(target_os = "macos"))]
            let window_attributes = window_attributes.with_window_icon(create_window_icon());

            let window = match event_loop.create_window(window_attributes) {
                Ok(w) => Rc::new(w),
                Err(e) => {
                    tracing::error!("Failed to create window: {}", e);
                    event_loop.exit();
                    return;
                }
            };

            let context = match Context::new(Rc::clone(&window)) {
                Ok(ctx) => ctx,
                Err(e) => {
                    tracing::error!("Failed to create rendering context: {}", e);
                    event_loop.exit();
                    return;
                }
            };

            if let Err(e) = self.init_renderer(Rc::clone(&window), &context) {
                tracing::error!("Failed to initialize renderer: {}", e);
                event_loop.exit();
                return;
            }

            self.window = Some(window);
            self.context = Some(context);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let should_exit = matches!(event, WindowEvent::CloseRequested);
        let should_redraw = if let Some(window) = &self.window {
            if window_id == window.id() && !should_exit {
                if let Some(cmd) = self.handle_event(&event) {
                    let needs_redraw = cmd.needs_redraw();
                    // Accumulate damage from command
                    self.pending_damage.merge(cmd.damage());
                    self.process_cmd(cmd);
                    needs_redraw
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        if should_exit || self.should_quit {
            // Window-close (titlebar X / OS gesture) bypasses Cmd::Quit, so
            // run the same LSP shutdown->exit->kill sequence here.
            if should_exit {
                self.graceful_lsp_teardown();
            }
            event_loop.exit();
        } else if should_redraw {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        // Waiters first: a `--wait` client must read its answer before
        // the endpoint it connected through disappears.
        self.answer_exit_waiters();
        automation::remove_own_endpoint();
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let mut needs_redraw = false;

        if self.process_automation_requests() {
            needs_redraw = true;
        }

        if self.process_async_messages() {
            needs_redraw = true;
        }

        // `Cmd::Quit` from a non-window source (automation, menu) only sets
        // the flag; `window_event` is the only other place that acts on it.
        if self.should_quit {
            event_loop.exit();
            return;
        }

        self.poll_document_waiters();

        if self
            .deferred_startup_at
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.finish_deferred_startup();
        }

        // Poll file system watcher for changes
        if self.poll_fs_watcher() {
            needs_redraw = true;
        }

        // Check syntax debounce deadlines, capturing any snapshot the
        // fired parses took — the LSP did-change check below reuses them
        // for documents whose deadlines coincide (design doc: "must not
        // double" the rope `to_string()`).
        let (syntax_redraw, syntax_snapshots) = self.check_syntax_deadlines();
        if syntax_redraw {
            needs_redraw = true;
        }
        self.check_lsp_did_change_deadlines(&syntax_snapshots);
        self.check_lsp_restart_deadlines();
        self.check_lsp_definition_deadlines();
        self.check_lsp_hover_deadlines();
        self.check_lsp_signature_help_deadlines();
        self.check_lsp_rename_deadlines();
        self.check_lsp_formatting_deadlines();
        self.check_lsp_references_deadlines();
        self.check_lsp_code_action_deadlines();
        self.check_lsp_completion_debounces();
        self.check_lsp_completion_deadlines();
        self.check_inline_deadlines();
        self.check_lsp_resolve_debounces();
        self.check_lsp_resolve_deadlines();
        if self.check_hover_dwell() {
            needs_redraw = true;
        }

        // Expire status flash messages
        if self.model.ui.expire_status_message() {
            self.pending_damage
                .merge(Damage::Areas(vec![DamageArea::StatusBar]));
            needs_redraw = true;
        }

        if needs_redraw {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }

        // Check if cursor blink timer has elapsed
        let now = Instant::now();
        let time_since_tick = now.duration_since(self.last_tick);
        let blink_interval = Duration::from_millis(self.model.config.cursor_blink_ms);

        if !blink_interval.is_zero() && time_since_tick >= blink_interval {
            self.last_tick = now;
            if let Some(cmd) = self.tick() {
                // Accumulate damage from cursor blink
                self.pending_damage.merge(cmd.damage());
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
        }

        // Use WaitUntil to wake up for the next cursor blink
        // This avoids spinning the event loop constantly (Poll mode)
        // while still handling async messages, fs changes, and cursor blinks
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_wake(now)));
    }
}

impl App {
    /// Earliest instant `about_to_wait` must run again: the next cursor
    /// blink or the earliest pending deadline. Deadlines already in the
    /// past are excluded — their check ran this tick and either fired or
    /// declined, and `WaitUntil` on a past instant spins the loop at 100%
    /// CPU.
    pub(super) fn next_wake(&self, now: Instant) -> Instant {
        let mut next_wake = if self.model.config.cursor_blink_ms == 0 {
            // Retain a modest maintenance wake for transient-message expiry.
            now + Duration::from_millis(600)
        } else {
            self.last_tick + Duration::from_millis(self.model.config.cursor_blink_ms)
        };
        if let Some(earliest_deadline) = self.syntax_deadlines.values().map(|(d, _)| *d).min() {
            next_wake = next_wake.min(earliest_deadline);
        }
        if let Some(earliest_deadline) = self.lsp_change_deadlines.next_deadline() {
            next_wake = next_wake.min(earliest_deadline);
        }
        if let Some(earliest_deadline) = self.inline_deadlines.values().map(|(d, _, _)| *d).min() {
            next_wake = next_wake.min(earliest_deadline);
        }
        if let Some(earliest_deadline) = self.lsp.restart_deadlines.values().min() {
            next_wake = next_wake.min(*earliest_deadline);
        }
        if let Some(earliest_deadline) = self.lsp.definition.earliest_deadline() {
            next_wake = next_wake.min(earliest_deadline);
        }
        if let Some(earliest_deadline) = self.lsp.hover.earliest_deadline() {
            next_wake = next_wake.min(earliest_deadline);
        }
        if let Some(earliest_deadline) = self.lsp.signature_help.earliest_deadline() {
            next_wake = next_wake.min(earliest_deadline);
        }
        for deadline in [
            self.lsp.prepare_rename.earliest_deadline(),
            self.lsp.rename.earliest_deadline(),
        ]
        .into_iter()
        .flatten()
        {
            next_wake = next_wake.min(deadline);
        }
        if let Some(earliest_deadline) = self.lsp.formatting.earliest_deadline() {
            next_wake = next_wake.min(earliest_deadline);
        }
        if let Some(earliest_deadline) = self.lsp.references.earliest_deadline() {
            next_wake = next_wake.min(earliest_deadline);
        }
        if let Some(earliest_deadline) = self.lsp.code_actions.earliest_deadline() {
            next_wake = next_wake.min(earliest_deadline);
        }
        if let Some(earliest_deadline) = self.lsp.completion.earliest_deadline() {
            next_wake = next_wake.min(earliest_deadline);
        }
        if let Some(earliest_deadline) = self.lsp.resolve.earliest_deadline() {
            next_wake = next_wake.min(earliest_deadline);
        }
        if let Some(earliest_deadline) = self
            .lsp
            .completion_debounces
            .values()
            .map(|scheduled| scheduled.deadline)
            .min()
        {
            next_wake = next_wake.min(earliest_deadline);
        }
        if let Some(earliest_deadline) = self
            .lsp
            .resolve_debounces
            .values()
            .map(|scheduled| scheduled.deadline)
            .min()
        {
            next_wake = next_wake.min(earliest_deadline);
        }
        if let Some(transient) = &self.model.ui.transient_message {
            next_wake = next_wake.min(transient.expires_at);
        }
        if let Some(deferred_startup_at) = self.deferred_startup_at {
            next_wake = next_wake.min(deferred_startup_at);
        }
        if let Some((_, _, started)) = self.hover_dwell {
            let delay = Duration::from_millis(self.model.config.hover_delay_ms);
            let dwell_deadline = started + delay;
            // An armed dwell whose deadline passed without firing (pointer
            // parked over the sidebar, hover disabled, ...) needs no wake-up.
            if dwell_deadline > now {
                next_wake = next_wake.min(dwell_deadline);
            }
        }
        next_wake
    }
}

impl App {
    fn process_automation_requests(&mut self) -> bool {
        let mut redraw = false;
        while let Ok(envelope) = self.automation_rx.try_recv() {
            match envelope.request {
                AutomationRequest::State => {
                    let _ = envelope.response_tx.send(self.automation_response("state"));
                }
                AutomationRequest::Document => {
                    let response =
                        match crate::automation::DocumentSnapshot::from_model(&self.model) {
                            Ok(document) => {
                                let mut response = self.automation_response("document");
                                response.document = Some(document);
                                response
                            }
                            Err(error) => AutomationResponse::error(error),
                        };
                    let _ = envelope.response_tx.send(response);
                }
                AutomationRequest::Actions => {
                    let mut actions: Vec<_> = self
                        .keymap
                        .bindings()
                        .iter()
                        .map(|binding| crate::automation::ActionSnapshot {
                            name: format!("{:?}", binding.command),
                            label: binding.command.display_name().to_owned(),
                            keybinding: binding.display_string(),
                        })
                        .collect();
                    actions.sort_by(|left, right| left.name.cmp(&right.name));
                    actions.dedup_by(|left, right| left.name == right.name);
                    let mut response = self.automation_response("actions");
                    response.actions = Some(actions);
                    let _ = envelope.response_tx.send(response);
                }
                AutomationRequest::InsertText { text } => {
                    self.process_automation_msg(Msg::Document(
                        token::messages::DocumentMsg::InsertText(text),
                    ));
                    let _ = envelope
                        .response_tx
                        .send(self.automation_response("text inserted"));
                    redraw = true;
                }
                AutomationRequest::SetCursor { line, column } => {
                    self.process_automation_msg(Msg::Editor(EditorMsg::CollapseToSingleCursor));
                    self.process_automation_msg(Msg::Editor(EditorMsg::SetCursorPosition {
                        line,
                        column,
                    }));
                    let _ = envelope
                        .response_tx
                        .send(self.automation_response("cursor set"));
                    redraw = true;
                }
                AutomationRequest::SetSelection {
                    anchor_line,
                    anchor_column,
                    head_line,
                    head_column,
                } => {
                    let (anchor_line, anchor_column) =
                        clamped_document_position(&self.model, anchor_line, anchor_column);
                    let (head_line, head_column) =
                        clamped_document_position(&self.model, head_line, head_column);
                    self.process_automation_msg(Msg::Editor(EditorMsg::CollapseToSingleCursor));
                    self.process_automation_msg(Msg::Editor(EditorMsg::SetCursorPosition {
                        line: anchor_line,
                        column: anchor_column,
                    }));
                    self.process_automation_msg(Msg::Editor(
                        EditorMsg::ExtendSelectionToPosition {
                            line: head_line,
                            column: head_column,
                        },
                    ));
                    let _ = envelope
                        .response_tx
                        .send(self.automation_response("selection set"));
                    redraw = true;
                }
                AutomationRequest::ExecuteAction { name } => {
                    let response = match Command::from_str(&name) {
                        // `ShowContextMenu` has no `to_msgs()` (it needs a
                        // live clipboard read, runtime-only) — route it
                        // through the same `dispatch_command` special case
                        // the real keyboard/palette paths use, per
                        // context-menu.md Phase 5 automation.
                        Ok(Command::ShowContextMenu) => {
                            if let Some(cmd) = self.dispatch_command(Command::ShowContextMenu) {
                                self.pending_damage.merge(cmd.damage());
                                self.process_cmd(cmd);
                            }
                            redraw = true;
                            self.automation_response("action executed")
                        }
                        Ok(command) if command.is_simple() && command != Command::Unbound => {
                            for msg in command.to_msgs() {
                                self.process_automation_msg(msg);
                            }
                            redraw = true;
                            self.automation_response("action executed")
                        }
                        Ok(_) => AutomationResponse::error(format!(
                            "action `{name}` requires raw keyboard context and cannot be executed semantically"
                        )),
                        Err(()) => AutomationResponse::error(format!(
                            "unknown action `{name}`; call list_actions to inspect bound action names"
                        )),
                    };
                    let _ = envelope.response_tx.send(response);
                }
                AutomationRequest::Scroll { lines } => {
                    self.process_automation_msg(Msg::Editor(EditorMsg::Scroll(lines)));
                    let _ = envelope
                        .response_tx
                        .send(self.automation_response("scrolled"));
                    redraw = true;
                }
                AutomationRequest::SetOverlayInput { text } => {
                    if self.model.ui.active_modal.is_some() {
                        self.process_automation_msg(Msg::Ui(UiMsg::Modal(ModalMsg::SetInput(
                            text,
                        ))));
                        redraw = true;
                        let _ = envelope
                            .response_tx
                            .send(self.automation_response("overlay input set"));
                    } else {
                        let _ = envelope
                            .response_tx
                            .send(AutomationResponse::error("no overlay is open"));
                    }
                }
                AutomationRequest::ProfileSyntax { text } => {
                    if text.is_empty() {
                        let _ = envelope
                            .response_tx
                            .send(AutomationResponse::error("profile text must not be empty"));
                    } else if self.automation_syntax_profile.is_some() {
                        let _ = envelope.response_tx.send(AutomationResponse::error(
                            "a syntax profile is already running",
                        ));
                    } else if !self.model.document().language.has_highlighting() {
                        let _ = envelope.response_tx.send(AutomationResponse::error(
                            "the active document has no syntax highlighter",
                        ));
                    } else {
                        self.process_automation_msg(Msg::Document(
                            token::messages::DocumentMsg::InsertText(text),
                        ));
                        let document_id = self.model.document().id;
                        if let Some(document_id) = document_id {
                            self.automation_syntax_profile = Some(AutomationSyntaxProfile {
                                document_id,
                                revision: self.model.document().revision,
                                response_tx: envelope.response_tx,
                            });
                            redraw = true;
                        } else {
                            let _ = envelope.response_tx.send(AutomationResponse::error(
                                "the active document has no document id",
                            ));
                        }
                    }
                }
                AutomationRequest::OpenPaths { paths, wait } => {
                    let mut remaining = HashSet::new();
                    for crate::automation::OpenPath { path, line, column } in paths {
                        if path.is_dir() {
                            self.start_editor_for_directory(path);
                            continue;
                        }
                        self.open_path_at(&path, line, column);
                        if wait {
                            if let Some((document_id, _, _)) =
                                self.model.editor_area.find_open_file(&path)
                            {
                                remaining.insert(document_id);
                            }
                        }
                    }
                    if let Some(window) = &self.window {
                        window.focus_window();
                    }
                    if wait {
                        self.document_waiters.push(DocumentWaiter {
                            exit_only: remaining.is_empty(),
                            remaining,
                            response_tx: envelope.response_tx,
                        });
                    } else {
                        let _ = envelope
                            .response_tx
                            .send(self.automation_response("opened"));
                    }
                    redraw = true;
                }
                AutomationRequest::ProfileFrames { frames } => {
                    if frames == 0 || frames > 10_000 {
                        let _ = envelope.response_tx.send(AutomationResponse::error(
                            "frames must be between 1 and 10000",
                        ));
                    } else if self.automation_profile.is_some() {
                        let _ = envelope.response_tx.send(AutomationResponse::error(
                            "a frame profile is already running",
                        ));
                    } else {
                        self.perf.clear_history();
                        self.automation_profile = Some(AutomationProfile {
                            remaining_frames: frames,
                            response_tx: envelope.response_tx,
                        });
                        self.pending_damage = Damage::Full;
                        redraw = true;
                    }
                }
            }
        }
        redraw
    }

    fn process_automation_msg(&mut self, msg: Msg) {
        if let Some(cmd) = update(&mut self.model, msg) {
            self.pending_damage.merge(cmd.damage());
            self.process_cmd(cmd);
        }
    }

    /// The macOS open-file hook pushes `OpenPaths` requests through this.
    pub fn automation_sender(&self) -> Sender<AutomationEnvelope> {
        self.automation_tx.clone()
    }

    /// Open (or focus) `path` in the focused group and, when a 1-indexed
    /// `line` is given, place the cursor there.
    fn open_path_at(&mut self, path: &Path, line: Option<usize>, column: Option<usize>) {
        use token::update::navigation::{focused_tab_shows, open_or_focus, place_cursor_char};
        if let Some(cmd) = open_or_focus(&mut self.model, path.to_path_buf()) {
            self.pending_damage.merge(cmd.damage());
            self.process_cmd(cmd);
        }
        if let Some(line) = line {
            if focused_tab_shows(&self.model, path) {
                let column = column.unwrap_or(1).saturating_sub(1);
                if let Some(cmd) =
                    place_cursor_char(&mut self.model, line.saturating_sub(1), column)
                {
                    self.pending_damage.merge(cmd.damage());
                    self.process_cmd(cmd);
                }
            }
        }
        self.model.record_file_opened(path.to_path_buf());
    }

    /// A window owns one workspace, so a directory delivered to a running
    /// editor (Finder, MCP) is focused here if it is ours, handed to the
    /// editor already showing it, or given its own process. Discovery
    /// runs off the main thread: this thread is what answers it, and a
    /// wedged sibling must not freeze the UI. Tests never spawn: the
    /// test binary would re-run itself.
    fn start_editor_for_directory(&self, path: PathBuf) {
        let root = path.canonicalize().unwrap_or_else(|_| path.clone());
        let is_ours = self.model.workspace_root().map(|r| r.as_path()) == Some(root.as_path());
        if is_ours {
            if let Some(window) = &self.window {
                window.focus_window();
            }
        }
        #[cfg(not(test))]
        if !is_ours {
            std::thread::spawn(move || {
                let child_args = [path.clone().into_os_string()];
                crate::launcher::open_directory(
                    &path,
                    Vec::new(),
                    &child_args,
                    false,
                    Some(std::process::id()),
                );
            });
        }
    }

    /// Answer every `--wait` client whose documents have all been released.
    /// `documents.remove` in `close_tab` is the single release truth and
    /// ids are never reused, so a missing id means "closed".
    fn poll_document_waiters(&mut self) {
        if self.document_waiters.is_empty() {
            return;
        }
        let documents = &self.model.editor_area.documents;
        let mut finished = Vec::new();
        self.document_waiters.retain_mut(|waiter| {
            if waiter.exit_only {
                return true;
            }
            waiter
                .remaining
                .retain(|document_id| documents.contains_key(document_id));
            if waiter.remaining.is_empty() {
                finished.push(waiter.response_tx.clone());
                false
            } else {
                true
            }
        });
        for response_tx in finished {
            let _ = response_tx.send(self.automation_response("closed"));
        }
    }

    /// The editor is exiting: every pending `--wait` is over.
    fn answer_exit_waiters(&mut self) {
        for waiter in std::mem::take(&mut self.document_waiters) {
            let _ = waiter
                .response_tx
                .send(self.automation_response("editor exited"));
        }
    }

    fn automation_response(&self, message: &str) -> AutomationResponse {
        let mut state = crate::automation::EditorSnapshot::from_model(&self.model);
        state.focused_at_ms = self
            .focused_at
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_millis() as u64);
        let mut response = AutomationResponse::success(message, state, self.perf.snapshot());
        response.syntax_performance = self.latest_syntax_performance.clone();
        response
    }
}

fn clamped_document_position(model: &AppModel, line: usize, column: usize) -> (usize, usize) {
    let line = line.min(model.document().line_count().saturating_sub(1));
    (line, column.min(model.document().line_length(line)))
}

impl App {
    /// Check syntax debounce deadlines and fire ParseReady for expired
    /// ones. Returns whether a redraw is needed, plus the buffer
    /// snapshot `update_syntax` took for each document whose parse fired
    /// this tick — `check_lsp_did_change_deadlines` reuses these instead
    /// of paying its own `buffer.to_string()` when both deadlines
    /// coincide (design doc: "must not double" the rope-to-string cost).
    fn check_syntax_deadlines(
        &mut self,
    ) -> (
        bool,
        HashMap<token::model::editor_area::DocumentId, Arc<str>>,
    ) {
        let mut snapshots = HashMap::new();
        if self.syntax_deadlines.is_empty() {
            return (false, snapshots);
        }
        let now = Instant::now();
        let expired: Vec<_> = self
            .syntax_deadlines
            .iter()
            .filter(|(_, (deadline, _))| now >= *deadline)
            .map(|(&doc_id, &(_, revision))| (doc_id, revision))
            .collect();

        if expired.is_empty() {
            return (false, snapshots);
        }

        let mut needs_redraw = false;
        for (document_id, revision) in expired {
            self.syntax_deadlines.remove(&document_id);
            tracing::debug!(
                "Syntax deadline fired: doc={} rev={}",
                document_id.0,
                revision
            );
            if let Some(cmd) = update(
                &mut self.model,
                Msg::Syntax(SyntaxMsg::ParseReady {
                    document_id,
                    revision,
                }),
            ) {
                if let Cmd::RunSyntaxParse { ref source, .. } = cmd {
                    snapshots.insert(document_id, source.clone());
                }
                if cmd.needs_redraw() {
                    needs_redraw = true;
                }
                self.pending_damage.merge(cmd.damage());
                self.process_cmd(cmd);
            }
        }
        (needs_redraw, snapshots)
    }

    /// Fires debounced `didChange` for expired deadlines
    /// (`Cmd::LspScheduleDidChange`'s deadline map). Discards stale
    /// entries whose document was closed/removed, or whose buffer moved
    /// on to a newer revision since the deadline was scheduled (the next
    /// edit's own `record_edit` call will have already re-armed the
    /// deadline for that newer revision).
    fn check_lsp_did_change_deadlines(
        &mut self,
        shared_snapshots: &HashMap<token::model::editor_area::DocumentId, Arc<str>>,
    ) {
        let now = Instant::now();
        for (document_id, revision) in self.lsp_change_deadlines.take_expired(now) {
            let current_revision = self
                .model
                .editor_area
                .documents
                .get(&document_id)
                .map(|doc| doc.revision);
            if current_revision != Some(revision) {
                continue; // document closed, or a newer edit superseded this deadline
            }
            let text = shared_snapshots.get(&document_id).cloned();
            self.send_lsp_did_change(document_id, revision, text);
        }
    }

    /// Poll file system watcher and dispatch events
    /// Returns true if any events were processed
    fn poll_fs_watcher(&mut self) -> bool {
        let Some(watcher) = &self.fs_watcher else {
            return false;
        };

        let events = watcher.poll_events();
        if events.is_empty() {
            return false;
        }

        // Extract changed paths from events for incremental update
        let paths: Vec<_> = events
            .into_iter()
            .map(|e| match e {
                FileSystemEvent::Created(p)
                | FileSystemEvent::Modified(p)
                | FileSystemEvent::Deleted(p)
                | FileSystemEvent::Changed(p) => p,
            })
            .collect();

        // Dispatch FileSystemChange with the changed paths for incremental update
        if let Some(cmd) = update(
            &mut self.model,
            Msg::Workspace(WorkspaceMsg::FileSystemChange { paths }),
        ) {
            // Accumulate damage from file system change
            self.pending_damage.merge(cmd.damage());
            if cmd.needs_redraw() {
                return true;
            }
        }

        true
    }
}

/// Syntax highlighting worker thread loop
fn syntax_worker_loop(
    rx: Receiver<SyntaxWorkerRequest>,
    msg_tx: Sender<Msg>,
    event_proxy: Option<EventLoopProxy<()>>,
) {
    use std::collections::HashMap;

    tracing::info!("Syntax worker thread started");

    let mut parser_state = ParserState::new();
    let mut pending: HashMap<token::model::editor_area::DocumentId, SyntaxParseRequest> =
        HashMap::new();

    loop {
        // Wait for first request (blocking)
        match rx.recv() {
            Ok(req) => handle_syntax_worker_request(&mut pending, &mut parser_state, req),
            Err(_) => {
                tracing::info!("Syntax worker channel closed, exiting");
                return;
            }
        }

        // Drain any additional pending requests (non-blocking)
        // Keep only the latest request per document
        while let Ok(req) = rx.try_recv() {
            handle_syntax_worker_request(&mut pending, &mut parser_state, req);
        }

        // Process all pending requests
        for (_doc_id, req) in pending.drain() {
            tracing::debug!(
                "Worker parsing: doc={} rev={} lang={:?}",
                req.document_id.0,
                req.revision,
                req.language
            );

            let worker_started = Instant::now();
            let queue_ms = req.queued_at.elapsed().as_secs_f64() * 1000.0;
            let parse_started = Instant::now();
            let full_highlights = parser_state.parse_and_highlight(
                &req.source,
                req.language,
                req.document_id,
                req.revision,
            );
            let parse_highlight_ms = parse_started.elapsed().as_secs_f64() * 1000.0;
            let parser_timing = parser_state.last_timing();
            let replace_line_ranges = parser_state.last_changed_line_ranges();
            let highlights = parser_state
                .take_last_highlight_patch()
                .unwrap_or(full_highlights);
            let syntax_tree = parser_state.syntax_tree_snapshot(req.document_id, req.revision);

            // Extract outline from the cached tree (just parsed above)
            let outline_started = Instant::now();
            let outline = req.extract_outline.then(|| {
                parser_state
                    .get_cached_tree(req.document_id)
                    .map(|(tree, lang)| {
                        token::outline::extract_outline(tree, &req.source, lang, req.revision)
                    })
                    .unwrap_or_else(|| token::outline::OutlineData::empty(req.revision))
            });
            let outline_ms = outline_started.elapsed().as_secs_f64() * 1000.0;

            let line_count = highlights.lines.len();
            let token_count: usize = highlights.lines.values().map(|lh| lh.tokens.len()).sum();

            tracing::debug!(
                "Worker sending ParseCompleted: doc={} rev={} lines={} tokens={} outline={}",
                req.document_id.0,
                req.revision,
                line_count,
                token_count,
                outline.as_ref().map(|o| o.roots.len()).unwrap_or(0)
            );

            if let Err(e) = msg_tx.send(Msg::Syntax(SyntaxMsg::ParseCompleted {
                document_id: req.document_id,
                revision: req.revision,
                highlights,
                syntax_tree,
                outline,
                timing: Box::new(token::messages::SyntaxWorkerTiming {
                    snapshot_ms: req.snapshot_ms,
                    queue_ms,
                    parse_highlight_ms,
                    parse_ms: parser_timing.parse_ms,
                    highlight_ms: parser_timing.highlight_ms,
                    outline_ms,
                    worker_total_ms: worker_started.elapsed().as_secs_f64() * 1000.0,
                    outline_extracted: req.extract_outline,
                    highlighted_line_count: line_count,
                    replaced_range_count: replace_line_ranges.as_ref().map_or(0, Vec::len),
                }),
                replace_line_ranges,
            })) {
                tracing::warn!("Failed to send parse completion to main thread: {}", e);
            } else if let Some(proxy) = &event_proxy {
                let _ = proxy.send_event(());
            }
        }
    }
}

fn is_outline_panel_open(model: &AppModel) -> bool {
    model
        .dock_layout
        .active_panel_position(token::panel::PanelId::OUTLINE)
        .is_some()
}

fn handle_syntax_worker_request(
    pending: &mut HashMap<token::model::editor_area::DocumentId, SyntaxParseRequest>,
    parser_state: &mut ParserState,
    request: SyntaxWorkerRequest,
) {
    match request {
        SyntaxWorkerRequest::Parse(req) => {
            tracing::debug!(
                "Worker queued parse request: doc={} rev={} lang={:?}",
                req.document_id.0,
                req.revision,
                req.language
            );
            pending.insert(req.document_id, req);
        }
        SyntaxWorkerRequest::ClearDocument(document_id) => {
            tracing::debug!("Worker clearing cached syntax state: doc={}", document_id.0);
            pending.remove(&document_id);
            parser_state.clear_doc_cache(document_id);
        }
    }
}

/// Number of lines a single discrete mouse-wheel notch scrolls. Matches the
/// common editor default (VS Code, etc.).
const LINES_PER_WHEEL_NOTCH: f64 = 3.0;

/// Carries fractional scroll remainders between wheel events so trackpad
/// scrolling keeps a consistent, non-truncating sensitivity.
///
/// Without carrying the remainder, every high-resolution `PixelDelta` event
/// that moved less than a full line/column truncated to zero and the
/// fractional motion was lost, so slow scrolling either did nothing or snapped
/// a whole line at once — the "threshold feels off" symptom.
#[derive(Default)]
struct ScrollAccumulator {
    h: f64,
    v: f64,
}

impl ScrollAccumulator {
    /// Convert a raw winit mouse-wheel delta into integer `(h_delta, v_delta)`
    /// in the sign convention shared by `EditorState::scroll_vertical_by`,
    /// `EditorState::scroll_horizontal_visible_window_by`, and CSV's
    /// `scroll_horizontal`: a positive delta reveals further content (down for
    /// vertical, right for horizontal). winit reports both axes in the opposite
    /// sense, so both are negated here.
    ///
    /// `line_height` and `char_width` are in physical pixels (font size is
    /// scaled by the display's scale factor), matching winit's `PixelDelta`, so
    /// no scale conversion is needed. Sub-unit remainders are retained for the
    /// next `PixelDelta` event.
    fn deltas(
        &mut self,
        delta: winit::event::MouseScrollDelta,
        char_width: f64,
        line_height: f64,
    ) -> (i32, i32) {
        use winit::event::MouseScrollDelta;
        match delta {
            // Discrete mouse-wheel notches are already whole steps; no
            // sub-unit remainder to accumulate.
            MouseScrollDelta::LineDelta(x, y) => (
                (-x as f64 * LINES_PER_WHEEL_NOTCH) as i32,
                (-y as f64 * LINES_PER_WHEEL_NOTCH) as i32,
            ),
            MouseScrollDelta::PixelDelta(pos) => {
                self.h += -pos.x / char_width;
                self.v += -pos.y / line_height;
                // `trunc` toward zero keeps the fractional remainder with the
                // correct sign whether scrolling up or down.
                let h_steps = self.h.trunc();
                let v_steps = self.v.trunc();
                self.h -= h_steps;
                self.v -= v_steps;
                (h_steps as i32, v_steps as i32)
            }
        }
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
