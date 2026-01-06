//! UI state - status bar, cursor blink, modals, and other UI concerns

use super::editor_area::SplitDirection;
use super::status_bar::{StatusBar, TransientMessage};
use crate::editable::{EditConstraints, EditableState, StringBuffer};
use crate::theme::{list_available_themes, ThemeInfo};
use std::path::PathBuf;
use std::time::{Duration, Instant};

// ============================================================================
// Focus Management
// ============================================================================

/// Which top-level UI region currently has keyboard focus
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusTarget {
    /// Main editor text area (default)
    #[default]
    Editor,
    /// File tree sidebar
    Sidebar,
    /// Modal dialog (command palette, goto line, find/replace, etc.)
    Modal,
}

/// Which UI region the mouse is currently hovering over
///
/// Used for:
/// - Determining scroll event targets (sidebar vs editor)
/// - Setting appropriate cursor icons
/// - Visual hover feedback
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HoverRegion {
    /// Not hovering over any tracked region
    #[default]
    None,
    /// Hovering over the sidebar file tree
    Sidebar,
    /// Hovering over the sidebar resize handle
    SidebarResize,
    /// Hovering over the editor text area
    EditorText,
    /// Hovering over the editor tab bar
    EditorTabBar,
    /// Hovering over the status bar
    StatusBar,
    /// Hovering over a modal dialog
    Modal,
    /// Hovering over a splitter (split view resize handle)
    Splitter,
}

// ============================================================================
// Modal System
// ============================================================================

/// Identifies which modal is currently active
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalId {
    /// Command palette (Shift+Cmd+A)
    CommandPalette,
    /// Go to line dialog (Cmd+L)
    GotoLine,
    /// Find/Replace dialog (Cmd+F)
    FindReplace,
    /// Theme picker
    ThemePicker,
}

/// State for the command palette modal
#[derive(Debug, Clone)]
pub struct CommandPaletteState {
    /// Editable state for the input field
    pub editable: EditableState<StringBuffer>,
    /// Index of selected command in filtered list
    pub selected_index: usize,
}

impl Default for CommandPaletteState {
    fn default() -> Self {
        Self {
            editable: EditableState::new(StringBuffer::new(), EditConstraints::single_line()),
            selected_index: 0,
        }
    }
}

impl CommandPaletteState {
    /// Get the input text (convenience accessor)
    pub fn input(&self) -> String {
        self.editable.text()
    }

    /// Set the input text (replaces content)
    pub fn set_input(&mut self, text: &str) {
        self.editable.set_content(text);
    }
}

/// State for the goto line modal
#[derive(Debug, Clone)]
pub struct GotoLineState {
    /// Editable state for the input field (numeric + colon only)
    pub editable: EditableState<StringBuffer>,
}

impl Default for GotoLineState {
    fn default() -> Self {
        Self {
            editable: EditableState::new(StringBuffer::new(), EditConstraints::goto_line()),
        }
    }
}

impl GotoLineState {
    /// Get the input text (convenience accessor)
    pub fn input(&self) -> String {
        self.editable.text()
    }

    /// Set the input text (replaces content)
    pub fn set_input(&mut self, text: &str) {
        self.editable.set_content(text);
    }
}

/// Which field is focused in find/replace
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FindReplaceField {
    #[default]
    Query,
    Replace,
}

/// State for the find/replace modal
#[derive(Debug, Clone)]
pub struct FindReplaceState {
    /// Editable state for the query field
    pub query_editable: EditableState<StringBuffer>,
    /// Editable state for the replacement field
    pub replace_editable: EditableState<StringBuffer>,
    /// Which field is currently focused
    pub focused_field: FindReplaceField,
    /// Whether replace mode is active (vs find-only)
    pub replace_mode: bool,
    /// Case-sensitive search
    pub case_sensitive: bool,
}

impl Default for FindReplaceState {
    fn default() -> Self {
        Self {
            query_editable: EditableState::new(StringBuffer::new(), EditConstraints::single_line()),
            replace_editable: EditableState::new(
                StringBuffer::new(),
                EditConstraints::single_line(),
            ),
            focused_field: FindReplaceField::Query,
            replace_mode: false,
            case_sensitive: false,
        }
    }
}

impl FindReplaceState {
    /// Get the query text (convenience accessor)
    pub fn query(&self) -> String {
        self.query_editable.text()
    }

    /// Set the query text (replaces content)
    pub fn set_query(&mut self, text: &str) {
        self.query_editable.set_content(text);
    }

    /// Get the replacement text (convenience accessor)
    pub fn replacement(&self) -> String {
        self.replace_editable.text()
    }

    /// Set the replacement text (replaces content)
    pub fn set_replacement(&mut self, text: &str) {
        self.replace_editable.set_content(text);
    }

