//! Hit-testing types and functions for unified mouse event handling
//!
//! This module provides a centralized system for determining which UI element
//! is under a given point, and for handling mouse events in a consistent way.
//!
//! The design follows a "hit-test → dispatch" pattern:
//! 1. `hit_test_ui()` determines the highest-priority `HitTarget` at a point
//! 2. Event handlers match on `(HitTarget, MouseButton, click_count)` to dispatch behavior
//! 3. Handlers return `EventResult` to indicate consumption, focus changes, and redraw needs
//!
//! This replaces ad-hoc if/else chains in app.rs with explicit priority ordering
//! and shared hit-testing logic across left/middle/right clicks.

use std::path::PathBuf;

use winit::event::{ElementState, MouseButton};
use winit::keyboard::ModifiersState;

use crate::commands::filter_commands;
use crate::model::editor_area::{DocumentId, EditorId, GroupId, PreviewId, Rect, TabId};
use crate::model::{AppModel, FocusTarget, ModalState};

use super::geometry::{is_in_group_tab_bar, is_in_status_bar, Pane};

// ============================================================================
// Core Types
// ============================================================================

/// A point in window coordinates (physical pixels)
#[derive(Clone, Copy, Debug, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// A mouse event with all relevant context for hit-testing and dispatch
#[derive(Clone, Debug)]
pub struct MouseEvent {
    /// Position in window coordinates
    pub pos: Point,
    /// Which mouse button
    pub button: MouseButton,
    /// Pressed or released (currently only press events are handled)
    #[allow(dead_code)]
    pub state: ElementState,
    /// Click count: 1=single, 2=double, 3=triple (computed by ClickTracker, not used here)
    #[allow(dead_code)]
    pub click_count: u8,
    /// Active keyboard modifiers
    pub modifiers: ModifiersState,
}

impl MouseEvent {
    pub fn new(
        x: f64,
        y: f64,
        button: MouseButton,
        state: ElementState,
        click_count: u8,
        modifiers: ModifiersState,
    ) -> Self {
        Self {
            pos: Point::new(x, y),
            button,
            state,
            click_count,
            modifiers,
        }
    }

    /// Check if this is a press event
    #[inline]
    #[allow(dead_code)]
    pub fn is_pressed(&self) -> bool {
        matches!(self.state, ElementState::Pressed)
    }

    /// Check if this is a release event
    #[inline]
    #[allow(dead_code)]
    pub fn is_released(&self) -> bool {
        matches!(self.state, ElementState::Released)
    }

    /// Check if shift modifier is active
    #[inline]
    pub fn shift(&self) -> bool {
        self.modifiers.shift_key()
    }

    /// Check if ctrl/cmd modifier is active (for future context menus)
    #[inline]
    #[allow(dead_code)]
    pub fn ctrl(&self) -> bool {
        self.modifiers.control_key()
    }

    /// Check if alt/option modifier is active
    #[inline]
    pub fn alt(&self) -> bool {
        self.modifiers.alt_key()
    }
}

// ============================================================================
// Hit Targets
// ============================================================================

