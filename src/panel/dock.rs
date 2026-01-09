//! Dock containers and layout state
//!
//! This module defines the core data structures for the docking panel system.

use serde::{Deserialize, Serialize};

/// Position where a dock can be placed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DockPosition {
    Left,
    Right,
    Bottom,
}

impl DockPosition {
    /// Returns the axis this dock expands along
    pub fn axis(&self) -> Axis {
        match self {
            DockPosition::Left | DockPosition::Right => Axis::Vertical,
            DockPosition::Bottom => Axis::Horizontal,
        }
    }

    /// Cycle to next dock position
    pub fn cycle_next(&self) -> DockPosition {
        match self {
            DockPosition::Left => DockPosition::Bottom,
            DockPosition::Bottom => DockPosition::Right,
            DockPosition::Right => DockPosition::Left,
        }
    }

    /// All dock positions for iteration
    pub const ALL: [DockPosition; 3] = [
        DockPosition::Left,
        DockPosition::Right,
        DockPosition::Bottom,
    ];
}

/// Axis for dock sizing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

/// Unique identifier for a panel type
///
/// Panel IDs are used for persistence, keybindings, and registry lookup.
/// Uses an enum internally for efficient comparison and serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PanelId {
    FileExplorer,
    Outline,
    Terminal,
    TaskRunner,
    AiChat,
    TodoList,
}

impl PanelId {
    pub const FILE_EXPLORER: PanelId = PanelId::FileExplorer;
    pub const OUTLINE: PanelId = PanelId::Outline;
    pub const TERMINAL: PanelId = PanelId::Terminal;
    pub const TASK_RUNNER: PanelId = PanelId::TaskRunner;
    pub const AI_CHAT: PanelId = PanelId::AiChat;
    pub const TODO_LIST: PanelId = PanelId::TodoList;

    /// Get the display name for this panel
    pub fn display_name(&self) -> &'static str {
        match self {
            PanelId::FileExplorer => "Explorer",
            PanelId::Outline => "Outline",
            PanelId::Terminal => "Terminal",
            PanelId::TaskRunner => "Tasks",
            PanelId::AiChat => "Chat",
            PanelId::TodoList => "TODOs",
        }
    }

    /// Get the default dock position for this panel
    pub fn default_position(&self) -> DockPosition {
        match self {
            PanelId::FileExplorer => DockPosition::Left,
            PanelId::Outline => DockPosition::Right,
            PanelId::Terminal | PanelId::TaskRunner | PanelId::TodoList => DockPosition::Bottom,
            PanelId::AiChat => DockPosition::Right,
        }
    }
}

/// State for a single dock (left, right, or bottom)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dock {
    /// Position of this dock
    pub position: DockPosition,

    /// Panels registered to this dock (by ID)
    pub panel_ids: Vec<PanelId>,

    /// Currently active panel index (None if dock has no panels)
    pub active_index: Option<usize>,

    /// Whether the dock is visible/open
    pub is_open: bool,

    /// Size in logical pixels (width for left/right, height for bottom)
    pub size_logical: f32,
}

impl Dock {
    /// Create a new dock at the given position
    pub fn new(position: DockPosition) -> Self {
        Self {
            position,
            panel_ids: Vec::new(),
            active_index: None,
            is_open: false,
            size_logical: match position {
                DockPosition::Left | DockPosition::Right => 250.0,
                DockPosition::Bottom => 200.0,
            },
        }
    }

    /// Get the active panel ID, if any
    pub fn active_panel(&self) -> Option<PanelId> {
        self.active_index
            .and_then(|i| self.panel_ids.get(i).copied())
    }

    /// Register a panel to this dock
    pub fn register_panel(&mut self, panel_id: PanelId) {
        if !self.panel_ids.contains(&panel_id) {
            self.panel_ids.push(panel_id);
            // If this is the first panel, make it active
            if self.active_index.is_none() {
                self.active_index = Some(0);
            }
        }
    }

    /// Activate a panel by ID, opening the dock if closed
    pub fn activate(&mut self, panel_id: PanelId) {
        if let Some(index) = self.panel_ids.iter().position(|id| *id == panel_id) {
            self.active_index = Some(index);
            self.is_open = true;
        }
    }