    /// Get the currently focused editable state
    pub fn focused_editable(&self) -> &EditableState<StringBuffer> {
        match self.focused_field {
            FindReplaceField::Query => &self.query_editable,
            FindReplaceField::Replace => &self.replace_editable,
        }
    }

    /// Get the currently focused editable state mutably
    pub fn focused_editable_mut(&mut self) -> &mut EditableState<StringBuffer> {
        match self.focused_field {
            FindReplaceField::Query => &mut self.query_editable,
            FindReplaceField::Replace => &mut self.replace_editable,
        }
    }

    /// Toggle focus between query and replacement fields
    pub fn toggle_field(&mut self) {
        self.focused_field = match self.focused_field {
            FindReplaceField::Query => FindReplaceField::Replace,
            FindReplaceField::Replace => FindReplaceField::Query,
        };
    }
}

/// State for the theme picker modal
#[derive(Debug, Clone)]
pub struct ThemePickerState {
    /// Index of selected theme in list
    pub selected_index: usize,
    /// Cached list of available themes (refreshed when modal opens)
    pub themes: Vec<ThemeInfo>,
}

impl Default for ThemePickerState {
    fn default() -> Self {
        Self {
            selected_index: 0,
            themes: list_available_themes(),
        }
    }
}

/// Union of all modal states
#[derive(Debug, Clone)]
pub enum ModalState {
    CommandPalette(CommandPaletteState),
    GotoLine(GotoLineState),
    FindReplace(FindReplaceState),
    ThemePicker(ThemePickerState),
}

impl ModalState {
    /// Get the modal ID for this state
    pub fn id(&self) -> ModalId {
        match self {
            ModalState::CommandPalette(_) => ModalId::CommandPalette,
            ModalState::GotoLine(_) => ModalId::GotoLine,
            ModalState::FindReplace(_) => ModalId::FindReplace,
            ModalState::ThemePicker(_) => ModalId::ThemePicker,
        }
    }
}

// ============================================================================
// Drop State (file drag-and-drop feedback)
// ============================================================================

/// State for file drag-and-drop visual feedback
#[derive(Debug, Clone, Default)]
pub struct DropState {
    /// Files currently being hovered over the window
    pub hovered_files: Vec<PathBuf>,
    /// Whether files are currently being dragged over the window
    pub is_hovering: bool,
}

impl DropState {
    /// Start hovering with a file
    pub fn start_hover(&mut self, path: PathBuf) {
        if !self.hovered_files.contains(&path) {
            self.hovered_files.push(path);
        }
        self.is_hovering = true;
    }

    /// Cancel the hover (user dragged away)
    pub fn cancel_hover(&mut self) {
        self.hovered_files.clear();
        self.is_hovering = false;
    }

    /// Get display text for the hover overlay
    pub fn display_text(&self) -> String {
        match self.hovered_files.len() {
            0 => String::new(),
            1 => {
                let filename = self.hovered_files[0]
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "file".to_string());
                format!("Drop to open: {}", filename)
            }
            n => format!("Drop to open {} files", n),
        }
    }
}

// ============================================================================
// Splitter Drag State
// ============================================================================

/// State for splitter (resize handle) dragging
#[derive(Debug, Clone)]
pub struct SplitterDragState {
    /// Index of the splitter being dragged (into the splitters vec from compute_layout)
    pub splitter_index: usize,
    /// Local index within the container (which children boundary)
    pub local_index: usize,
    /// Starting mouse position when drag began (pixels)
    pub start_position: (f32, f32),
    /// Original ratios before drag started (for cancel/restore)
    pub original_ratios: Vec<f32>,
    /// Direction of the split (determines which axis to track)
    pub direction: SplitDirection,
    /// Container's total size in the drag direction (pixels)
    pub container_size: f32,
    /// Whether threshold exceeded (true = actively dragging with visual updates)
    pub active: bool,
}

// ============================================================================
// Sidebar Resize State
// ============================================================================

/// State for sidebar resize dragging
#[derive(Debug, Clone)]
pub struct SidebarResizeState {
    /// Starting mouse X position when drag began
    pub start_x: f64,
    /// Original sidebar width (logical pixels) before drag started
    pub original_width: f32,
}

