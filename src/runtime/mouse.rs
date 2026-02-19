//! Mouse event handling using the unified hit-test system
//!
//! This module provides centralized mouse event dispatch that:
//! - Uses `hit_test_ui()` to determine the target under the cursor
//! - Dispatches behavior based on (target, button, click_count)
//! - Handles focus changes consistently
//! - Shares hit-testing logic across left/middle/right clicks

use std::time::{Duration, Instant};

use winit::event::{ElementState, MouseButton};
use winit::keyboard::ModifiersState;

use token::commands::Cmd;
use token::messages::{LayoutMsg, ModalMsg, Msg, PreviewMsg, UiMsg, WorkspaceMsg};
use token::model::AppModel;
use token::update::update;

use token::view::hit_test::{hit_test_ui, EventResult, HitTarget, MouseEvent};
use token::view::Renderer;

/// Click tracking state for double/triple click detection
pub struct ClickTracker {
    pub last_click_time: Instant,
    pub last_click_position: Option<(usize, usize)>,
    pub click_count: u32,
}

impl Default for ClickTracker {
    fn default() -> Self {
        Self {
            last_click_time: Instant::now() - Duration::from_secs(10),
            last_click_position: None,
            click_count: 0,
        }
    }
}

impl ClickTracker {
    /// Update click count based on timing and position
    ///
    /// Returns the new click count (1, 2, or 3)
    pub fn track_click(&mut self, line: usize, column: usize) -> u8 {
        let now = Instant::now();
        let double_click_time = Duration::from_millis(300);

        let is_rapid_click = now.duration_since(self.last_click_time) < double_click_time;
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

        self.click_count as u8
    }

    /// Track a click at a generic row (for sidebar)
    pub fn track_row_click(&mut self, row: usize) -> u8 {
        self.track_click(row, 0)
    }

    /// Reset click tracking (e.g., when target changes)
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.click_count = 0;
        self.last_click_position = None;
    }
}

/// Construct a MouseEvent from raw input data
pub fn make_mouse_event(
    x: f64,
    y: f64,
    button: MouseButton,
    state: ElementState,
    click_count: u8,
    modifiers: ModifiersState,
) -> MouseEvent {
    MouseEvent::new(x, y, button, state, click_count, modifiers)
}

/// Result of mouse press handling, including state changes for the App
#[derive(Debug, Clone)]
pub struct MousePressResult {
    /// Command to execute (usually Redraw or None)
    pub cmd: Option<Cmd>,
    /// Whether to start tracking left mouse drag (for text selection)
    pub start_drag_tracking: bool,
}

/// Handle a mouse press event using the unified hit-test system
///
/// This is the main entry point for mouse click handling. It:
/// 1. Performs hit-testing to find the target
/// 2. Dispatches to the appropriate handler based on (target, button)
/// 3. Applies focus changes from EventResult
/// 4. Returns MousePressResult with command and state changes
pub fn handle_mouse_press(
    model: &mut AppModel,
    renderer: &mut Renderer,
    event: MouseEvent,
    click_tracker: &mut ClickTracker,
) -> MousePressResult {
    let char_width = renderer.char_width();
    let pt = event.pos;

    // Perform hit-testing
    let Some(target) = hit_test_ui(model, pt, char_width) else {
        return MousePressResult {
            cmd: None,
            start_drag_tracking: false,
        };
    };

    // Track if we're clicking on editor content (for drag tracking)
    let is_editor_content = matches!(
        target,
        HitTarget::EditorContent { .. } | HitTarget::EditorGutter { .. }
    );
    let is_left_click = matches!(event.button, MouseButton::Left);

    // Dispatch based on target and button
    let result = dispatch_mouse_press(model, renderer, &target, &event, click_tracker);

    // Apply focus changes
    if let EventResult::Consumed {
        focus: Some(focus_target),
        ..
    } = &result
    {
        match focus_target {
            token::model::FocusTarget::Editor => model.ui.focus_editor(),
            token::model::FocusTarget::Sidebar => model.ui.focus_sidebar(),
            token::model::FocusTarget::Dock(pos) => model.ui.focus_dock(*pos),
            token::model::FocusTarget::Modal => {}
        }
    }

    // Determine command - use explicit cmd if present, otherwise fallback to redraw
    let cmd = match &result {
        EventResult::Consumed { cmd: Some(c), .. } => Some(c.clone()),
        EventResult::Consumed { redraw: true, .. } => Some(Cmd::Redraw),
        EventResult::Consumed { redraw: false, .. } => None,
        EventResult::Bubble => None,
    };

    MousePressResult {
        cmd,
        start_drag_tracking: is_editor_content && is_left_click,
    }
}

