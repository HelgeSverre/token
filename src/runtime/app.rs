use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use anyhow::Result;
use softbuffer::Context;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
#[cfg(debug_assertions)]
use winit::keyboard::{Key, NamedKey};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorIcon, Icon, Window};

use token::cli::StartupConfig;
use token::commands::{Cmd, Damage};
use token::fs_watcher::{FileSystemEvent, FileSystemWatcher};
use token::keymap::{
    keystroke_from_winit, load_default_keymap, Command, KeyAction, KeyContext, Keymap,
};
use token::messages::{AppMsg, CsvMsg, EditorMsg, LayoutMsg, Msg, SyntaxMsg, UiMsg, WorkspaceMsg};
use token::model::editor::Position;
use token::model::editor_area::{Rect, SplitDirection};
use token::model::AppModel;
use token::syntax::{LanguageId, ParserState};
use token::update::update;

use crate::view::geometry::is_in_group_tab_bar;

use super::input::handle_key;
use super::mouse::{handle_mouse_press, make_mouse_event, ClickTracker};
use super::webview::WebviewManager;
use crate::view::Renderer;

use super::perf::PerfStats;

use winit::keyboard::ModifiersState;

/// Request sent to syntax worker thread
struct SyntaxParseRequest {
    document_id: token::model::editor_area::DocumentId,
    revision: u64,
    source: String,
    language: LanguageId,
}

pub struct App {
    model: AppModel,
    keymap: Keymap,
    renderer: Option<Renderer>,
    window: Option<Rc<Window>>,
    context: Option<Context<Rc<Window>>>,
    last_tick: Instant,
    modifiers: ModifiersState,
    mouse_position: Option<(f64, f64)>,
    last_option_press: Option<Instant>,
    option_double_tapped: bool,
    left_mouse_down: bool,
    last_auto_scroll: Option<Instant>,
    drag_start_position: Option<(f64, f64)>,
    drag_active: bool,
    msg_tx: Sender<Msg>,
    msg_rx: Receiver<Msg>,
    perf: PerfStats,
    /// Channel to send parse requests to syntax worker
    syntax_tx: Sender<SyntaxParseRequest>,
    /// File system watcher for workspace directory (if workspace is open)
    fs_watcher: Option<FileSystemWatcher>,
    /// Pending damage for the next render (accumulated from commands)
    pending_damage: Damage,
    /// Flag to request application exit (set by Cmd::Quit)
    should_quit: bool,
    /// Webview manager for markdown preview
    webview_manager: WebviewManager,
    /// Click tracker for unified mouse event handling
    click_tracker: ClickTracker,
}

