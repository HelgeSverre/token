//! Right-click context menu (`docs/feature/context-menu.md`).
//!
//! `types` owns the region-agnostic data (`ContextMenuTarget`, `MenuItem`,
//! `MenuAction`); `builders` maps a target to its `Vec<MenuItem>` per the
//! V1 region tables. Both are pure/update-layer: no `Keymap` threading (menu
//! items take their keybinding hint straight from the `commands::COMMANDS`
//! registry), no I/O (the caller — `runtime/mouse.rs`'s `handle_right_click`
//! or `App::open_context_menu_at_caret` — resolves anything requiring I/O,
//! such as clipboard content, before building the target).

pub mod builders;
pub mod types;

pub use builders::build_menu;
pub use types::{
    first_enabled_index, selectable_items, ContextMenuRegion, ContextMenuTarget, MenuAction,
    MenuItem,
};