    /// Close the dock
    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// Toggle dock visibility
    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }

    /// Physical size accounting for scale factor
    pub fn size(&self, scale_factor: f64) -> f32 {
        if self.is_open {
            self.size_logical * scale_factor as f32
        } else {
            0.0
        }
    }

    /// Set size from physical pixels
    pub fn set_size(&mut self, physical_size: f32, scale_factor: f64) {
        self.size_logical = physical_size / scale_factor as f32;
    }

    /// Minimum size in logical pixels
    pub fn min_size(&self) -> f32 {
        150.0
    }

    /// Maximum size as fraction of window dimension
    pub fn max_size_fraction(&self) -> f32 {
        0.5
    }

    /// Cycle to next panel in this dock
    pub fn next_panel(&mut self) {
        if self.panel_ids.len() > 1 {
            if let Some(current) = self.active_index {
                self.active_index = Some((current + 1) % self.panel_ids.len());
            }
        }
    }

    /// Cycle to previous panel in this dock
    pub fn prev_panel(&mut self) {
        if self.panel_ids.len() > 1 {
            if let Some(current) = self.active_index {
                let len = self.panel_ids.len();
                self.active_index = Some((current + len - 1) % len);
            }
        }
    }

    /// Check if this dock has any panels
    pub fn has_panels(&self) -> bool {
        !self.panel_ids.is_empty()
    }
}

/// Complete dock layout state
///
/// NOTE: Focus is NOT stored here. Focus lives in `UiState::focus` as
/// `FocusTarget::Dock(DockPosition)`. Use `UiState::focused_dock()` to query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockLayout {
    pub left: Dock,
    pub right: Dock,
    pub bottom: Dock,
}

impl Default for DockLayout {
    fn default() -> Self {
        let mut layout = Self {
            left: Dock::new(DockPosition::Left),
            right: Dock::new(DockPosition::Right),
            bottom: Dock::new(DockPosition::Bottom),
        };

        // Register default panels
        layout.left.register_panel(PanelId::FILE_EXPLORER);
        layout.right.register_panel(PanelId::OUTLINE);
        layout.bottom.register_panel(PanelId::TERMINAL);

        // Left dock (file explorer) is open by default
        layout.left.is_open = true;

        layout
    }
}

impl DockLayout {
    /// Get dock by position
    pub fn dock(&self, position: DockPosition) -> &Dock {
        match position {
            DockPosition::Left => &self.left,
            DockPosition::Right => &self.right,
            DockPosition::Bottom => &self.bottom,
        }
    }

    /// Get mutable dock by position
    pub fn dock_mut(&mut self, position: DockPosition) -> &mut Dock {
        match position {
            DockPosition::Left => &mut self.left,
            DockPosition::Right => &mut self.right,
            DockPosition::Bottom => &mut self.bottom,
        }
    }

    /// Find which dock contains a panel
    pub fn find_panel(&self, panel_id: PanelId) -> Option<DockPosition> {
        DockPosition::ALL
            .into_iter()
            .find(|&pos| self.dock(pos).panel_ids.contains(&panel_id))
    }

    /// Focus-then-toggle logic for panel keybindings (Cmd+1, Cmd+7, etc.)
    ///
    /// Behavior:
    /// - If the target dock is NOT focused: open dock, activate panel, return (true, Some(position))
    /// - If the target dock IS focused on this panel: close dock, return (false, None)
    /// - If the target dock IS focused on a DIFFERENT panel: switch to this panel
    ///
    /// The caller should update focus based on return value:
    /// - (true, Some(pos)) → focus Dock(pos)
    /// - (false, None) → focus Editor
    pub fn focus_or_toggle_panel(
        &mut self,
        panel_id: PanelId,
        is_dock_focused: impl Fn(DockPosition) -> bool,
    ) -> (bool, Option<DockPosition>) {
        let Some(position) = self.find_panel(panel_id) else {
            return (false, None); // Panel not registered
        };

        let is_target_dock_focused = is_dock_focused(position);
        let dock = self.dock_mut(position);
        let is_panel_active = dock.active_panel() == Some(panel_id);

        if is_target_dock_focused && is_panel_active && dock.is_open {
            // Already focused on this panel → close and unfocus
            dock.close();
            (false, None)
        } else {
            // Not focused on this panel → open, activate, focus
            dock.activate(panel_id);
            (true, Some(position))
        }
    }

    /// Close dock at given position
    pub fn close_dock(&mut self, position: DockPosition) {
        self.dock_mut(position).close();
    }

    /// Toggle dock visibility at given position
    pub fn toggle_dock(&mut self, position: DockPosition) {
        self.dock_mut(position).toggle();
    }

    /// Cycle to next panel in the specified dock
    pub fn next_panel_in_dock(&mut self, position: DockPosition) {
        self.dock_mut(position).next_panel();
    }

    /// Cycle to previous panel in the specified dock
    pub fn prev_panel_in_dock(&mut self, position: DockPosition) {
        self.dock_mut(position).prev_panel();
    }

