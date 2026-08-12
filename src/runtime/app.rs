use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str::FromStr;
use std::sync::mpsc::{self, Receiver, Sender};
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
use token::commands::{Cmd, Damage, DamageArea};
use token::fs_watcher::{FileSystemEvent, FileSystemWatcher};
use token::keymap::{
    keystroke_from_winit, load_default_keymap, Command, KeyAction, KeyContext, Keymap,
};
use token::lsp::{self, client::ServerHandle, LspServerId, ServerState};
use token::messages::{
    AppMsg, EditorMsg, ImageMsg, LayoutMsg, LspMsg, ModalMsg, Msg, SyntaxMsg, UiMsg, WorkspaceMsg,
};
use token::model::editor::Position;
use token::model::AppModel;
use token::panel::DockPosition;
use token::syntax::{LanguageId, ParserState};
use token::update::update;

use super::input::{
    handle_cursor_overlay_key, handle_key, is_outline_dock_focused, KeyModifiers, OptionKeyGesture,
};
use super::mouse::{
    end_tab_drag, handle_mouse_press, handle_mouse_wheel, make_mouse_event, update_tab_drag,
    ClickTracker, DragState,
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
    source: String,
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
    let terminal_focused = model.ui.focused_dock() == Some(token::panel::DockPosition::Bottom)
        && model.dock_layout.bottom.is_open
        && model.dock_layout.bottom.active_panel() == Some(token::panel::PanelId::TERMINAL);

    model.ui.has_modal()
        || (option_double_tapped && alt_pressed)
        || sidebar_focused
        || is_outline_dock_focused(model)
        || terminal_focused
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
    /// A sender into `automation_rx`, kept around only so tests can push
    /// requests through the exact same `process_automation_requests` path
    /// the socket/MCP server feeds in production, without standing up a
    /// real socket.
    #[cfg(test)]
    automation_tx: Sender<AutomationEnvelope>,
    automation_profile: Option<AutomationProfile>,
    syntax_scheduled: HashMap<(token::model::editor_area::DocumentId, u64), Instant>,
    syntax_present_pending: Vec<SyntaxPresentationPending>,
    automation_syntax_profile: Option<AutomationSyntaxProfile>,
    latest_syntax_performance: Option<crate::automation::SyntaxPerfSnapshot>,
    lsp: LspManager,
    /// Wakes the event loop from an LSP worker thread; `None` in tests
    /// that construct `App` without a real event loop (matches
    /// `automation_proxy`'s optionality).
    lsp_wake: Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
}

struct AutomationProfile {
    remaining_frames: usize,
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
}