/// Logical targets in the UI that can receive mouse events.
///
/// These are returned by hit-testing and used by event handlers to dispatch
/// behavior. The variants carry enough context to handle the event without
/// re-querying the model.
///
/// Note: Some variant fields are not currently read but are populated for
/// future use (e.g., context menus, detailed click handling).
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum HitTarget {
    /// Modal overlay (command palette, goto line, find/replace, etc.)
    /// `inside` indicates whether the click was inside or outside the modal bounds
    Modal { inside: bool },

    /// Status bar at the bottom of the window
    StatusBar,

    /// Sidebar resize handle (the border between sidebar and editor area)
    SidebarResize,

    /// Sidebar file tree area (but not on a specific item)
    SidebarEmpty,

    /// A specific item in the sidebar file tree
    SidebarItem {
        path: PathBuf,
        row: usize,
        is_dir: bool,
        clicked_on_chevron: bool,
    },

    /// A splitter bar between split panes
    Splitter { index: usize },

    /// Header area of a preview pane (can be middle-clicked to close)
    PreviewHeader { preview_id: PreviewId },

    /// Content area of a preview pane (webview or native rendering)
    PreviewContent { preview_id: PreviewId },

    /// A specific tab in a group's tab bar
    GroupTab {
        group_id: GroupId,
        tab_index: usize,
        tab_id: TabId,
    },

    /// Empty area of a group's tab bar (no specific tab)
    GroupTabBarEmpty { group_id: GroupId },

    /// Editor gutter (line numbers)
    EditorGutter {
        group_id: GroupId,
        editor_id: EditorId,
        line: usize,
    },

    /// Editor text content area
    EditorContent {
        group_id: GroupId,
        editor_id: EditorId,
        document_id: DocumentId,
    },

    /// A cell in CSV grid view mode
    CsvCell {
        group_id: GroupId,
        editor_id: EditorId,
        row: usize,
        col: usize,
    },

    /// Dock resize handle (between dock and editor area)
    DockResize {
        position: crate::panel::DockPosition,
    },

    /// A tab in a dock's tab bar
    DockTab {
        position: crate::panel::DockPosition,
        panel_id: crate::panel::PanelId,
    },

    /// Empty area of a dock's tab bar (no specific tab)
    DockTabBarEmpty {
        position: crate::panel::DockPosition,
    },

    /// Dock content area (the active panel's content)
    DockContent {
        position: crate::panel::DockPosition,
        active_panel_id: crate::panel::PanelId,
    },
}

impl HitTarget {
    /// Get the group ID if this target is associated with an editor group
    #[allow(dead_code)]
    pub fn group_id(&self) -> Option<GroupId> {
        match self {
            HitTarget::GroupTab { group_id, .. }
            | HitTarget::GroupTabBarEmpty { group_id }
            | HitTarget::EditorGutter { group_id, .. }
            | HitTarget::EditorContent { group_id, .. }
            | HitTarget::CsvCell { group_id, .. } => Some(*group_id),
            _ => None,
        }
    }

    /// Get the suggested focus target for this hit
    #[allow(dead_code)]
    pub fn suggested_focus(&self) -> Option<FocusTarget> {
        match self {
            HitTarget::Modal { .. } => Some(FocusTarget::Modal),
            HitTarget::SidebarEmpty | HitTarget::SidebarItem { .. } => Some(FocusTarget::Sidebar),
            HitTarget::GroupTab { .. }
            | HitTarget::GroupTabBarEmpty { .. }
            | HitTarget::EditorGutter { .. }
            | HitTarget::EditorContent { .. }
            | HitTarget::CsvCell { .. } => Some(FocusTarget::Editor),
            // Dock content areas suggest sidebar focus for left dock (file explorer),
            // editor focus for others (until we have FocusTarget::Dock)
            HitTarget::DockTab { position, .. }
            | HitTarget::DockTabBarEmpty { position }
            | HitTarget::DockContent { position, .. } => {
                match position {
                    crate::panel::DockPosition::Left => Some(FocusTarget::Sidebar),
                    _ => Some(FocusTarget::Editor), // TODO: FocusTarget::Dock(position)
                }
            }
            // These don't change focus
            HitTarget::StatusBar
            | HitTarget::SidebarResize
            | HitTarget::DockResize { .. }
            | HitTarget::Splitter { .. }
            | HitTarget::PreviewHeader { .. }
            | HitTarget::PreviewContent { .. } => None,
        }
    }
}

// ============================================================================
// Event Results
// ============================================================================

/// Result of handling a mouse event
#[derive(Clone, Debug)]
pub enum EventResult {
    /// Event was fully handled; stop propagation
    Consumed {
        /// Whether a redraw is needed (ignored if cmd is Some)
        redraw: bool,
        /// Optional focus change to apply
        focus: Option<FocusTarget>,
        /// Optional command to execute (takes precedence over redraw flag)
        cmd: Option<crate::commands::Cmd>,
    },

    /// Event was not handled by this target; allow fallback handling
    Bubble,
}