    /// Get the total width consumed by side docks (for editor area calculation)
    pub fn side_docks_width(&self, scale_factor: f64) -> f32 {
        self.left.size(scale_factor) + self.right.size(scale_factor)
    }

    /// Get the height consumed by bottom dock
    pub fn bottom_dock_height(&self, scale_factor: f64) -> f32 {
        self.bottom.size(scale_factor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dock_position_cycle() {
        assert_eq!(DockPosition::Left.cycle_next(), DockPosition::Bottom);
        assert_eq!(DockPosition::Bottom.cycle_next(), DockPosition::Right);
        assert_eq!(DockPosition::Right.cycle_next(), DockPosition::Left);
    }

    #[test]
    fn test_panel_id_defaults() {
        assert_eq!(
            PanelId::FILE_EXPLORER.default_position(),
            DockPosition::Left
        );
        assert_eq!(PanelId::TERMINAL.default_position(), DockPosition::Bottom);
        assert_eq!(PanelId::OUTLINE.default_position(), DockPosition::Right);
    }

    #[test]
    fn test_dock_panel_registration() {
        let mut dock = Dock::new(DockPosition::Left);
        assert!(!dock.has_panels());
        assert_eq!(dock.active_panel(), None);

        dock.register_panel(PanelId::FILE_EXPLORER);
        assert!(dock.has_panels());
        assert_eq!(dock.active_panel(), Some(PanelId::FILE_EXPLORER));

        dock.register_panel(PanelId::OUTLINE);
        assert_eq!(dock.panel_ids.len(), 2);
        // First panel still active
        assert_eq!(dock.active_panel(), Some(PanelId::FILE_EXPLORER));
    }

    #[test]
    fn test_dock_panel_cycling() {
        let mut dock = Dock::new(DockPosition::Left);
        dock.register_panel(PanelId::FILE_EXPLORER);
        dock.register_panel(PanelId::OUTLINE);
        dock.is_open = true;

        assert_eq!(dock.active_panel(), Some(PanelId::FILE_EXPLORER));

        dock.next_panel();
        assert_eq!(dock.active_panel(), Some(PanelId::OUTLINE));

        dock.next_panel();
        assert_eq!(dock.active_panel(), Some(PanelId::FILE_EXPLORER));

        dock.prev_panel();
        assert_eq!(dock.active_panel(), Some(PanelId::OUTLINE));
    }

    #[test]
    fn test_dock_size_with_visibility() {
        let mut dock = Dock::new(DockPosition::Left);
        dock.size_logical = 250.0;

        // Closed dock has zero size
        dock.is_open = false;
        assert_eq!(dock.size(1.0), 0.0);

        // Open dock has its size
        dock.is_open = true;
        assert_eq!(dock.size(1.0), 250.0);
        assert_eq!(dock.size(2.0), 500.0);
    }

    #[test]
    fn test_dock_layout_default() {
        let layout = DockLayout::default();

        // Left dock should have file explorer and be open
        assert!(layout.left.has_panels());
        assert!(layout.left.is_open);
        assert_eq!(layout.left.active_panel(), Some(PanelId::FILE_EXPLORER));

        // Right and bottom should be closed
        assert!(!layout.right.is_open);
        assert!(!layout.bottom.is_open);
    }

    #[test]
    fn test_dock_layout_find_panel() {
        let layout = DockLayout::default();

        assert_eq!(
            layout.find_panel(PanelId::FILE_EXPLORER),
            Some(DockPosition::Left)
        );
        assert_eq!(
            layout.find_panel(PanelId::OUTLINE),
            Some(DockPosition::Right)
        );
        assert_eq!(
            layout.find_panel(PanelId::TERMINAL),
            Some(DockPosition::Bottom)
        );
        assert_eq!(layout.find_panel(PanelId::AI_CHAT), None);
    }

    #[test]
    fn test_focus_or_toggle_panel() {
        let mut layout = DockLayout::default();

        // Toggle file explorer when not focused → opens and focuses
        let (opened, pos) = layout.focus_or_toggle_panel(PanelId::FILE_EXPLORER, |_| false);
        assert!(opened);
        assert_eq!(pos, Some(DockPosition::Left));
        assert!(layout.left.is_open);

        // Toggle again when focused → closes
        let (opened, pos) = layout.focus_or_toggle_panel(PanelId::FILE_EXPLORER, |p| {
            p == DockPosition::Left
        });
        assert!(!opened);
        assert_eq!(pos, None);
        assert!(!layout.left.is_open);
    }
}