impl App {
    pub fn new(window_width: u32, window_height: u32, startup_config: StartupConfig) -> Self {
        let (msg_tx, msg_rx) = mpsc::channel();
        let keymap = Keymap::with_bindings(load_default_keymap());

        // Spawn syntax highlighting worker thread
        let (syntax_tx, syntax_rx) = mpsc::channel::<SyntaxParseRequest>();
        {
            let msg_tx_clone = msg_tx.clone();
            std::thread::spawn(move || syntax_worker_loop(syntax_rx, msg_tx_clone));
        }

        // Extract file paths and workspace from config
        let file_paths = startup_config.file_paths();
        let workspace_root = startup_config.workspace_root().cloned();
        let initial_position = startup_config.initial_position;

        let mut model = AppModel::new(window_width, window_height, 1.0, file_paths);

        // Open workspace if specified and start file watcher
        let fs_watcher = if let Some(root) = workspace_root {
            model.open_workspace(root.clone());
            // Start file system watcher for the workspace
            match FileSystemWatcher::new(root) {
                Ok(watcher) => Some(watcher),
                Err(e) => {
                    tracing::warn!("Failed to start file system watcher: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // Apply initial cursor position if specified (--line/--column)
        if let Some((line, column)) = initial_position {
            let editor = model.editor_mut();
            editor.cursors[0].line = line;
            editor.cursors[0].column = column;
            editor.selections[0].anchor = Position::new(line, column);
            editor.selections[0].head = Position::new(line, column);
            model.ensure_cursor_visible();
        }

        let mut app = Self {
            model,
            keymap,
            renderer: None,
            window: None,
            context: None,
            last_tick: Instant::now(),
            modifiers: ModifiersState::empty(),
            mouse_position: None,
            last_option_press: None,
            option_double_tapped: false,
            left_mouse_down: false,
            last_auto_scroll: None,
            drag_start_position: None,
            drag_active: false,
            msg_tx,
            msg_rx,
            perf: PerfStats::default(),
            syntax_tx,
            fs_watcher,
            pending_damage: Damage::Full, // Start with full render
            should_quit: false,
            webview_manager: WebviewManager::new(),
            click_tracker: ClickTracker::default(),
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
            let _ = self.syntax_tx.send(SyntaxParseRequest {
                document_id: doc_id,
                revision,
                source,
                language,
            });
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
            sidebar_focused: matches!(focus, FocusTarget::Sidebar),
        }
    }

    fn init_renderer(&mut self, window: Rc<Window>, context: &Context<Rc<Window>>) -> Result<()> {
        let renderer = Renderer::new(Rc::clone(&window), context)?;

        self.model.set_char_width(renderer.char_width());
        self.model.set_scale_factor(renderer.scale_factor());
        self.model.line_height = renderer.line_height();

        // Derive tab bar height from glyph metrics instead of hardcoded value
        self.model.recompute_tab_bar_height_from_line_height();

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

        // Recompute viewport geometry for new char_width/line_height
        let size = window.inner_size();
        self.model.resize(size.width, size.height);

        self.renderer = Some(renderer);
        Ok(())
    }

    fn try_auto_scroll_for_drag(&mut self, y: f64) -> Option<Cmd> {
        const AUTO_SCROLL_INTERVAL_MS: u64 = 80;

        let line_height = self.model.line_height as f64;
        let window_height = self.model.window_size.1 as f64;
        let status_bar_top = window_height - line_height;

        let scroll_direction = if y < 0.0 {
            Some(-1)
        } else if y >= status_bar_top {
            Some(1)
        } else {
            None
        };

        let direction = scroll_direction?;

        let now = Instant::now();
        if let Some(last) = self.last_auto_scroll {
            if now.duration_since(last) < Duration::from_millis(AUTO_SCROLL_INTERVAL_MS) {
                return None;
            }
        }

        self.last_auto_scroll = Some(now);
        update(&mut self.model, Msg::Editor(EditorMsg::Scroll(direction)))
    }

    /// Update both hover region tracking and cursor icon based on mouse position.
    /// This ensures hover state and cursor icons are always in sync.
    fn update_cursor_icon(&mut self, x: f64, y: f64) {
        use token::model::HoverRegion;

        let Some(window) = &self.window else { return };
        let Some(renderer) = &self.renderer else {
            return;
        };

        let status_bar_height = renderer.line_height();
        let (width, height) = renderer.dimensions();

        // Calculate sidebar dimensions
        let sidebar_width = self
            .model
            .workspace
            .as_ref()
            .filter(|ws| ws.sidebar_visible)
            .map(|ws| ws.sidebar_width(self.model.metrics.scale_factor))
            .unwrap_or(0.0);

        // Sidebar resize in progress → always show ColResize, keep current hover
        if self.model.ui.sidebar_resize.is_some() {
            self.model.ui.hover = HoverRegion::SidebarResize;
            window.set_cursor(CursorIcon::ColResize);
            return;
        }

        // Modal overlay → Modal hover region
        if self.model.ui.has_modal() {
            self.model.ui.hover = HoverRegion::Modal;
            window.set_cursor(CursorIcon::Default);
            return;
        }

        // Status bar has highest priority - "on top" of everything
        if renderer.is_in_status_bar(y) {
            self.model.ui.hover = HoverRegion::StatusBar;
            window.set_cursor(CursorIcon::Default);
            return;
        }

        // Sidebar resize border (right edge, ~4px hit zone)
        const RESIZE_HIT_ZONE: f64 = 4.0;
        if sidebar_width > 0.0 {
            let resize_zone_start = sidebar_width as f64 - RESIZE_HIT_ZONE;
            let resize_zone_end = sidebar_width as f64 + RESIZE_HIT_ZONE;
            if x >= resize_zone_start && x <= resize_zone_end {
                self.model.ui.hover = HoverRegion::SidebarResize;
                window.set_cursor(CursorIcon::ColResize);
                return;
            }

            // Sidebar file tree area
            if x < sidebar_width as f64 {
                self.model.ui.hover = HoverRegion::Sidebar;
                window.set_cursor(CursorIcon::Default);
                return;
            }
        }

        // Calculate dock dimensions
        let scale_factor = self.model.metrics.scale_factor;
        let right_dock_width = self.model.dock_layout.right.size(scale_factor) as f64;
        let bottom_dock_height = self.model.dock_layout.bottom.size(scale_factor) as f64;
        let content_height = (height as usize).saturating_sub(status_bar_height) as f64;

        // Right dock resize handle (left edge)
        if self.model.dock_layout.right.is_open && right_dock_width > 0.0 {
            let right_dock_x = width as f64 - right_dock_width;
            let resize_zone_start = right_dock_x - RESIZE_HIT_ZONE;
            let resize_zone_end = right_dock_x + RESIZE_HIT_ZONE;
            if x >= resize_zone_start && x <= resize_zone_end && y < content_height - bottom_dock_height {
                self.model.ui.hover = HoverRegion::DockResize(token::panel::DockPosition::Right);
                window.set_cursor(CursorIcon::ColResize);
                return;
            }
        }

        // Bottom dock resize handle (top edge)
        if self.model.dock_layout.bottom.is_open && bottom_dock_height > 0.0 {
            let bottom_dock_y = content_height - bottom_dock_height;
            let resize_zone_start = bottom_dock_y - RESIZE_HIT_ZONE;
            let resize_zone_end = bottom_dock_y + RESIZE_HIT_ZONE;
            if y >= resize_zone_start && y <= resize_zone_end && x >= sidebar_width as f64 {
                self.model.ui.hover = HoverRegion::DockResize(token::panel::DockPosition::Bottom);
                window.set_cursor(CursorIcon::RowResize);
                return;
            }
        }

        // Right dock content area
        if self.model.dock_layout.right.is_open && right_dock_width > 0.0 {
            let right_dock_x = width as f64 - right_dock_width;
            if x >= right_dock_x && y < content_height - bottom_dock_height {
                self.model.ui.hover = HoverRegion::Dock(token::panel::DockPosition::Right);
                window.set_cursor(CursorIcon::Default);
                return;
            }
        }

        // Bottom dock content area
        if self.model.dock_layout.bottom.is_open && bottom_dock_height > 0.0 {
            let bottom_dock_y = content_height - bottom_dock_height;
            if y >= bottom_dock_y && x >= sidebar_width as f64 {
                self.model.ui.hover = HoverRegion::Dock(token::panel::DockPosition::Bottom);
                window.set_cursor(CursorIcon::Default);
                return;
            }
        }

        // Check preview panes
        for preview in self.model.editor_area.previews.values() {
            if preview.rect.contains(x as f32, y as f32) {
                self.model.ui.hover = HoverRegion::Preview;
                window.set_cursor(CursorIcon::Default);
                return;
            }
        }

        // Compute splitter layout for hit testing
        let available_rect = Rect::new(
            sidebar_width,
            0.0,
            (width as f32) - sidebar_width,
            (height as usize).saturating_sub(status_bar_height) as f32,
        );
        let splitter_width = self.model.metrics.splitter_width;
        let splitters = self
            .model
            .editor_area
            .compute_layout_scaled(available_rect, splitter_width);

        // Splitter bars
        if let Some(idx) = self
            .model
            .editor_area
            .splitter_at_point(&splitters, x as f32, y as f32)
        {
            self.model.ui.hover = HoverRegion::Splitter;
            let icon = match splitters[idx].direction {
                SplitDirection::Horizontal => CursorIcon::ColResize,
                SplitDirection::Vertical => CursorIcon::RowResize,
            };
            window.set_cursor(icon);
            return;
        }

        // Tab bars
        let tab_bar_height = self.model.metrics.tab_bar_height;
        for group in self.model.editor_area.groups.values() {
            if is_in_group_tab_bar(y, &group.rect, tab_bar_height)
                && x >= group.rect.x as f64
                && x < (group.rect.x + group.rect.width) as f64
            {
                self.model.ui.hover = HoverRegion::EditorTabBar;
                window.set_cursor(CursorIcon::Default);
                return;
            }
        }

        // Gutter area (line numbers)
        let gutter_width =
            token::model::gutter_border_x_scaled(renderer.char_width(), &self.model.metrics) as f64;
        if let Some(group) = self.model.editor_area.focused_group() {
            let gutter_x_end = group.rect.x as f64 + gutter_width;
            let content_y_start = group.rect.y as f64 + tab_bar_height as f64;
            if x >= group.rect.x as f64 && x < gutter_x_end && y >= content_y_start {
                // Gutter is part of the editor, but use default pointer
                self.model.ui.hover = HoverRegion::EditorText;
                window.set_cursor(CursorIcon::Default);
                return;
            }
        }

        // Editor text area
        self.model.ui.hover = HoverRegion::EditorText;
        window.set_cursor(CursorIcon::Text);
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
                        let now = Instant::now();
                        if let Some(last) = self.last_option_press {
                            if now.duration_since(last) < Duration::from_millis(300) {
                                self.option_double_tapped = true;
                            }
                        }
                        self.last_option_press = Some(now);
                    } else if event.state == ElementState::Released {
                        self.option_double_tapped = false;
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

                    let ctrl = self.modifiers.control_key();
                    let shift = self.modifiers.shift_key();
                    let alt = self.modifiers.alt_key();
                    let logo = self.modifiers.super_key();

                    // Check for global commands first (work regardless of focus state)
                    // These include command palette, save, quit, etc.
                    if let Some(keystroke) = keystroke_from_winit(
                        &event.logical_key,
                        event.physical_key,
                        ctrl,
                        shift,
                        alt,
                        logo,
                    ) {
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
                    // - Not editing a CSV cell (CSV cell editor handled by handle_csv_edit_key in input.rs)
                    let sidebar_focused =
                        matches!(self.model.ui.focus, token::model::FocusTarget::Sidebar);
                    let skip_keymap = self.model.ui.has_modal()
                        || (self.option_double_tapped && alt)
                        || sidebar_focused
                        || self.model.is_csv_editing();

                    if !skip_keymap {
                        if let Some(keystroke) = keystroke_from_winit(
                            &event.logical_key,
                            event.physical_key,
                            ctrl,
                            shift,
                            alt,
                            logo,
                        ) {
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
                        ctrl,
                        shift,
                        alt,
                        logo,
                        self.option_double_tapped,
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
                self.update_cursor_icon(position.x, position.y);

                // Handle splitter drag first (highest priority)
                if self.model.ui.splitter_drag.is_some() {
                    return update(
                        &mut self.model,
                        Msg::Layout(LayoutMsg::UpdateSplitterDrag {
                            position: (position.x as f32, position.y as f32),
                        }),
                    );
                }

                // Handle sidebar resize drag
                if self.model.ui.sidebar_resize.is_some() {
                    return update(
                        &mut self.model,
                        Msg::Workspace(WorkspaceMsg::UpdateSidebarResize { x: position.x }),
                    );
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
                } else if self.left_mouse_down {
                    const DRAG_THRESHOLD_PIXELS: f64 = 4.0;

                    if let Some(renderer) = &mut self.renderer {
                        if !self.drag_active {
                            if let Some((start_x, start_y)) = self.drag_start_position {
                                let dx = position.x - start_x;
                                let dy = position.y - start_y;
                                let distance = (dx * dx + dy * dy).sqrt();

                                if distance >= DRAG_THRESHOLD_PIXELS {
                                    self.drag_active = true;
                                    let (start_line, start_col) =
                                        renderer.pixel_to_cursor(start_x, start_y, &self.model);
                                    self.model.editor_mut().primary_selection_mut().anchor =
                                        Position::new(start_line, start_col);
                                }
                            }
                        }

                        if self.drag_active {
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
                        let event = make_mouse_event(
                            x,
                            y,
                            MouseButton::Left,
                            ElementState::Pressed,
                            1, // Click count computed inside handler
                            self.modifiers,
                        );
                        let result = handle_mouse_press(
                            &mut self.model,
                            renderer,
                            event,
                            &mut self.click_tracker,
                        );

                        // Update drag tracking state
                        if result.start_drag_tracking {
                            self.left_mouse_down = true;
                            self.drag_start_position = Some((x, y));
                            self.drag_active = false;
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
                self.left_mouse_down = false;
                self.last_auto_scroll = None;
                self.drag_start_position = None;
                self.drag_active = false;

                // End splitter drag if active
                if self.model.ui.splitter_drag.is_some() {
                    return update(&mut self.model, Msg::Layout(LayoutMsg::EndSplitterDrag));
                }

                // End sidebar resize drag if active
                if self.model.ui.sidebar_resize.is_some() {
                    return update(
                        &mut self.model,
                        Msg::Workspace(WorkspaceMsg::EndSidebarResize),
                    );
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
                        let event = make_mouse_event(
                            x,
                            y,
                            MouseButton::Middle,
                            ElementState::Pressed,
                            1,
                            self.modifiers,
                        );
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
                use token::model::HoverRegion;
                use winit::event::MouseScrollDelta;

                let (h_delta, v_delta) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => ((x * 3.0) as i32, (-y * 3.0) as i32),
                    MouseScrollDelta::PixelDelta(pos) => {
                        let line_height = self.model.line_height as f64;
                        let char_width = self.model.char_width as f64;
                        ((pos.x / char_width) as i32, (-pos.y / line_height) as i32)
                    }
                };

                // Route scroll based on hover region
                match self.model.ui.hover {
                    // Sidebar: scroll the file tree
                    HoverRegion::Sidebar => {
                        if v_delta != 0 {
                            update(
                                &mut self.model,
                                Msg::Workspace(WorkspaceMsg::Scroll { lines: v_delta }),
                            )
                        } else {
                            None
                        }
                    }

                    // Dock panels: consume scroll events but don't route to editor
                    // Future: could route to panel-specific scroll handlers
                    HoverRegion::Dock(_position) => {
                        // Consume the scroll event to prevent bleeding into editor
                        // TODO: Implement per-panel scroll handling when panels have scrollable content
                        None
                    }

                    // Preview panes: consume scroll but don't route to editor
                    // Webview handles its own scrolling
                    HoverRegion::Preview => None,

                    // Modal/StatusBar/Splitter/TabBar/DockResize: ignore scroll
                    HoverRegion::Modal
                    | HoverRegion::StatusBar
                    | HoverRegion::Splitter
                    | HoverRegion::EditorTabBar
                    | HoverRegion::SidebarResize
                    | HoverRegion::DockResize(_)
                    | HoverRegion::None => None,

                    // Editor text area: scroll the editor (or CSV if in CSV mode)
                    HoverRegion::EditorText => {
                        // Check if focused editor is in CSV mode
                        let in_csv_mode = self
                            .model
                            .editor_area
                            .focused_editor()
                            .map(|e| e.view_mode.is_csv())
                            .unwrap_or(false);

                        if in_csv_mode {
                            let v_cmd = if v_delta != 0 {
                                update(&mut self.model, Msg::Csv(CsvMsg::ScrollVertical(v_delta)))
                            } else {
                                None
                            };

                            let h_cmd = if h_delta != 0 {
                                update(&mut self.model, Msg::Csv(CsvMsg::ScrollHorizontal(h_delta)))
                            } else {
                                None
                            };

                            return v_cmd.or(h_cmd);
                        }

                        let v_cmd = if v_delta != 0 {
                            update(&mut self.model, Msg::Editor(EditorMsg::Scroll(v_delta)))
                        } else {
                            None
                        };

                        let h_cmd = if h_delta != 0 {
                            update(
                                &mut self.model,
                                Msg::Editor(EditorMsg::ScrollHorizontal(h_delta)),
                            )
                        } else {
                            None
                        };

                        v_cmd.or(h_cmd)
                    }
                }
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

        // Sync webviews with preview panes
        self.sync_webviews();

        // Hide webviews when modals are active (so they don't render on top)
        let show_webviews = self.model.ui.active_modal.is_none();
        self.webview_manager.set_all_visible(show_webviews);

        self.perf.record_frame_time();
        self.perf.record_render_history();
        Ok(())
    }

    /// Synchronize webview instances with preview panes in the model.
    /// Creates, updates, or destroys webviews as needed.
    fn sync_webviews(&mut self) {
        use token::markdown::{markdown_to_html, PreviewTheme};
        use token::model::editor_area::PreviewId;

        let Some(window) = &self.window else {
            return;
        };

        // Get current preview IDs from the model
        let model_preview_ids: std::collections::HashSet<_> =
            self.model.editor_area.previews.keys().copied().collect();

        // Debug: log preview count
        if !model_preview_ids.is_empty() {
            tracing::debug!("sync_webviews: {} preview(s) in model", model_preview_ids.len());
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
            html: String,
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
                let markdown_content = document.buffer.to_string();
                let html = markdown_to_html(&markdown_content, &theme);

                let needs_create = !self.webview_manager.has_webview(preview_id);
                let needs_content_update = preview.needs_refresh(document.revision);

                // Compute content-area rect for the webview (below the "Preview" header)
                // preview.rect is the outer pane rect; we need to offset by header height
                let header_height = metrics.tab_bar_height as f32;
                let webview_rect = token::model::editor_area::Rect::new(
                    preview.rect.x,
                    preview.rect.y + header_height,
                    preview.rect.width,
                    (preview.rect.height - header_height).max(0.0),
                );

                Some(PreviewUpdate {
                    preview_id,
                    rect: webview_rect,
                    html,
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
                if let Err(e) = self.webview_manager.create_webview(
                    update.preview_id,
                    window,
                    update.rect,
                    &update.html,
                ) {
                    tracing::error!("Failed to create webview for preview: {}", e);
                    continue;
                }
                // Update last_revision after successful creation
                if let Some(preview) = self.model.editor_area.preview_mut(update.preview_id) {
                    preview.last_revision = update.doc_revision;
                }
            } else {
                // Update existing webview bounds
                let window_height = window.inner_size().height;
                self.webview_manager
                    .update_bounds(update.preview_id, update.rect, scale_factor, window_height);

                // Update content if revision changed
                if update.needs_content_update {
                    self.webview_manager
                        .update_content(update.preview_id, &update.html);
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
                    let _ = tx.send(Msg::App(AppMsg::SaveCompleted(result)));
                });
            }
            Cmd::LoadFile { path } => {
                let tx = self.msg_tx.clone();
                std::thread::spawn(move || {
                    let result = std::fs::read_to_string(&path).map_err(|e| e.to_string());
                    let _ = tx.send(Msg::App(AppMsg::FileLoaded { path, result }));
                });
            }
            Cmd::OpenInExplorer { path } => {
                #[cfg(target_os = "macos")]
                {
                    let _ = std::process::Command::new("open").arg(&path).spawn();
                }
                #[cfg(target_os = "windows")]
                {
                    let _ = std::process::Command::new("explorer").arg(&path).spawn();
                }
                #[cfg(target_os = "linux")]
                {
                    let _ = std::process::Command::new("xdg-open").arg(&path).spawn();
                }
            }
            Cmd::OpenFileInEditor { path } => {
                let tx = self.msg_tx.clone();
                std::thread::spawn(move || {
                    let result = std::fs::read_to_string(&path).map_err(|e| e.to_string());
                    let _ = tx.send(Msg::App(AppMsg::FileLoaded { path, result }));
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

                    let _ = tx.send(Msg::App(AppMsg::OpenFileDialogResult { paths }));
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
                    let _ = tx.send(Msg::App(AppMsg::SaveFileAsDialogResult { path }));
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
                    let _ = tx.send(Msg::App(AppMsg::OpenFolderDialogResult { folder }));
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
                tracing::debug!(
                    "DebouncedSyntaxParse: doc={} rev={} delay={}ms",
                    document_id.0,
                    revision,
                    delay_ms
                );
                let tx = self.msg_tx.clone();
                std::thread::spawn(move || {
                    if delay_ms > 0 {
                        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                    }
                    tracing::debug!("Sending ParseReady: doc={} rev={}", document_id.0, revision);
                    let _ = tx.send(Msg::Syntax(SyntaxMsg::ParseReady {
                        document_id,
                        revision,
                    }));
                });
            }

            Cmd::RunSyntaxParse {
                document_id,
                revision,
                source,
                language,
            } => {
                tracing::debug!(
                    "RunSyntaxParse: doc={} rev={} lang={:?} len={}",
                    document_id.0,
                    revision,
                    language,
                    source.len()
                );
                let syntax_tx = self.syntax_tx.clone();
                let _ = syntax_tx.send(SyntaxParseRequest {
                    document_id,
                    revision,
                    source,
                    language,
                });
            }

            // =====================================================================
            // Application Commands
            // =====================================================================
            Cmd::Quit => {
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
        let mut needs_redraw = false;
        while let Ok(msg) = self.msg_rx.try_recv() {
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
        }
        needs_redraw
    }
}

/// Create window icon from embedded PNG
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
                .with_title("Token")
                .with_window_icon(create_window_icon())
                .with_inner_size(LogicalSize::new(800, 600)); // TODO: Persist window size/position/monitor on exit/boot

            let window = Rc::new(event_loop.create_window(window_attributes).unwrap());
            let context = Context::new(Rc::clone(&window)).unwrap();

            self.init_renderer(Rc::clone(&window), &context).unwrap();
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

        if self.process_async_messages() {
            needs_redraw = true;
        }

        // Poll file system watcher for changes
        if self.poll_fs_watcher() {
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
        let next_blink = self.last_tick + blink_interval;
        event_loop.set_control_flow(ControlFlow::WaitUntil(next_blink));
    }
}

impl App {
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
fn syntax_worker_loop(rx: Receiver<SyntaxParseRequest>, msg_tx: Sender<Msg>) {
    use std::collections::HashMap;

    tracing::info!("Syntax worker thread started");

    let mut parser_state = ParserState::new();
    let mut pending: HashMap<token::model::editor_area::DocumentId, SyntaxParseRequest> =
        HashMap::new();

    loop {
        // Wait for first request (blocking)
        let request = match rx.recv() {
            Ok(req) => {
                tracing::debug!(
                    "Worker received request: doc={} rev={} lang={:?}",
                    req.document_id.0,
                    req.revision,
                    req.language
                );
                req
            }
            Err(_) => {
                tracing::info!("Syntax worker channel closed, exiting");
                return;
            }
        };
        pending.insert(request.document_id, request);

        // Drain any additional pending requests (non-blocking)
        // Keep only the latest request per document
        while let Ok(req) = rx.try_recv() {
            tracing::debug!(
                "Worker draining request: doc={} rev={}",
                req.document_id.0,
                req.revision
            );
            pending.insert(req.document_id, req);
        }

        // Process all pending requests
        for (_doc_id, req) in pending.drain() {
            tracing::debug!(
                "Worker parsing: doc={} rev={} lang={:?}",
                req.document_id.0,
                req.revision,
                req.language
            );

            let highlights = parser_state.parse_and_highlight(
                &req.source,
                req.language,
                req.document_id,
                req.revision,
            );

            let line_count = highlights.lines.len();
            let token_count: usize = highlights.lines.values().map(|lh| lh.tokens.len()).sum();

            tracing::debug!(
                "Worker sending ParseCompleted: doc={} rev={} lines={} tokens={}",
                req.document_id.0,
                req.revision,
                line_count,
                token_count
            );

            let _ = msg_tx.send(Msg::Syntax(SyntaxMsg::ParseCompleted {
                document_id: req.document_id,
                revision: req.revision,
                highlights,
            }));
        }
    }
}