impl EventResult {
    /// Create a consumed result that requests redraw but no focus change
    pub fn consumed_redraw() -> Self {
        Self::Consumed {
            redraw: true,
            focus: None,
            cmd: None,
        }
    }

    /// Create a consumed result with focus change and redraw
    pub fn consumed_with_focus(focus: FocusTarget) -> Self {
        Self::Consumed {
            redraw: true,
            focus: Some(focus),
            cmd: None,
        }
    }

    /// Create a consumed result with no redraw (event blocked but nothing changed)
    pub fn consumed_no_redraw() -> Self {
        Self::Consumed {
            redraw: false,
            focus: None,
            cmd: None,
        }
    }

    /// Create a consumed result with a command and focus change
    pub fn consumed_with_cmd(cmd: Option<crate::commands::Cmd>, focus: FocusTarget) -> Self {
        Self::Consumed {
            redraw: cmd.is_none(), // Only set redraw if no cmd
            focus: Some(focus),
            cmd,
        }
    }

    /// Check if this result requests a redraw
    #[allow(dead_code)]
    pub fn needs_redraw(&self) -> bool {
        match self {
            Self::Consumed { redraw, cmd, .. } => cmd.is_some() || *redraw,
            Self::Bubble => false,
        }
    }

    /// Get the command from this result
    #[allow(dead_code)]
    pub fn cmd(&self) -> Option<&crate::commands::Cmd> {
        match self {
            Self::Consumed { cmd, .. } => cmd.as_ref(),
            Self::Bubble => None,
        }
    }
}

// ============================================================================
// Hit-Testing Functions
// ============================================================================

/// Hit-test the modal overlay.
///
/// If a modal is active, returns `HitTarget::Modal` with `inside` indicating
/// whether the point is inside the modal bounds. This allows the caller to
/// decide whether to close the modal (outside click) or handle the click
/// inside the modal.
pub fn hit_test_modal(model: &AppModel, pt: Point) -> Option<HitTarget> {
    if !model.ui.has_modal() {
        return None;
    }

    let ww = model.window_size.0 as usize;
    let wh = model.window_size.1 as usize;
    let lh = model.line_height;

    // Use the same layout functions as rendering — single source of truth
    let layout = match &model.ui.active_modal {
        Some(ModalState::CommandPalette(state)) => {
            let input_text = state.input();
            let (l, _) =
                super::geometry::command_palette_layout(ww, wh, lh, filter_commands(&input_text).len());
            l
        }
        Some(ModalState::FileFinder(state)) => {
            let (l, _) = super::geometry::file_finder_layout(
                ww, wh, lh, state.results.len(), !state.input().is_empty(),
            );
            l
        }
        Some(ModalState::ThemePicker(state)) => {
            use crate::theme::ThemeSource;
            let themes = &state.themes;
            let has_user = themes.iter().any(|t| t.source == ThemeSource::User);
            let has_builtin = themes.iter().any(|t| t.source == ThemeSource::Builtin);
            let section_count = has_user as usize + has_builtin as usize;
            let total_rows = themes.len() + section_count;
            let (l, _) = super::geometry::theme_picker_layout(ww, wh, lh, total_rows);
            l
        }
        Some(ModalState::GotoLine(_)) => {
            let (l, _) = super::geometry::goto_line_layout(ww, wh, lh);
            l
        }
        Some(ModalState::FindReplace(state)) => {
            let (l, _) = super::geometry::find_replace_layout(ww, wh, lh, state.replace_mode);
            l
        }
        Some(ModalState::RecentFiles(state)) => {
            let filtered = state.filtered_entries();
            let (l, _) = super::geometry::file_finder_layout(
                ww, wh, lh, filtered.len(), !state.input().is_empty(),
            );
            l
        }
        None => return None,
    };

    let inside = layout.contains(pt.x as usize, pt.y as usize);

    Some(HitTarget::Modal { inside })
}

/// Hit-test the status bar at the bottom of the window.
pub fn hit_test_status_bar(model: &AppModel, pt: Point) -> Option<HitTarget> {
    if is_in_status_bar(pt.y, model.window_size.1, model.line_height) {
        Some(HitTarget::StatusBar)
    } else {
        None
    }
}