/// Dispatch a mouse press to the appropriate handler based on target and button
fn dispatch_mouse_press(
    model: &mut AppModel,
    renderer: &mut Renderer,
    target: &HitTarget,
    event: &MouseEvent,
    click_tracker: &mut ClickTracker,
) -> EventResult {
    match event.button {
        MouseButton::Left => handle_left_click(model, renderer, target, event, click_tracker),
        MouseButton::Middle => handle_middle_click(model, renderer, target, event),
        MouseButton::Right => handle_right_click(model, target, event),
        _ => EventResult::Bubble,
    }
}

/// Handle left mouse button clicks
fn handle_left_click(
    model: &mut AppModel,
    renderer: &mut Renderer,
    target: &HitTarget,
    event: &MouseEvent,
    click_tracker: &mut ClickTracker,
) -> EventResult {
    use token::model::FocusTarget;

    match target {
        // Modal handling
        HitTarget::Modal { inside } => {
            if *inside {
                // Click inside modal - consume but don't close
                // Future: could handle clicking on list items
                EventResult::consumed_redraw()
            } else {
                // Click outside modal - close it
                update(model, Msg::Ui(UiMsg::Modal(ModalMsg::Close)));
                EventResult::consumed_redraw()
            }
        }

        // Status bar - consume but do nothing
        HitTarget::StatusBar => EventResult::consumed_no_redraw(),

        // Sidebar resize handle
        HitTarget::SidebarResize => {
            update(
                model,
                Msg::Workspace(WorkspaceMsg::StartSidebarResize {
                    initial_x: event.pos.x,
                }),
            );
            EventResult::consumed_with_focus(FocusTarget::Editor)
        }

        // Sidebar empty area
        HitTarget::SidebarEmpty => EventResult::consumed_with_focus(FocusTarget::Sidebar),

        // Sidebar item
        HitTarget::SidebarItem {
            path,
            row,
            is_dir,
            clicked_on_chevron,
        } => {
            // Track clicks for double-click detection
            let click_count = click_tracker.track_row_click(*row);

            // Always select the item
            update(
                model,
                Msg::Workspace(WorkspaceMsg::SelectItem(path.clone())),
            );

            // Chevron click immediately toggles folder
            if *clicked_on_chevron {
                update(
                    model,
                    Msg::Workspace(WorkspaceMsg::ToggleFolder(path.clone())),
                );
                return EventResult::consumed_with_focus(FocusTarget::Sidebar);
            }

            // Double-click opens file or toggles folder
            if click_count >= 2 {
                let cmd = if *is_dir {
                    update(
                        model,
                        Msg::Workspace(WorkspaceMsg::ToggleFolder(path.clone())),
                    )
                } else {
                    update(
                        model,
                        Msg::Workspace(WorkspaceMsg::OpenFile {
                            path: path.clone(),
                            preview: false,
                        }),
                    )
                };
                // Return the command from opening the file (includes syntax parse)
                return EventResult::consumed_with_cmd(cmd, FocusTarget::Sidebar);
            }

            EventResult::consumed_with_focus(FocusTarget::Sidebar)
        }

        // Splitter drag
        HitTarget::Splitter { index } => {
            update(
                model,
                Msg::Layout(LayoutMsg::BeginSplitterDrag {
                    splitter_index: *index,
                    position: (event.pos.x as f32, event.pos.y as f32),
                }),
            );
            EventResult::consumed_redraw()
        }

        // Preview pane header - consume, keep editor focus
        HitTarget::PreviewHeader { .. } => {
            // Just consume - middle-click closes
            EventResult::consumed_with_focus(FocusTarget::Editor)
        }

        // Preview pane content - consume, keep editor focus for keyboard
        HitTarget::PreviewContent { .. } => {
            // Webview handles its own clicks; just keep editor focus
            EventResult::consumed_with_focus(FocusTarget::Editor)
        }

        // Tab click
        HitTarget::GroupTab {
            group_id,
            tab_index,
            ..
        } => {
            // Focus group if not already focused
            if *group_id != model.editor_area.focused_group_id {
                update(model, Msg::Layout(LayoutMsg::FocusGroup(*group_id)));
            }
            update(model, Msg::Layout(LayoutMsg::SwitchToTab(*tab_index)));
            EventResult::consumed_with_focus(FocusTarget::Editor)
        }

        // Empty tab bar area
        HitTarget::GroupTabBarEmpty { group_id } => {
            if *group_id != model.editor_area.focused_group_id {
                update(model, Msg::Layout(LayoutMsg::FocusGroup(*group_id)));
            }
            EventResult::consumed_with_focus(FocusTarget::Editor)
        }

        // Editor gutter (line numbers) - could be used for line selection
        HitTarget::EditorGutter { group_id, .. } => {
            if *group_id != model.editor_area.focused_group_id {
                update(model, Msg::Layout(LayoutMsg::FocusGroup(*group_id)));
            }
            // For now, treat like editor content click
            // Future: could select entire line
            EventResult::consumed_with_focus(FocusTarget::Editor)
        }

        // Editor content - handled specially due to complex selection logic
        HitTarget::EditorContent { group_id, .. } => {
            handle_editor_content_click(model, renderer, *group_id, event, click_tracker)
        }

        // CSV cell click - use renderer to find actual cell
        HitTarget::CsvCell { group_id, .. } => {
            use token::messages::CsvMsg;

            if *group_id != model.editor_area.focused_group_id {
                update(model, Msg::Layout(LayoutMsg::FocusGroup(*group_id)));
            }

            // Use renderer to find the actual cell at this position
            if let Some(cell) = renderer.pixel_to_csv_cell(event.pos.x, event.pos.y, model) {
                update(
                    model,
                    Msg::Csv(CsvMsg::SelectCell {
                        row: cell.row,
                        col: cell.col,
                    }),
                );
            }
            EventResult::consumed_with_focus(FocusTarget::Editor)
        }

        // Dock resize handle
        HitTarget::DockResize { position } => {
            update(
                model,
                Msg::Dock(token::messages::DockMsg::StartResize {
                    position: *position,
                    initial_coord: event.pos.x,
                }),
            );
            EventResult::consumed_with_focus(FocusTarget::Editor)
        }

        // Dock tab click - focus/toggle panel
        HitTarget::DockTab { panel_id, .. } => {
            update(
                model,
                Msg::Dock(token::messages::DockMsg::FocusOrTogglePanel(*panel_id)),
            );
            EventResult::consumed_redraw()
        }

        // Dock tab bar empty area
        HitTarget::DockTabBarEmpty { position } => {
            // Focus the dock
            update(
                model,
                Msg::Dock(token::messages::DockMsg::FocusDock(*position)),
            );
            EventResult::consumed_redraw()
        }

        // Dock content area - handle panel-specific interactions
        HitTarget::DockContent {
            position,
            active_panel_id,
        } => {
            // Focus the dock first
            update(
                model,
                Msg::Dock(token::messages::DockMsg::FocusDock(*position)),
            );

            // Handle outline panel clicks
            if *active_panel_id == token::panel::PanelId::Outline {
                use token::messages::OutlineMsg;

                let scale_factor = model.metrics.scale_factor;
                let dock_width = model.dock_layout.right.size(scale_factor);
                let bottom_height = model.dock_layout.bottom.size(scale_factor);
                let window_height = model.window_size.1 as f32;
                let status_bar_height = model.line_height as f32;
                let dock_height = window_height - status_bar_height - bottom_height;
                let dock_x = model.window_size.0 as f32 - dock_width;

                let row_height = model.metrics.file_tree_row_height;
                let content_y = 0.0 + row_height as f32 + 4.0; // title bar offset

                let local_y = event.pos.y as f32 - content_y;
                if local_y >= 0.0 && (event.pos.y as f32) < dock_height {
                    let clicked_visual_row = (local_y / row_height as f32) as usize;
                    let clicked_index = model.outline_panel.scroll_offset + clicked_visual_row;

                    // Determine if click was on the chevron/indicator area
                    let local_x = event.pos.x as f32 - dock_x - 8.0;
                    let indent = model.metrics.file_tree_indent;
                    // We approximate: consider clicks in the first 14px of the row as chevron clicks
                    // A more precise check would need the node's depth, but this is sufficient
                    let on_chevron = local_x >= 0.0 && local_x < 14.0 + indent;

                    let click_count = click_tracker.track_row_click(clicked_index);

                    update(
                        model,
                        Msg::Outline(OutlineMsg::ClickRow {
                            index: clicked_index,
                            click_count,
                            on_chevron,
                        }),
                    );
                }

                return EventResult::consumed_with_focus(FocusTarget::Dock(*position));
            }

            // For left dock (file explorer), return sidebar focus
            match position {
                token::panel::DockPosition::Left => {
                    EventResult::consumed_with_focus(FocusTarget::Sidebar)
                }
                _ => EventResult::consumed_redraw(),
            }
        }
    }
}

