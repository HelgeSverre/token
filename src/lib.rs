//! Rust Editor - Elm-style text editor
//!
//! This crate provides the core types and logic for a minimal text editor
//! implementing the Elm Architecture pattern.

pub mod commands;
pub mod messages;
pub mod model;
pub mod overlay;
pub mod theme;
pub mod update;
pub mod util;

// Re-export commonly used types
pub use commands::Cmd;
pub use messages::Msg;
pub use model::AppModel;
pub use theme::Theme;