impl LspManager {
    fn new() -> Self {
        Self {
            servers: HashMap::new(),
            detached_roots: Vec::new(),
            restart_attempts: HashMap::new(),
            failed_roots: HashMap::new(),
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

    fn kill_all(&mut self) {
        for handle in self.servers.values_mut() {
            handle.kill();
        }
        self.servers.clear();
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
        #[cfg(test)]
        let automation_tx_for_tests = automation_tx.clone();
        if let Some(proxy) = automation_proxy.clone() {
            automation::start_server(automation_tx, proxy);
        }
        // LSP worker threads wake the event loop the same way the syntax
        // worker does; cloned before `automation_proxy` is moved below.
        let lsp_wake: Option<std::sync::Arc<dyn Fn() + Send + Sync>> =
            automation_proxy.clone().map(|proxy| {
                std::sync::Arc::new(move || {
                    let _ = proxy.send_event(());
                }) as std::sync::Arc<dyn Fn() + Send + Sync>
            });
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
            #[cfg(test)]
            automation_tx: automation_tx_for_tests,
            automation_profile: None,
            syntax_scheduled: HashMap::new(),
            syntax_present_pending: Vec::new(),
            automation_syntax_profile: None,
            latest_syntax_performance: None,
            lsp: LspManager::new(),
            lsp_wake,
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
            .map(|(&id, doc)| (id, doc.revision, doc.buffer.to_string(), doc.language))
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
    }

    /// Dispatch a command through the update loop
    fn dispatch_command(&mut self, command: Command) -> Option<Cmd> {
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
    fn update_cursor_icon(&mut self, x: f64, y: f64) {
        use token::model::HoverRegion;
        use token::view::hit_test::{hit_test_ui, Point};

        let Some(window) = &self.window else { return };
        let Some(renderer) = &self.renderer else {
            return;
        };

        // In-progress sidebar resize overrides all hit-testing
        if self.model.ui.sidebar_resize.is_some() {
            self.model.ui.hover = HoverRegion::SidebarResize;
            window.set_cursor(CursorIcon::ColResize);
            return;
        }

        // In-progress dock resize overrides hit-testing
        if let Some(ref resize_state) = self.model.ui.dock_resize {
            self.model.ui.hover = HoverRegion::DockResize(resize_state.position);
            let icon = match resize_state.axis {
                token::model::ui::DockResizeAxis::Horizontal => CursorIcon::ColResize,
                token::model::ui::DockResizeAxis::Vertical => CursorIcon::RowResize,
            };
            window.set_cursor(icon);
            return;
        }

        let pt = Point::new(x, y);
        if let Some(target) = hit_test_ui(&self.model, pt, renderer.char_width()) {
            window.set_cursor(target.cursor_icon());
            self.model.ui.hover = target.hover_region();
            self.model.ui.modal_hover_row =
                if let token::view::hit_test::HitTarget::ModalRow { flat_index } = target {
                    Some(flat_index)
                } else {
                    None
                };
        } else {
            self.model.ui.hover = HoverRegion::None;
            self.model.ui.modal_hover_row = None;
            window.set_cursor(CursorIcon::Default);
        }
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
                    // Up/Down/Enter/Esc/Tab (overlay-surface.md Phase 5) and must
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
                self.mouse_position = Some((position.x, position.y));
                let prev_modal_hover_row = self.model.ui.modal_hover_row;
                self.update_cursor_icon(position.x, position.y);

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
                    return (self.model.ui.modal_hover_row != prev_modal_hover_row)
                        .then_some(Cmd::Redraw);
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
                None
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
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
                button: MouseButton::Middle,
                ..
            } => {
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
        use token::markdown::{content_to_preview_html, PreviewTheme};
        use token::model::editor_area::PreviewId;
        use token::syntax::LanguageId;
        use token::view::geometry::PreviewPaneLayout;

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
                    PreviewPaneLayout::new(preview.rect, metrics).hosted_content_rect();

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
                self.restart_lsp_server(&server_id);
            }

            // =====================================================================
            // Application Commands
            // =====================================================================
            Cmd::Quit => {
                // No quit-time teardown existed anywhere in the runtime
                // before this (not even for PTY children); this is the
                // first one. It only kills the child here rather than
                // running the full shutdown/exit handshake from the
                // design doc — see `ServerHandle::kill`'s doc comment.
                self.lsp.kill_all();
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
        while let Ok(msg) = self.msg_rx.try_recv() {
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
                self.lsp.restart_attempts.remove(&(server_id, root));
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

    /// Spawns a server for `language` rooted for `file_path`, if one is
    /// registered, enabled, and not already running for that root
    /// (`Cmd::LspEnsureServer`; see design doc's Process Model — lazy
    /// spawn on first matching `didOpen`, wired by the next unit).
    fn ensure_lsp_server(&mut self, language: token::syntax::LanguageId, file_path: &Path) {
        let Some(def) = lsp::lsp_server_def(language) else {
            return;
        };
        let Some(resolved) = lsp::resolve_server(def, &self.model.config.lsp) else {
            return;
        };
        let workspace_root = self.model.workspace.as_ref().map(|w| w.root.as_path());
        let root = lsp::client::resolve_root(file_path, workspace_root, def.project_markers, |p| {
            p.is_file()
        });

        if self.lsp.is_running(&resolved.id, &root) {
            return;
        }

        let is_detached = workspace_root.is_none_or(|ws| !root.starts_with(ws));
        if is_detached && !self.lsp.detached_roots.contains(&root) {
            if self.lsp.detached_roots.len() >= MAX_DETACHED_ROOTS {
                tracing::debug!(
                    "Detached LSP root cap ({MAX_DETACHED_ROOTS}) reached; not spawning {} for {}",
                    resolved.id,
                    root.display()
                );
                return;
            }
            self.lsp.detached_roots.push(root.clone());
        }

        self.spawn_lsp_server_at(&resolved, &root);
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
        for root in roots {
            if let Some(mut handle) = self.lsp.servers.remove(&(server_id.clone(), root.clone())) {
                handle.kill();
            }
            self.spawn_lsp_server_at(&resolved, &root);
        }
    }

    fn spawn_lsp_server_at(&mut self, resolved: &lsp::ResolvedServer, root: &Path) {
        self.set_lsp_server_state(resolved.id.clone(), root, ServerState::Starting);
        match lsp::client::spawn_server(
            &resolved.command,
            &resolved.args,
            root,
            resolved.id.clone(),
            self.msg_tx.clone(),
            self.lsp_wake.clone(),
        ) {
            Ok(handle) => {
                self.lsp
                    .servers
                    .insert((resolved.id.clone(), root.to_path_buf()), handle);
            }
            Err(e) => {
                tracing::warn!("Failed to spawn LSP server {}: {}", resolved.id, e);
                self.set_lsp_server_state(resolved.id.clone(), root, ServerState::Missing);
            }
        }
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
    }

    /// A server's worker thread hit EOF/error on stdout — the child
    /// exited (crash or otherwise). Removes the dead handle and retries
    /// with a simple attempt cap; giving up reports `Failed`.
    ///
    /// ponytail: no exponential delay between attempts (the design doc
    /// specifies backoff); this retries immediately, capped at
    /// `MAX_RESTART_ATTEMPTS`. Add a delay if a flapping server ever
    /// makes that matter in practice.
    fn handle_lsp_server_exited(&mut self, server_id: &LspServerId, generation: u64) {
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
            self.lsp.servers.remove(&(server_id.clone(), root.clone()));
        }

        let Some(def) = lsp::server_def_by_id(&server_id.0) else {
            return;
        };
        let Some(resolved) = lsp::resolve_server(def, &self.model.config.lsp) else {
            return;
        };
        for root in roots {
            let key = (server_id.clone(), root.clone());
            let entry = self.lsp.restart_attempts.entry(key).or_insert(0);
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
            self.spawn_lsp_server_at(&resolved, &root);
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
            && self.model.dock_layout.bottom.is_open
            && self.model.dock_layout.bottom.active_panel() == Some(token::panel::PanelId::TERMINAL)
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
            event_loop.exit();
        } else if should_redraw {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let mut needs_redraw = false;

        if self.process_automation_requests() {
            needs_redraw = true;
        }

        if self.process_async_messages() {
            needs_redraw = true;
        }

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

        // Check syntax debounce deadlines
        if self.check_syntax_deadlines() {
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

        if time_since_tick >= blink_interval {
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
        // Calculate next wake time: earliest of blink timer and syntax deadlines
        let mut next_wake = self.last_tick + blink_interval;
        if let Some(earliest_deadline) = self.syntax_deadlines.values().map(|(d, _)| *d).min() {
            next_wake = next_wake.min(earliest_deadline);
        }
        if let Some(transient) = &self.model.ui.transient_message {
            next_wake = next_wake.min(transient.expires_at);
        }
        if let Some(deferred_startup_at) = self.deferred_startup_at {
            next_wake = next_wake.min(deferred_startup_at);
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(next_wake));
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

    fn automation_response(&self, message: &str) -> AutomationResponse {
        let mut response = AutomationResponse::success(
            message,
            crate::automation::EditorSnapshot::from_model(&self.model),
            self.perf.snapshot(),
        );
        response.syntax_performance = self.latest_syntax_performance.clone();
        response
    }
}

fn clamped_document_position(model: &AppModel, line: usize, column: usize) -> (usize, usize) {
    let line = line.min(model.document().line_count().saturating_sub(1));
    (line, column.min(model.document().line_length(line)))
}

impl App {
    /// Check syntax debounce deadlines and fire ParseReady for expired ones
    fn check_syntax_deadlines(&mut self) -> bool {
        if self.syntax_deadlines.is_empty() {
            return false;
        }
        let now = Instant::now();
        let expired: Vec<_> = self
            .syntax_deadlines
            .iter()
            .filter(|(_, (deadline, _))| now >= *deadline)
            .map(|(&doc_id, &(_, revision))| (doc_id, revision))
            .collect();

        if expired.is_empty() {
            return false;
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
                if cmd.needs_redraw() {
                    needs_redraw = true;
                }
                self.pending_damage.merge(cmd.damage());
                self.process_cmd(cmd);
            }
        }
        needs_redraw
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

#[cfg(test)]
mod tests {
    use super::*;
    use token::cli::{StartupConfig, StartupMode};
    use token::outline::{OutlineData, OutlineKind, OutlineNode, OutlineRange};

    fn empty_startup_config() -> StartupConfig {
        StartupConfig {
            mode: StartupMode::Empty,
            initial_position: None,
            wait_mode: false,
        }
    }

    fn focus_outline_with_symbols(app: &mut App) {
        app.model
            .dock_layout
            .right
            .activate(token::panel::PanelId::OUTLINE);
        app.model.ui.focus_dock(token::panel::DockPosition::Right);
        let revision = app.model.document().revision;
        app.model.document_mut().outline = Some(OutlineData {
            revision,
            roots: [1, 2]
                .into_iter()
                .map(|line| OutlineNode {
                    kind: OutlineKind::Function,
                    name: format!("symbol_{line}"),
                    range: OutlineRange {
                        start_line: line,
                        start_col: 0,
                        end_line: line,
                        end_col: 1,
                    },
                    children: Vec::new(),
                })
                .collect(),
        });
    }

    fn dispatch_legacy_key(app: &mut App, key: Key) -> Option<Cmd> {
        handle_key(
            &mut app.model,
            key,
            PhysicalKey::Unidentified(winit::keyboard::NativeKeyCode::Unidentified),
            KeyModifiers::default(),
            false,
        )
    }

    #[test]
    fn focused_outline_bypasses_editor_keymap_and_captures_navigation() {
        let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
        app.model
            .document_mut()
            .buffer
            .insert(0, "zero\none\ntwo\n");
        focus_outline_with_symbols(&mut app);
        let editor_cursor = app.model.editor().primary_cursor();
        let editor_position = (editor_cursor.line, editor_cursor.column);
        let document_text = app.model.document().buffer.to_string();

        assert!(should_skip_non_global_keymap(&app.model, false, false));

        dispatch_legacy_key(&mut app, Key::Named(NamedKey::ArrowDown));
        assert_eq!(app.model.outline_panel.selected_index, Some(0));
        dispatch_legacy_key(&mut app, Key::Named(NamedKey::ArrowDown));
        assert_eq!(app.model.outline_panel.selected_index, Some(1));
        let editor_cursor = app.model.editor().primary_cursor();
        assert_eq!((editor_cursor.line, editor_cursor.column), editor_position);

        dispatch_legacy_key(&mut app, Key::Character("x".into()));
        assert_eq!(app.model.document().buffer.to_string(), document_text);

        dispatch_legacy_key(&mut app, Key::Named(NamedKey::Enter));
        assert_eq!(app.model.editor().primary_cursor().line, 2);
        assert!(matches!(
            app.model.ui.focus,
            token::model::FocusTarget::Editor
        ));

        app.model.ui.focus_dock(token::panel::DockPosition::Right);
        dispatch_legacy_key(&mut app, Key::Named(NamedKey::Escape));
        assert!(matches!(
            app.model.ui.focus,
            token::model::FocusTarget::Editor
        ));
    }

    #[test]
    fn multiple_startup_files_open_as_distinct_tabs() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let first = directory.path().join("first.rs");
        let second = directory.path().join("second.py");
        std::fs::write(&first, "fn main() {}\n").expect("first fixture should be written");
        std::fs::write(&second, "print('hello')\n").expect("second fixture should be written");
        let config = StartupConfig {
            mode: StartupMode::MultipleFiles(vec![first.clone(), second.clone()]),
            initial_position: None,
            wait_mode: false,
        };
        let preparation = AppPreparation::start(800, 600, config.clone())
            .expect("application preparation thread should start");

        let app = App::new(800, 600, config, None, None, Some(preparation));
        let open_paths: std::collections::HashSet<_> = app
            .model
            .editor_area
            .documents
            .values()
            .filter_map(|document| document.file_path.as_ref())
            .collect();
        let tab_count: usize = app
            .model
            .editor_area
            .groups
            .values()
            .map(|group| group.tabs.len())
            .sum();

        assert_eq!(tab_count, 2);
        assert!(open_paths.contains(&first));
        assert!(open_paths.contains(&second));
    }

    #[test]
    fn spawn_terminal_command_adds_session_to_model() {
        use std::time::{Duration, Instant};

        let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
        app.model
            .dock_layout
            .bottom
            .activate(token::panel::PanelId::TERMINAL);

        app.process_cmd(Cmd::SpawnTerminal {
            session_id: 42,
            rows: 12,
            cols: 34,
        });

        let deadline = Instant::now() + Duration::from_secs(5);
        let session = loop {
            app.process_terminal_spawn_results();
            if let Some(session) = app.model.terminal.sessions.iter().find(|s| s.id == 42) {
                break session;
            }
            if Instant::now() >= deadline {
                panic!("spawned terminal session should be stored in the model");
            }
            std::thread::sleep(Duration::from_millis(10));
        };

        assert_eq!(session.size, (12, 34));
        assert_eq!(app.model.terminal.active, 0);

        session.pty.write(b"exit\n".to_vec());
    }

    #[test]
    fn terminal_spawn_result_is_discarded_when_terminal_is_closed() {
        let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
        let (spawn_tx, spawn_rx) = mpsc::channel();
        let (pty, _pty_rx) = token::terminal::PtyHandle::new_for_test();

        spawn_tx
            .send(Ok(token::terminal::TerminalSpawnResult {
                session_id: 99,
                rows: 24,
                cols: 80,
                pty,
            }))
            .expect("test spawn result should send");
        app.terminal_spawn_rx = Some((99, spawn_rx));
        app.model.terminal.mark_spawn_pending(99);

        let needs_redraw = app.process_terminal_spawn_results();

        assert!(!needs_redraw);
        assert!(app.model.terminal.sessions.is_empty());
        assert!(!app.model.terminal.is_spawn_pending(99));
    }

    #[test]
    fn ignored_terminal_spawn_command_clears_its_pending_marker() {
        let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
        let (_spawn_tx, spawn_rx) = mpsc::channel();
        app.terminal_spawn_rx = Some((6, spawn_rx));
        app.model.terminal.mark_spawn_pending(6);
        app.model.terminal.mark_spawn_pending(7);

        app.process_cmd(Cmd::SpawnTerminal {
            session_id: 7,
            rows: 24,
            cols: 80,
        });

        assert!(!app.model.terminal.is_spawn_pending(7));
        assert!(app.model.terminal.is_spawn_pending(6));
    }

    #[test]
    fn duplicate_terminal_spawn_command_preserves_in_flight_pending_marker() {
        let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
        let (_spawn_tx, spawn_rx) = mpsc::channel();
        app.terminal_spawn_rx = Some((7, spawn_rx));
        app.model.terminal.mark_spawn_pending(7);

        app.process_cmd(Cmd::SpawnTerminal {
            session_id: 7,
            rows: 24,
            cols: 80,
        });

        assert!(app.model.terminal.is_spawn_pending(7));
    }

    /// Push a request through `automation_tx` -> `automation_rx` and drain
    /// it with `process_automation_requests`, the same path a real socket
    /// client (MCP tool, CLI) drives — exercises `AutomationRequest`
    /// end-to-end instead of calling `update()` directly.
    fn send_automation_request(app: &mut App, request: AutomationRequest) -> AutomationResponse {
        let (response_tx, response_rx) = mpsc::sync_channel(1);
        app.automation_tx
            .send(AutomationEnvelope {
                request,
                response_tx,
            })
            .expect("automation channel should still be open");
        app.process_automation_requests();
        response_rx
            .try_recv()
            .expect("a response should have been sent for the request")
    }

    #[test]
    fn automation_flow_triggers_menu_and_reports_completion_snapshot() {
        // autocomplete.md Phase 1 Gate: type -> menu opens -> filter,
        // driven through the actual `AutomationRequest` socket path
        // (`EditorSnapshot.completion` exists for exactly this).
        let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
        app.model
            .editor_area
            .documents
            .values_mut()
            .next()
            .unwrap()
            .buffer
            .insert(0, "value_one\nval");

        let response = send_automation_request(
            &mut app,
            AutomationRequest::SetCursor { line: 1, column: 3 },
        );
        assert!(response.ok);
        let state = response.state.expect("state should be present");
        assert!(
            state.completion.is_none(),
            "cursor move alone shouldn't open the popup"
        );

        let response = send_automation_request(
            &mut app,
            AutomationRequest::ExecuteAction {
                name: "TriggerCompletionMenu".to_string(),
            },
        );
        assert!(response.ok, "{}", response.message);

        let response = send_automation_request(&mut app, AutomationRequest::State);
        let completion = response
            .state
            .expect("state should be present")
            .completion
            .expect("completion popup should be open after the trigger action");
        assert!(completion.items.contains(&"value_one".to_string()));
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
    let dock = &model.dock_layout.right;
    dock.is_open && dock.active_panel() == Some(token::panel::PanelId::OUTLINE)
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
mod mouse_wheel_tests {
    use super::ScrollAccumulator;
    use winit::dpi::PhysicalPosition;
    use winit::event::MouseScrollDelta;

    #[test]
    fn horizontal_and_vertical_line_delta_negate_symmetrically() {
        let mut accum = ScrollAccumulator::default();
        let (h, v) = accum.deltas(MouseScrollDelta::LineDelta(1.0, 1.0), 8.0, 16.0);
        // Regression test: horizontal scroll used to pass `x` through
        // unnegated while vertical negated `y`, inverting horizontal scroll
        // direction relative to vertical (and relative to the "positive
        // delta reveals further content" convention both axes share in the
        // model layer). Both axes must now negate the same way.
        assert_eq!(h, -3);
        assert_eq!(v, -3);
    }

    #[test]
    fn horizontal_and_vertical_pixel_delta_negate_symmetrically() {
        let mut accum = ScrollAccumulator::default();
        let (h, v) = accum.deltas(
            MouseScrollDelta::PixelDelta(PhysicalPosition::new(16.0, 32.0)),
            8.0,
            16.0,
        );
        assert_eq!(h, -2);
        assert_eq!(v, -2);
    }

    #[test]
    fn sub_line_pixel_deltas_accumulate_instead_of_truncating_to_zero() {
        let mut accum = ScrollAccumulator::default();
        // Each event moves less than a full line (16px); previously every one
        // truncated to 0 and the motion was lost entirely.
        let mut emitted = 0;
        for _ in 0..4 {
            let (_, v) = accum.deltas(
                MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, 6.0)),
                8.0,
                16.0,
            );
            emitted += v;
        }
        // 4 × 6px = 24px ≈ 1.5 lines → one line emitted, remainder carried.
        assert_eq!(emitted, -1);
    }
}