/// Handle editor content click with full selection logic
fn handle_editor_content_click(
    model: &mut AppModel,
    renderer: &mut Renderer,
    group_id: token::model::editor_area::GroupId,
    event: &MouseEvent,
    click_tracker: &mut ClickTracker,
) -> EventResult {
    use token::messages::EditorMsg;
    use token::model::FocusTarget;

    // Focus group if needed
    if group_id != model.editor_area.focused_group_id {
        update(model, Msg::Layout(LayoutMsg::FocusGroup(group_id)));
    }

    // Convert pixel to cursor position
    let (line, column) = renderer.pixel_to_cursor(event.pos.x, event.pos.y, model);

    // Track clicks for double/triple detection
    let click_count = click_tracker.track_click(line, column);

    // Handle modifiers
    if event.shift() {
        update(
            model,
            Msg::Editor(EditorMsg::ExtendSelectionToPosition { line, column }),
        );
        return EventResult::consumed_with_focus(FocusTarget::Editor);
    }

    if event.alt() {
        update(
            model,
            Msg::Editor(EditorMsg::ToggleCursorAtPosition { line, column }),
        );
        return EventResult::consumed_with_focus(FocusTarget::Editor);
    }

    // Handle click count
    match click_count {
        2 => {
            update(
                model,
                Msg::Editor(EditorMsg::SetCursorPosition { line, column }),
            );
            update(model, Msg::Editor(EditorMsg::SelectWord));
        }
        3 => {
            update(
                model,
                Msg::Editor(EditorMsg::SetCursorPosition { line, column }),
            );
            update(model, Msg::Editor(EditorMsg::SelectLine));
        }
        _ => {
            model.editor_mut().clear_selection();
            update(
                model,
                Msg::Editor(EditorMsg::SetCursorPosition { line, column }),
            );
        }
    }

    EventResult::consumed_with_focus(FocusTarget::Editor)
}

