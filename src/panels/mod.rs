//! Panel implementations for the docking system
//!
//! This module contains concrete panel implementations that can be
//! displayed in docks. Each panel implements rendering and interaction
//! logic for its specific functionality.
//!
//! ## Available Panels
//!
//! - **PlaceholderPanel**: Generic placeholder for prototyping
//! - **FileExplorer**: File tree sidebar (wraps existing workspace sidebar)
//! - **Terminal**: Terminal emulator (future)
//! - **Outline**: Code outline/structure view (future)

mod placeholder;

pub use placeholder::PlaceholderPanel;

use crate::panel::PanelId;

/// Get a display title for a panel
pub fn panel_title(panel_id: PanelId) -> &'static str {
    panel_id.display_name()
}

/// Get an icon for a panel (Nerd Font icon)
pub fn panel_icon(panel_id: PanelId) -> &'static str {
    match panel_id {
        PanelId::FileExplorer => "󰙅", // file tree
        PanelId::Outline => "",       // list/outline
        PanelId::Terminal => "",      // terminal
        PanelId::TaskRunner => "",    // tasks/play
        PanelId::AiChat => "",        // chat/comment
        PanelId::TodoList => "",      // checklist
    }
}