/// Hit-test the sidebar resize handle.
///
/// Returns `SidebarResize` if the point is within the resize hit zone
/// (a few pixels on either side of the sidebar border).
pub fn hit_test_sidebar_resize(model: &AppModel, pt: Point) -> Option<HitTarget> {
    const SIDEBAR_RESIZE_HIT_ZONE: f64 = 4.0;

    let workspace = model.workspace.as_ref()?;
    if !workspace.sidebar_visible {
        return None;
    }

    let sidebar_width = workspace.sidebar_width(model.metrics.scale_factor) as f64;
    let resize_zone_start = sidebar_width - SIDEBAR_RESIZE_HIT_ZONE;
    let resize_zone_end = sidebar_width + SIDEBAR_RESIZE_HIT_ZONE;

    if pt.x >= resize_zone_start && pt.x <= resize_zone_end {
        Some(HitTarget::SidebarResize)
    } else {
        None
    }
}

/// Hit-test the sidebar file tree.
///
/// Returns `SidebarItem` if clicking on a file/folder, or `SidebarEmpty`
/// if clicking in the sidebar area but not on an item.
pub fn hit_test_sidebar(model: &AppModel, pt: Point) -> Option<HitTarget> {
    let workspace = model.workspace.as_ref()?;
    if !workspace.sidebar_visible {
        return None;
    }

    let sidebar_width = workspace.sidebar_width(model.metrics.scale_factor) as f64;
    if pt.x >= sidebar_width {
        return None;
    }

    let row_height = model.metrics.file_tree_row_height as f64;
    let indent = model.metrics.file_tree_indent as f64;
    let clicked_visual_row = (pt.y / row_height) as usize;
    let clicked_row = workspace.scroll_offset.saturating_add(clicked_visual_row);

    if let Some((node, depth)) = workspace
        .file_tree
        .get_visible_item_with_depth(clicked_row, &workspace.expanded_folders)
    {
        let chevron_start = (depth as f64 * indent) + 8.0;
        let chevron_end = chevron_start + 16.0;
        let clicked_on_chevron = node.is_dir && pt.x >= chevron_start && pt.x < chevron_end;

        Some(HitTarget::SidebarItem {
            path: node.path.clone(),
            row: clicked_row,
            is_dir: node.is_dir,
            clicked_on_chevron,
        })
    } else {
        Some(HitTarget::SidebarEmpty)
    }
}

/// Hit-test splitter bars between split panes.
///
/// Requires the pre-computed splitters from `EditorArea::compute_layout_scaled()`.
pub fn hit_test_splitters(
    _model: &AppModel,
    pt: Point,
    splitters: &[crate::model::editor_area::SplitterBar],
) -> Option<HitTarget> {
    for (i, splitter) in splitters.iter().enumerate() {
        if splitter.rect.contains(pt.x as f32, pt.y as f32) {
            return Some(HitTarget::Splitter { index: i });
        }
    }
    None
}

/// Hit-test preview panes.
///
/// Returns `PreviewHeader` if clicking on the header area, or `PreviewContent`
/// if clicking in the content area.
pub fn hit_test_previews(model: &AppModel, pt: Point) -> Option<HitTarget> {
    for (&preview_id, preview) in &model.editor_area.previews {
        if preview.rect.contains(pt.x as f32, pt.y as f32) {
            let pane = Pane::with_header(preview.rect, &model.metrics);
            if pane.is_in_header(pt.x, pt.y) {
                return Some(HitTarget::PreviewHeader { preview_id });
            } else {
                return Some(HitTarget::PreviewContent { preview_id });
            }
        }
    }
    None
}