/// Handle middle mouse button clicks
fn handle_middle_click(
    model: &mut AppModel,
    renderer: &mut Renderer,
    target: &HitTarget,
    event: &MouseEvent,
) -> EventResult {
    match target {
        // Status bar - ignore
        HitTarget::StatusBar => EventResult::consumed_no_redraw(),

        // Preview header - middle click closes preview
        HitTarget::PreviewHeader { .. } => {
            update(model, Msg::Preview(PreviewMsg::Close));
            EventResult::consumed_redraw()
        }

        // Preview content - consume but no action (webview handles its own)
        HitTarget::PreviewContent { .. } => EventResult::consumed_no_redraw(),

        // Tab - middle click closes tab
        HitTarget::GroupTab {
            group_id, tab_id, ..
        } => {
            if *group_id != model.editor_area.focused_group_id {
                update(model, Msg::Layout(LayoutMsg::FocusGroup(*group_id)));
            }
            update(model, Msg::Layout(LayoutMsg::CloseTab(*tab_id)));
            EventResult::consumed_redraw()
        }

        // Empty tab bar area - consume but no action
        HitTarget::GroupTabBarEmpty { .. } => EventResult::consumed_no_redraw(),

        // Editor gutter - treat like editor content for rectangle selection
        HitTarget::EditorGutter { group_id, .. } => {
            use token::messages::EditorMsg;

            if *group_id != model.editor_area.focused_group_id {
                update(model, Msg::Layout(LayoutMsg::FocusGroup(*group_id)));
            }

            let (line, visual_col) =
                renderer.pixel_to_line_and_visual_column(event.pos.x, event.pos.y, model);
            update(
                model,
                Msg::Editor(EditorMsg::StartRectangleSelection { line, visual_col }),
            );
            EventResult::consumed_redraw()
        }

        // Editor content - start rectangle selection
        HitTarget::EditorContent { group_id, .. } => {
            use token::messages::EditorMsg;

            if *group_id != model.editor_area.focused_group_id {
                update(model, Msg::Layout(LayoutMsg::FocusGroup(*group_id)));
            }

            let (line, visual_col) =
                renderer.pixel_to_line_and_visual_column(event.pos.x, event.pos.y, model);
            update(
                model,
                Msg::Editor(EditorMsg::StartRectangleSelection { line, visual_col }),
            );
            EventResult::consumed_redraw()
        }

        // CSV cell - no middle-click behavior
        HitTarget::CsvCell { .. } => EventResult::consumed_no_redraw(),

        // Modal - consume, no action
        HitTarget::Modal { .. } => EventResult::consumed_no_redraw(),

        // Sidebar targets - consume, no action for middle-click
        HitTarget::SidebarEmpty | HitTarget::SidebarItem { .. } => {
            EventResult::consumed_no_redraw()
        }

        // Sidebar resize and splitters - consume, no action
        HitTarget::SidebarResize | HitTarget::Splitter { .. } => EventResult::consumed_no_redraw(),

        // Dock targets - consume, no special middle-click action
        HitTarget::DockResize { .. }
        | HitTarget::DockTab { .. }
        | HitTarget::DockTabBarEmpty { .. }
        | HitTarget::DockContent { .. } => EventResult::consumed_no_redraw(),
    }
}

/// Handle right mouse button clicks (context menus - future)
fn handle_right_click(
    _model: &mut AppModel,
    _target: &HitTarget,
    _event: &MouseEvent,
) -> EventResult {
    // Future: show context menus based on target
    EventResult::Bubble
}
