//! Placeholder panel for prototyping
//!
//! A simple panel that displays its name and a placeholder message.
//! Used for Terminal, Outline, and other panels during development.

use crate::panel::PanelId;

/// Placeholder panel state
#[derive(Debug, Clone)]
pub struct PlaceholderPanel {
    pub panel_id: PanelId,
}

impl PlaceholderPanel {
    pub fn new(panel_id: PanelId) -> Self {
        Self { panel_id }
    }

    /// Get the title for this placeholder
    pub fn title(&self) -> &'static str {
        self.panel_id.display_name()
    }

    /// Get placeholder message
    pub fn message(&self) -> &'static str {
        match self.panel_id {
            PanelId::Terminal => "Terminal panel coming soon...",
            PanelId::Outline => "Code outline coming soon...",
            PanelId::TaskRunner => "Task runner coming soon...",
            PanelId::AiChat => "AI chat coming soon...",
            PanelId::TodoList => "TODO list coming soon...",
            PanelId::FileExplorer => "File explorer",
        }
    }
}
