//! Rust Editor - Elm-style text editor
//!
//! This crate provides the core types and logic for a minimal text editor
//! implementing the Elm Architecture pattern.

pub mod commands;
pub mod config;
#[cfg(debug_assertions)]
pub mod debug_overlay;
pub mod keymap;
pub mod messages;
pub mod model;
pub mod overlay;
pub mod theme;
pub mod tracing;
pub mod update;
pub mod util;

// Re-export commonly used types
pub use commands::Cmd;
pub use config::EditorConfig;
pub use messages::Msg;
pub use model::AppModel;
pub use theme::Theme;
