//! UI state - status bar, cursor blink, modals, and other UI concerns

use super::editor_area::SplitDirection;
use super::status_bar::{StatusBar, TransientMessage};
use crate::theme::{list_available_themes, ThemeInfo};
use std::path::PathBuf;
use std::time::{Duration, Instant};

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
#[derive(Debug, Clone, Default)]
pub struct CommandPaletteState {
    /// Current input text
    pub input: String,
    /// Index of selected command in filtered list
    pub selected_index: usize,
}

/// State for the goto line modal
#[derive(Debug, Clone, Default)]
pub struct GotoLineState {
    /// Current input text (line number)
    pub input: String,
}

/// State for the find/replace modal
#[derive(Debug, Clone, Default)]
pub struct FindReplaceState {
    /// Search query
    pub query: String,
    /// Replacement text
    pub replacement: String,
    /// Whether replace mode is active (vs find-only)
    pub replace_mode: bool,
    /// Case-sensitive search
    pub case_sensitive: bool,
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
    /// File drag-and-drop state
    pub drop_state: DropState,
    /// Splitter (resize handle) drag state
    pub splitter_drag: Option<SplitterDragState>,
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
            drop_state: DropState::default(),
            splitter_drag: None,
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
            drop_state: DropState::default(),
            splitter_drag: None,
        }
    }

    /// Check if a modal is currently active
    pub fn has_modal(&self) -> bool {
        self.active_modal.is_some()
    }

    /// Open a modal
    pub fn open_modal(&mut self, state: ModalState) {
        self.active_modal = Some(state);
    }

    /// Close the active modal
    pub fn close_modal(&mut self) {
        self.active_modal = None;
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
