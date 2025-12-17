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
use winit::window::{CursorIcon, Window};

use token::cli::StartupConfig;
use token::commands::{filter_commands, Cmd};
use token::keymap::{
    keystroke_from_winit, load_default_keymap, Command, KeyAction, KeyContext, Keymap,
};
use token::messages::{
    AppMsg, CsvMsg, EditorMsg, LayoutMsg, ModalMsg, Msg, SyntaxMsg, UiMsg, WorkspaceMsg,
};
use token::model::editor::Position;
use token::model::editor_area::{Rect, SplitDirection};
use token::model::{AppModel, ModalState};
use token::syntax::{LanguageId, ParserState};
use token::update::update;

use crate::view::geometry::{is_in_group_tab_bar, is_in_modal};

use super::input::handle_key;
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
    last_click_time: Instant,
    last_click_position: Option<(usize, usize)>,
    click_count: u32,
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

        // Open workspace if specified
        if let Some(root) = workspace_root {
            model.open_workspace(root);
        }

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
            last_click_time: Instant::now(),
            last_click_position: None,
            click_count: 0,
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
        KeyContext {
            has_selection: !self.model.editor().active_selection().is_empty(),
            has_multiple_cursors: self.model.editor().has_multiple_cursors(),
            modal_active: self.model.ui.has_modal(),
            editor_focused: !self.model.ui.has_modal(),
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

    fn update_cursor_icon(&mut self, x: f64, y: f64) {
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

        // Modal overlay → Default pointer
        if self.model.ui.has_modal() {
            window.set_cursor(CursorIcon::Default);
            return;
        }

        // Sidebar resize border (right edge, ~4px hit zone) → ColResize
        const SIDEBAR_RESIZE_HIT_ZONE: f64 = 4.0;
        if sidebar_width > 0.0 {
            let resize_zone_start = sidebar_width as f64 - SIDEBAR_RESIZE_HIT_ZONE;
            let resize_zone_end = sidebar_width as f64 + SIDEBAR_RESIZE_HIT_ZONE;
            if x >= resize_zone_start && x <= resize_zone_end {
                window.set_cursor(CursorIcon::ColResize);
                return;
            }

            // Sidebar file tree area → Default pointer
            if x < sidebar_width as f64 {
                window.set_cursor(CursorIcon::Default);
                return;
            }
        }

        // Status bar → Default pointer
        if renderer.is_in_status_bar(y) {
            window.set_cursor(CursorIcon::Default);
            return;
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

        // Splitter bars → ColResize/RowResize
        if let Some(idx) = self
            .model
            .editor_area
            .splitter_at_point(&splitters, x as f32, y as f32)
        {
            let icon = match splitters[idx].direction {
                SplitDirection::Horizontal => CursorIcon::ColResize,
                SplitDirection::Vertical => CursorIcon::RowResize,
            };
            window.set_cursor(icon);
            return;
        }

        // Tab bars → Default pointer
        let tab_bar_height = self.model.metrics.tab_bar_height;
        for group in self.model.editor_area.groups.values() {
            if is_in_group_tab_bar(y, &group.rect, tab_bar_height)
                && x >= group.rect.x as f64
                && x < (group.rect.x + group.rect.width) as f64
            {
                window.set_cursor(CursorIcon::Default);
                return;
            }
        }

        // Gutter area (line numbers) → Default pointer
        let gutter_width = token::model::gutter_border_x_scaled(
            renderer.char_width(),
            &self.model.metrics,
        ) as f64;
        if let Some(group) = self.model.editor_area.focused_group() {
            let gutter_x_end = group.rect.x as f64 + gutter_width;
            let content_y_start = group.rect.y as f64 + tab_bar_height as f64;
            if x >= group.rect.x as f64
                && x < gutter_x_end
                && y >= content_y_start
            {
                window.set_cursor(CursorIcon::Default);
                return;
            }
        }

        // Editor text area → I-beam cursor
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

                    // Try keymap first for simple commands, but only when:
                    // - No modal is active (modals handled by handle_modal_key in input.rs)
                    // - Not in option double-tap mode with alt pressed (multi-cursor gesture)
                    let skip_keymap =
                        self.model.ui.has_modal() || (self.option_double_tapped && alt);

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
                    // Modal mouse blocking - click outside closes, click inside is consumed
                    if self.model.ui.has_modal() {
                        let (has_list, list_items) = match &self.model.ui.active_modal {
                            Some(ModalState::CommandPalette(state)) => {
                                (true, filter_commands(&state.input).len())
                            }
                            _ => (false, 0),
                        };
                        let in_modal = is_in_modal(
                            x,
                            y,
                            self.model.window_size.0 as usize,
                            self.model.window_size.1 as usize,
                            self.model.line_height,
                            has_list,
                            list_items,
                        );

                        if in_modal {
                            // Future: could handle clicking on list items here
                            return Some(Cmd::Redraw);
                        } else {
                            // Click outside modal closes it
                            return update(&mut self.model, Msg::Ui(UiMsg::Modal(ModalMsg::Close)));
                        }
                    }

                    // Check for sidebar click before other interactions
                    // Extract sidebar info without holding borrow across update() calls
                    let sidebar_click_info = if let Some(workspace) = &self.model.workspace {
                        if workspace.sidebar_visible {
                            // Mouse coordinates are already in physical pixels (winit provides physical coords)
                            // sidebar_width and file_tree_row_height are also in physical pixels (scaled)
                            let sidebar_width =
                                workspace.sidebar_width(self.model.metrics.scale_factor) as f64;

                            if x < sidebar_width {
                                let row_height = self.model.metrics.file_tree_row_height as f64;
                                let indent = self.model.metrics.file_tree_indent as f64;
                                let clicked_row = (y / row_height) as usize;

                                // Find the item at this row and extract info including depth
                                if let Some((node, depth)) = workspace
                                    .file_tree
                                    .get_visible_item_with_depth(
                                        clicked_row,
                                        &workspace.expanded_folders,
                                    )
                                {
                                    // Calculate chevron area: starts at (depth * indent + 8), width ~16px
                                    let chevron_start = (depth as f64 * indent) + 8.0;
                                    let chevron_end = chevron_start + 16.0;
                                    let clicked_on_chevron =
                                        node.is_dir && x >= chevron_start && x < chevron_end;
                                    Some((
                                        node.path.clone(),
                                        node.is_dir,
                                        clicked_row,
                                        clicked_on_chevron,
                                    ))
                                } else {
                                    Some((std::path::PathBuf::new(), false, clicked_row, false)) // Empty click in sidebar
                                }
                            } else {
                                None // Not in sidebar
                            }
                        } else {
                            None // Sidebar not visible
                        }
                    } else {
                        None // No workspace
                    };

                    if let Some((path, is_dir, clicked_row, clicked_on_chevron)) = sidebar_click_info
                    {
                        if path.as_os_str().is_empty() {
                            // Click in sidebar but not on an item
                            return Some(Cmd::Redraw);
                        }

                        // Single-click on chevron immediately toggles folder
                        if clicked_on_chevron {
                            update(
                                &mut self.model,
                                Msg::Workspace(WorkspaceMsg::SelectItem(path.clone())),
                            );
                            return update(
                                &mut self.model,
                                Msg::Workspace(WorkspaceMsg::ToggleFolder(path)),
                            );
                        }

                        let now = Instant::now();
                        let double_click_time = Duration::from_millis(300);
                        let is_double_click = now.duration_since(self.last_click_time)
                            < double_click_time
                            && self.last_click_position == Some((clicked_row, 0));

                        // Update click tracking for sidebar
                        self.last_click_time = now;
                        self.last_click_position = Some((clicked_row, 0));

                        // Always select the item
                        update(
                            &mut self.model,
                            Msg::Workspace(WorkspaceMsg::SelectItem(path.clone())),
                        );

                        // Only toggle folder or open file on double-click
                        if is_double_click {
                            if is_dir {
                                return update(
                                    &mut self.model,
                                    Msg::Workspace(WorkspaceMsg::ToggleFolder(path)),
                                );
                            } else {
                                return update(
                                    &mut self.model,
                                    Msg::Workspace(WorkspaceMsg::OpenFile {
                                        path,
                                        preview: false,
                                    }),
                                );
                            }
                        }
                        return Some(Cmd::Redraw);
                    }

                    // Check for splitter hit before other interactions
                    {
                        // Calculate sidebar offset for fallback rect
                        let sidebar_width = self
                            .model
                            .workspace
                            .as_ref()
                            .filter(|ws| ws.sidebar_visible)
                            .map(|ws| ws.sidebar_width(self.model.metrics.scale_factor))
                            .unwrap_or(0.0);

                        let available =
                            self.model.editor_area.last_layout_rect.unwrap_or(Rect::new(
                                sidebar_width,
                                0.0,
                                self.model.window_size.0 as f32 - sidebar_width,
                                self.model.window_size.1 as f32,
                            ));
                        let splitters = self
                            .model
                            .editor_area
                            .compute_layout_scaled(available, self.model.metrics.splitter_width);
                        if let Some(idx) = self
                            .model
                            .editor_area
                            .splitter_at_point(&splitters, x as f32, y as f32)
                        {
                            return update(
                                &mut self.model,
                                Msg::Layout(LayoutMsg::BeginSplitterDrag {
                                    splitter_index: idx,
                                    position: (x as f32, y as f32),
                                }),
                            );
                        }
                    }

                    if let Some(renderer) = &mut self.renderer {
                        if renderer.is_in_status_bar(y) {
                            return None;
                        }

                        // Per-group tab bar hit testing (handles splits correctly)
                        // First, find the clicked group/tab without holding borrow
                        let tab_bar_h = self.model.metrics.tab_bar_height;
                        let tab_click_info: Option<(_, f64, Rect)> =
                            self.model.editor_area.groups.iter().find_map(|(&gid, g)| {
                                if is_in_group_tab_bar(y, &g.rect, tab_bar_h)
                                    && x >= g.rect.x as f64
                                    && x < (g.rect.x + g.rect.width) as f64
                                {
                                    Some((gid, x - g.rect.x as f64, g.rect))
                                } else {
                                    None
                                }
                            });

                        if let Some((group_id, local_x, _rect)) = tab_click_info {
                            // Focus the group if not already focused
                            if group_id != self.model.editor_area.focused_group_id {
                                update(
                                    &mut self.model,
                                    Msg::Layout(LayoutMsg::FocusGroup(group_id)),
                                );
                            }

                            // Find which tab was clicked
                            if let Some(group) = self.model.editor_area.groups.get(&group_id) {
                                if let Some(tab_index) =
                                    renderer.tab_at_position(local_x, &self.model, group)
                                {
                                    return update(
                                        &mut self.model,
                                        Msg::Layout(LayoutMsg::SwitchToTab(tab_index)),
                                    );
                                }
                            }

                            // Click in empty tab bar area - consume but don't click-through
                            return None;
                        }

                        self.left_mouse_down = true;
                        self.drag_start_position = Some((x, y));
                        self.drag_active = false;

                        if let Some(group_id) =
                            self.model.editor_area.group_at_point(x as f32, y as f32)
                        {
                            if group_id != self.model.editor_area.focused_group_id {
                                update(
                                    &mut self.model,
                                    Msg::Layout(LayoutMsg::FocusGroup(group_id)),
                                );
                            }
                        }

                        // Check if focused editor is in CSV mode - route click to CSV hit-testing
                        let in_csv_mode = self
                            .model
                            .editor_area
                            .focused_editor()
                            .map(|e| e.view_mode.is_csv())
                            .unwrap_or(false);

                        if in_csv_mode {
                            if let Some(cell) = renderer.pixel_to_csv_cell(x, y, &self.model) {
                                return update(
                                    &mut self.model,
                                    Msg::Csv(CsvMsg::SelectCell {
                                        row: cell.row,
                                        col: cell.col,
                                    }),
                                );
                            }
                            return None;
                        }

                        let (line, column) = renderer.pixel_to_cursor(x, y, &self.model);
                        let now = Instant::now();
                        let double_click_time = Duration::from_millis(300);

                        let is_rapid_click =
                            now.duration_since(self.last_click_time) < double_click_time;
                        let is_same_position = self.last_click_position == Some((line, column));

                        if is_rapid_click && is_same_position {
                            self.click_count += 1;
                            if self.click_count > 3 {
                                self.click_count = 1;
                            }
                        } else {
                            self.click_count = 1;
                        }

                        self.last_click_time = now;
                        self.last_click_position = Some((line, column));

                        if self.modifiers.shift_key() {
                            return update(
                                &mut self.model,
                                Msg::Editor(EditorMsg::ExtendSelectionToPosition { line, column }),
                            );
                        }

                        if self.modifiers.alt_key() {
                            return update(
                                &mut self.model,
                                Msg::Editor(EditorMsg::ToggleCursorAtPosition { line, column }),
                            );
                        }

                        match self.click_count {
                            2 => {
                                update(
                                    &mut self.model,
                                    Msg::Editor(EditorMsg::SetCursorPosition { line, column }),
                                );
                                return update(&mut self.model, Msg::Editor(EditorMsg::SelectWord));
                            }
                            3 => {
                                update(
                                    &mut self.model,
                                    Msg::Editor(EditorMsg::SetCursorPosition { line, column }),
                                );
                                return update(&mut self.model, Msg::Editor(EditorMsg::SelectLine));
                            }
                            _ => {
                                self.model.editor_mut().clear_selection();
                                return update(
                                    &mut self.model,
                                    Msg::Editor(EditorMsg::SetCursorPosition { line, column }),
                                );
                            }
                        }
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
                None
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Middle,
                ..
            } => {
                if let Some((x, y)) = self.mouse_position {
                    if let Some(renderer) = &mut self.renderer {
                        if renderer.is_in_status_bar(y) {
                            return None;
                        }

                        // Middle-click on tab bar closes the clicked tab
                        let tab_h = self.model.metrics.tab_bar_height;
                        let tab_click_info: Option<(_, f64)> =
                            self.model.editor_area.groups.iter().find_map(|(&gid, g)| {
                                if is_in_group_tab_bar(y, &g.rect, tab_h)
                                    && x >= g.rect.x as f64
                                    && x < (g.rect.x + g.rect.width) as f64
                                {
                                    Some((gid, x - g.rect.x as f64))
                                } else {
                                    None
                                }
                            });

                        if let Some((group_id, local_x)) = tab_click_info {
                            // Find which tab was clicked and close it
                            if let Some(group) = self.model.editor_area.groups.get(&group_id) {
                                if let Some(tab_index) =
                                    renderer.tab_at_position(local_x, &self.model, group)
                                {
                                    // Get the TabId from the group's tabs
                                    if let Some(tab) = group.tabs.get(tab_index) {
                                        let tab_id = tab.id;
                                        // Focus the group first if needed
                                        if group_id != self.model.editor_area.focused_group_id {
                                            update(
                                                &mut self.model,
                                                Msg::Layout(LayoutMsg::FocusGroup(group_id)),
                                            );
                                        }
                                        return update(
                                            &mut self.model,
                                            Msg::Layout(LayoutMsg::CloseTab(tab_id)),
                                        );
                                    }
                                }
                            }
                            return None;
                        }

                        let (line, visual_col) =
                            renderer.pixel_to_line_and_visual_column(x, y, &self.model);
                        return update(
                            &mut self.model,
                            Msg::Editor(EditorMsg::StartRectangleSelection { line, visual_col }),
                        );
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
                use winit::event::MouseScrollDelta;
                let (h_delta, v_delta) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => ((x * 3.0) as i32, (-y * 3.0) as i32),
                    MouseScrollDelta::PixelDelta(pos) => {
                        let line_height = self.model.line_height as f64;
                        let char_width = self.model.char_width as f64;
                        ((pos.x / char_width) as i32, (-pos.y / line_height) as i32)
                    }
                };

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
            renderer.render(&mut self.model, &mut self.perf)?;
        }

        self.perf.record_frame_time();
        self.perf.record_render_history();
        Ok(())
    }

    fn tick(&mut self) -> Option<Cmd> {
        update(&mut self.model, Msg::Ui(UiMsg::BlinkCursor))
    }

    fn process_cmd(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::None => {}
            Cmd::Redraw => {}
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
                self.process_cmd(cmd);
            }
        }
        needs_redraw
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window_attributes = Window::default_attributes()
                .with_title("Token")
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

        if should_exit {
            event_loop.exit();
        } else if should_redraw {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Poll);

        if self.process_async_messages() {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }

        let now = Instant::now();
        if now.duration_since(self.last_tick) > Duration::from_millis(500) {
            self.last_tick = now;
            if self.tick().is_some() {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
        }
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