/// Hit-test editor groups (tab bar and content area).
///
/// Returns `GroupTab` if clicking on a specific tab, `GroupTabBarEmpty` if
/// clicking in the tab bar but not on a tab, or `EditorContent`/`CsvCell`
/// if clicking in the editor content area.
pub fn hit_test_groups(model: &AppModel, pt: Point, char_width: f32) -> Option<HitTarget> {
    let tab_bar_height = model.metrics.tab_bar_height;

    // First check which group contains the point
    let group_id = model.editor_area.group_at_point(pt.x as f32, pt.y as f32)?;
    let group = model.editor_area.groups.get(&group_id)?;

    // Check if in tab bar
    if is_in_group_tab_bar(pt.y, &group.rect, tab_bar_height)
        && pt.x >= group.rect.x as f64
        && pt.x < (group.rect.x + group.rect.width) as f64
    {
        let local_x = pt.x - group.rect.x as f64;

        // Find which tab was clicked using geometry
        if let Some(tab_index) = super::geometry::tab_at_position(local_x, char_width, model, group)
        {
            if let Some(tab) = group.tabs.get(tab_index) {
                return Some(HitTarget::GroupTab {
                    group_id,
                    tab_index,
                    tab_id: tab.id,
                });
            }
        }

        return Some(HitTarget::GroupTabBarEmpty { group_id });
    }

    // Get the active editor for this group
    let editor_id = group.active_editor_id()?;
    let editor = model.editor_area.editors.get(&editor_id)?;
    let doc_id = editor.document_id?;

    // Check if in CSV mode
    if editor.view_mode.is_csv() {
        // For CSV mode, we could compute the exact cell here
        // For now, return a placeholder that the caller can refine
        return Some(HitTarget::CsvCell {
            group_id,
            editor_id,
            row: 0,
            col: 0,
        });
    }

    // Check if in gutter area
    let gutter_width = crate::model::gutter_border_x_scaled(char_width, &model.metrics) as f64;
    let gutter_x_end = group.rect.x as f64 + gutter_width;
    let content_y_start = group.rect.y as f64 + tab_bar_height as f64;

    if pt.x >= group.rect.x as f64 && pt.x < gutter_x_end && pt.y >= content_y_start {
        // Compute which line was clicked
        let local_y = pt.y - content_y_start;
        let line = (local_y / model.line_height as f64) as usize + editor.viewport.top_line;
        return Some(HitTarget::EditorGutter {
            group_id,
            editor_id,
            line,
        });
    }

    // Editor content area
    Some(HitTarget::EditorContent {
        group_id,
        editor_id,
        document_id: doc_id,
    })
}

/// Hit-test dock panels (right and bottom docks).
///
/// Computes dock rectangles and checks if the point falls within any open dock.
/// Returns `DockContent` with the active panel ID for content clicks.
pub fn hit_test_docks(model: &AppModel, pt: Point) -> Option<HitTarget> {
    let scale = model.metrics.scale_factor;
    let w = model.window_size.0 as f32;
    let status_bar_h = model.line_height as f32;
    let content_h = model.window_size.1 as f32 - status_bar_h;
    let bottom_h = model.dock_layout.bottom.size(scale);
    let side_h = content_h - bottom_h;

    // Right dock
    if model.dock_layout.right.is_open {
        let right_w = model.dock_layout.right.size(scale);
        if right_w > 0.0 {
            let right_rect = Rect::new(w - right_w, 0.0, right_w, side_h);
            if right_rect.contains(pt.x as f32, pt.y as f32) {
                let dock = &model.dock_layout.right;
                if let Some(panel_id) = dock.active_panel() {
                    return Some(HitTarget::DockContent {
                        position: crate::panel::DockPosition::Right,
                        active_panel_id: panel_id,
                    });
                }
                return Some(HitTarget::DockTabBarEmpty {
                    position: crate::panel::DockPosition::Right,
                });
            }
        }
    }

    // Bottom dock
    if model.dock_layout.bottom.is_open && bottom_h > 0.0 {
        let left_w = model.dock_layout.left.size(scale);
        let bottom_rect = Rect::new(left_w, side_h, w - left_w, bottom_h);
        if bottom_rect.contains(pt.x as f32, pt.y as f32) {
            let dock = &model.dock_layout.bottom;
            if let Some(panel_id) = dock.active_panel() {
                return Some(HitTarget::DockContent {
                    position: crate::panel::DockPosition::Bottom,
                    active_panel_id: panel_id,
                });
            }
            return Some(HitTarget::DockTabBarEmpty {
                position: crate::panel::DockPosition::Bottom,
            });
        }
    }

    None
}

