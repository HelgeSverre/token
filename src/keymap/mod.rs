//! Configurable keyboard mapping system
//!
//! This module provides a data-driven keybinding system that:
//! - Maps keystrokes to editor commands
//! - Supports platform-specific modifier handling (Cmd on macOS, Ctrl elsewhere)
//! - Enables user customization via YAML config files
//! - Supports multi-key sequences/chords
//!
//! # Architecture
//!
//! ```text
//! winit::KeyEvent → Keystroke → Keymap::lookup() → Command → Vec<Msg>
//! ```
//!
//! # Loading Keymaps
//!
//! ```ignore
//! // Load from embedded defaults
//! let keymap = Keymap::with_bindings(default_bindings());
//!
//! // Or load from YAML file
//! let keymap = Keymap::load_from_yaml("keymap.yaml")?;
//! ```

mod binding;
mod command;
mod config;
mod context;
mod defaults;
#[allow(clippy::module_inception)]
mod keymap;
mod types;
mod winit_adapter;

pub use binding::Keybinding;
pub use command::Command;
pub use config::{load_keymap_file, parse_keymap_yaml, KeymapError};
pub use context::{Condition, KeyContext};
pub use defaults::{
    default_bindings, get_default_keymap_yaml, load_default_keymap, merge_bindings,
};
pub use keymap::{KeyAction, Keymap};
pub use types::{KeyCode, Keystroke, Modifiers};
pub use winit_adapter::keystroke_from_winit;

#[cfg(test)]
mod tests;
