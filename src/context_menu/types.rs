//! Data structures for the context menu (`docs/feature/context-menu.md`
//! "Data Structures").

use std::path::PathBuf;

use crate::commands::CommandId;
use crate::messages::Msg;
use crate::model::editor_area::{GroupId, TabId};

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
        has_selection: bool,
        clipboard_has_content: bool,
    },
    /// Right-click on a tab. `file_path` is `None` for an untitled buffer.
    Tab {
        group_id: GroupId,
        tab_id: TabId,
        file_path: Option<PathBuf>,
    },
    /// Right-click on a file/folder in the file tree.
    FileTreeItem { path: PathBuf, is_dir: bool },
}

impl ContextMenuTarget {
    pub fn region(&self) -> ContextMenuRegion {
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
    /// Display keybinding string (e.g. `"⌘B"`), taken from the
    /// `commands::COMMANDS` registry — `None` for unbound/targeted items.
    pub shortcut_hint: Option<String>,
    pub action: MenuAction,
    pub is_separator: bool,
}

impl MenuItem {
    /// A row that runs `id` through the palette's `execute_command`,
    /// enabled per `enabled`, hint taken from the command registry.
    pub fn from_command(id: CommandId, label: &str, enabled: bool) -> Self {
        let shortcut_hint = crate::commands::command_def(id)
            .and_then(|def| def.keybinding)
            .map(str::to_owned);
        Self {
            label: label.to_owned(),
            enabled,
            shortcut_hint,
            action: MenuAction::Command(id),
            is_separator: false,
        }
    }

    /// A row with a targeted `Msg` action (a specific tab/path), no
    /// keybinding hint (targeted actions aren't bound to a key).
    pub fn custom(label: &str, enabled: bool, msgs: Vec<Msg>) -> Self {
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
    pub fn separator() -> Self {
        Self {
            label: String::new(),
            enabled: false,
            shortcut_hint: None,
            action: MenuAction::None,
            is_separator: true,
        }
    }
}

/// Non-separator items, in display order — the addressing space every
/// consumer shares (`Body::List`'s `FlatIndex` via
/// `view::modal::context_menu_sections`, keyboard nav in
/// `runtime/input.rs`, and `ContextMenuMsg::ActivateItem`'s `index`), so a
/// separator is transparently non-addressable everywhere at once
/// (context-menu.md "Separators").
pub fn selectable_items(items: &[MenuItem]) -> impl Iterator<Item = &MenuItem> {
    items.iter().filter(|item| !item.is_separator)
}

/// The selectable-space index (`selectable_items`' order) of the first
/// enabled item, or `0` if none are enabled (an all-disabled menu is a
/// degenerate case — nothing to select, but `selected` still needs a
/// value).
pub fn first_enabled_index(items: &[MenuItem]) -> usize {
    selectable_items(items)
        .position(|item| item.enabled)
        .unwrap_or(0)
}