/// UI state - status messages and cursor animation
#[derive(Debug, Clone)]
pub struct UiState {
    /// Message displayed in the status bar (legacy, kept for compatibility)
    pub status_message: String,
    /// Structured status bar with segments
    pub status_bar: StatusBar,
    /// Transient message with auto-expiry
    pub transient_message: Option<TransientMessage>,
    /// Whether the cursor is currently visible (for blinking)
    pub cursor_visible: bool,
    /// Timestamp of last cursor blink state change
    pub last_cursor_blink: Instant,
    /// Whether a file is currently being loaded
    pub is_loading: bool,
    /// Whether a file is currently being saved
    pub is_saving: bool,
    /// Currently active modal (if any)
    pub active_modal: Option<ModalState>,
    /// Last command palette state (persisted for quick re-execution)
    pub last_command_palette: Option<CommandPaletteState>,
    /// Last find/replace state (persisted for quick re-use)
    pub last_find_replace: Option<FindReplaceState>,
    /// File drag-and-drop state
    pub drop_state: DropState,
    /// Splitter (resize handle) drag state
    pub splitter_drag: Option<SplitterDragState>,
    /// Sidebar resize drag state
    pub sidebar_resize: Option<SidebarResizeState>,
    /// Which UI region has keyboard focus
    pub focus: FocusTarget,
    /// Which UI region the mouse is currently hovering over
    pub hover: HoverRegion,
    /// Lines that contained cursors in the previous frame (for damage tracking)
    /// Used by cursor blink to determine which lines need redrawing
    pub previous_cursor_lines: Vec<usize>,
}

impl UiState {
    /// Create a new UI state with default settings
    pub fn new() -> Self {
        Self {
            status_message: String::new(),
            status_bar: StatusBar::new(),
            transient_message: None,
            cursor_visible: true,
            last_cursor_blink: Instant::now(),
            is_loading: false,
            is_saving: false,
            active_modal: None,
            last_command_palette: None,
            last_find_replace: None,
            drop_state: DropState::default(),
            splitter_drag: None,
            sidebar_resize: None,
            focus: FocusTarget::Editor,
            hover: HoverRegion::None,
            previous_cursor_lines: Vec::new(),
        }
    }

    /// Create a UI state with an initial status message
    pub fn with_status(message: impl Into<String>) -> Self {
        Self {
            status_message: message.into(),
            status_bar: StatusBar::new(),
            transient_message: None,
            cursor_visible: true,
            last_cursor_blink: Instant::now(),
            is_loading: false,
            is_saving: false,
            active_modal: None,
            last_command_palette: None,
            last_find_replace: None,
            drop_state: DropState::default(),
            splitter_drag: None,
            sidebar_resize: None,
            focus: FocusTarget::Editor,
            hover: HoverRegion::None,
            previous_cursor_lines: Vec::new(),
        }
    }

    // =========================================================================
    // Focus Management
    // =========================================================================

    /// Check if a modal is currently active
    pub fn has_modal(&self) -> bool {
        self.active_modal.is_some()
    }

    /// Open a modal (also sets focus to Modal)
    pub fn open_modal(&mut self, state: ModalState) {
        self.active_modal = Some(state);
        self.focus = FocusTarget::Modal;
    }

    /// Close the active modal (returns focus to Editor)
    pub fn close_modal(&mut self) {
        self.active_modal = None;
        self.focus = FocusTarget::Editor;
    }

    /// Set focus to the editor
    pub fn focus_editor(&mut self) {
        if self.focus != FocusTarget::Editor {
            tracing::trace!("Focus changed: {:?} -> Editor", self.focus);
            self.focus = FocusTarget::Editor;
        }
    }

    /// Set focus to the sidebar
    pub fn focus_sidebar(&mut self) {
        if self.focus != FocusTarget::Sidebar {
            tracing::trace!("Focus changed: {:?} -> Sidebar", self.focus);
            self.focus = FocusTarget::Sidebar;
        }
    }

    /// Set focus to a modal (prefer using open_modal instead)
    pub fn focus_modal(&mut self) {
        if self.focus != FocusTarget::Modal {
            tracing::trace!("Focus changed: {:?} -> Modal", self.focus);
            self.focus = FocusTarget::Modal;
        }
    }

    /// Reset cursor blink timer (call after user input)
    pub fn reset_cursor_blink(&mut self) {
        self.cursor_visible = true;
        self.last_cursor_blink = Instant::now();
    }

    /// Update cursor blink state based on elapsed time
    /// Returns true if the state changed (needs redraw)
    pub fn update_cursor_blink(&mut self, blink_interval: Duration) -> bool {
        if self.last_cursor_blink.elapsed() >= blink_interval {
            self.cursor_visible = !self.cursor_visible;
            self.last_cursor_blink = Instant::now();
            true
        } else {
            false
        }
    }

    /// Set the status message
    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status_message = message.into();
    }

    /// Check if the UI is busy (loading or saving)
    pub fn is_busy(&self) -> bool {
        self.is_loading || self.is_saving
    }
}

impl Default for UiState {
    fn default() -> Self {
        Self::new()
    }
}