/// Main hit-testing function that checks all UI regions in priority order.
///
/// Returns the highest-priority `HitTarget` at the given point, or `None`
/// if the point is not over any interactive region.
///
/// # Priority Order (highest first)
/// 1. Modal overlay (blocks everything when active)
/// 2. Status bar (always on top at bottom of window)
/// 3. Sidebar resize handle
/// 4. Sidebar file tree
/// 5. Dock panels (right, bottom)
/// 6. Splitter bars
/// 7. Preview panes (header and content)
/// 8. Editor groups (tab bar, gutter, content)
pub fn hit_test_ui(model: &AppModel, pt: Point, char_width: f32) -> Option<HitTarget> {
    // 1. Modal overlay (highest priority)
    if let Some(target) = hit_test_modal(model, pt) {
        return Some(target);
    }

    // 2. Status bar
    if let Some(target) = hit_test_status_bar(model, pt) {
        return Some(target);
    }

    // 3. Sidebar resize handle
    if let Some(target) = hit_test_sidebar_resize(model, pt) {
        return Some(target);
    }

    // 4. Sidebar file tree
    if let Some(target) = hit_test_sidebar(model, pt) {
        return Some(target);
    }

    // 5. Dock panels (must be checked before editor groups, which may overlap)
    if let Some(target) = hit_test_docks(model, pt) {
        return Some(target);
    }

    // 6. Splitter bars (need to compute layout first)
    let sidebar_width = model
        .workspace
        .as_ref()
        .filter(|ws| ws.sidebar_visible)
        .map(|ws| ws.sidebar_width(model.metrics.scale_factor))
        .unwrap_or(0.0);

    let status_bar_height = model.line_height as f32;
    let available = model.editor_area.last_layout_rect.unwrap_or(Rect::new(
        sidebar_width,
        0.0,
        model.window_size.0 as f32 - sidebar_width,
        model.window_size.1 as f32 - status_bar_height,
    ));

    // Note: This creates a temporary copy of splitters; in production code
    // the splitters should be passed in or cached
    let splitters = model
        .editor_area
        .clone() // Avoid borrow issues
        .compute_layout_scaled(available, model.metrics.splitter_width);

    if let Some(target) = hit_test_splitters(model, pt, &splitters) {
        return Some(target);
    }

    // 7. Preview panes
    if let Some(target) = hit_test_previews(model, pt) {
        return Some(target);
    }

    // 8. Editor groups
    hit_test_groups(model, pt, char_width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_new() {
        let pt = Point::new(100.0, 200.0);
        assert_eq!(pt.x, 100.0);
        assert_eq!(pt.y, 200.0);
    }

    #[test]
    fn test_mouse_event_helpers() {
        let event = MouseEvent::new(
            50.0,
            50.0,
            MouseButton::Left,
            ElementState::Pressed,
            1,
            ModifiersState::empty(),
        );
        assert!(event.is_pressed());
        assert!(!event.is_released());
        assert!(!event.shift());
        assert!(!event.ctrl());
        assert!(!event.alt());
    }

    #[test]
    fn test_hit_target_suggested_focus() {
        let modal = HitTarget::Modal { inside: true };
        assert_eq!(modal.suggested_focus(), Some(FocusTarget::Modal));

        let sidebar = HitTarget::SidebarEmpty;
        assert_eq!(sidebar.suggested_focus(), Some(FocusTarget::Sidebar));

        let splitter = HitTarget::Splitter { index: 0 };
        assert_eq!(splitter.suggested_focus(), None);
    }

    #[test]
    fn test_event_result_helpers() {
        let consumed = EventResult::consumed_redraw();
        assert!(consumed.needs_redraw());

        let no_redraw = EventResult::consumed_no_redraw();
        assert!(!no_redraw.needs_redraw());

        let bubble = EventResult::Bubble;
        assert!(!bubble.needs_redraw());
    }
}
