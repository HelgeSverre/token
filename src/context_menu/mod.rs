//! Context-menu construction, addressing, navigation, and activation policy.
//!
//! Callers identify a target and supply clipboard availability (which needs
//! runtime I/O). This module derives enablement and hints from the model and
//! keeps separators out of the row indices shared by input, rendering, and
//! automation.

mod builders;

use std::path::PathBuf;

use crate::commands::CommandId;
use crate::messages::Msg;
use crate::model::editor_area::{GroupId, TabId};
use crate::model::AppModel;

/// Which UI region spawned the menu — V1 covers three; the rest are Future.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextMenuRegion {
    Editor,
    EditorTabBar,
    FileTree,
}

/// Detailed context about what was right-clicked, built from the
/// `HitTarget` hit-testing already resolved for the click (or, for
/// Shift+F10, from the live caret/selection state).
#[derive(Debug, Clone)]
pub enum ContextMenuTarget {
    /// Right-click in editor text area.
    Editor {
        group_id: GroupId,
        clipboard_has_content: bool,
    },
    /// Right-click on a tab, which need not be the active tab.
    Tab { group_id: GroupId, tab_id: TabId },
    /// Right-click on a file/folder in the file tree.
    FileTreeItem { path: PathBuf, is_dir: bool },
}

impl ContextMenuTarget {
    fn region(&self) -> ContextMenuRegion {
        match self {
            ContextMenuTarget::Editor { .. } => ContextMenuRegion::Editor,
            ContextMenuTarget::Tab { .. } => ContextMenuRegion::EditorTabBar,
            ContextMenuTarget::FileTreeItem { .. } => ContextMenuRegion::FileTree,
        }
    }
}

/// What activating a `MenuItem` does.
#[derive(Debug, Clone)]
pub enum MenuAction {
    /// Routes through `update::execute_command` — inherits palette
    /// behavior (and its keybinding hint) exactly.
    Command(CommandId),
    /// Targeted actions that need data the palette's `CommandId` path
    /// doesn't carry (a specific tab/path, not "the focused document").
    /// Dispatched through `update()` in order.
    Messages(Vec<Msg>),
    /// Disabled items / separators have nothing to run.
    None,
}

/// One row of a context menu, or a separator (`is_separator`, rendered as a
/// `Section` boundary — see `view::modal::context_menu_sections`).
#[derive(Debug, Clone)]
pub struct MenuItem {
    pub label: String,
    pub enabled: bool,
    /// Display keybinding string (e.g. `"⌘B"`) from the active keymap.
    pub shortcut_hint: Option<String>,
    pub(crate) action: MenuAction,
    pub(crate) is_separator: bool,
}

impl MenuItem {
    /// A row that runs `id` through the palette's `execute_command`,
    /// enabled per `enabled`. The builder resolves hints from the active keymap.
    pub(crate) fn from_command(id: CommandId, label: &str, enabled: bool) -> Self {
        Self {
            label: label.to_owned(),
            enabled,
            shortcut_hint: None,
            action: MenuAction::Command(id),
            is_separator: false,
        }
    }

    /// A row with a targeted `Msg` action (a specific tab/path), no
    /// keybinding hint (targeted actions aren't bound to a key).
    pub(crate) fn custom(label: &str, enabled: bool, msgs: Vec<Msg>) -> Self {
        Self {
            label: label.to_owned(),
            enabled,
            shortcut_hint: None,
            action: MenuAction::Messages(msgs),
            is_separator: false,
        }
    }

    /// A section-boundary marker — not addressable by keyboard nav or
    /// click (`view::modal::context_menu_sections`).
    pub(crate) fn separator() -> Self {
        Self {
            label: String::new(),
            enabled: false,
            shortcut_hint: None,
            action: MenuAction::None,
            is_separator: true,
        }
    }
}

/// An open-time snapshot. The anchor is `(x, y, height)` in pixels: zero
/// height for a click, the caret height for a keyboard-triggered menu.
#[derive(Debug, Clone)]
pub struct ContextMenuState {
    pub(crate) items: Vec<MenuItem>,
    pub anchor: (usize, usize, usize),
    pub region: ContextMenuRegion,
}

impl ContextMenuState {
    pub fn new(
        model: &AppModel,
        target: &ContextMenuTarget,
        anchor: (usize, usize, usize),
    ) -> Self {
        Self {
            items: builders::build_menu(model, target),
            anchor,
            region: target.region(),
        }
    }

    /// Addressable rows in display order, including disabled rows.
    pub fn rows(&self) -> impl Iterator<Item = &MenuItem> {
        self.items.iter().filter(|item| !item.is_separator)
    }

    /// Nonempty section ranges in the same address space as `rows`.
    pub fn sections(&self) -> impl Iterator<Item = std::ops::Range<usize>> + '_ {
        let mut start = 0;
        self.items
            .split(|item| item.is_separator)
            .filter(|items| !items.is_empty())
            .map(move |items| {
                let end = start + items.len();
                let range = start..end;
                start = end;
                range
            })
    }

    /// An all-disabled menu has no selection, represented by index zero.
    pub fn initial_selection(&self) -> usize {
        self.rows().position(|item| item.enabled).unwrap_or(0)
    }

    /// Wrap past disabled rows; leave selection alone if none can be selected.
    pub fn next_selection(&self, selected: usize, forward: bool) -> usize {
        let enabled: Vec<bool> = self.rows().map(|item| item.enabled).collect();
        let len = enabled.len();
        if len == 0 {
            return selected;
        }
        let mut index = selected.min(len - 1);
        for _ in 0..len {
            index = if forward {
                (index + 1) % len
            } else {
                (index + len - 1) % len
            };
            if enabled[index] {
                return index;
            }
        }
        selected
    }

    /// Disabled and out-of-range rows cannot activate or dismiss the menu.
    pub fn action(&self, index: usize) -> Option<&MenuAction> {
        self.rows()
            .nth(index)
            .filter(|item| item.enabled)
            .map(|item| &item.action)
    }
}
