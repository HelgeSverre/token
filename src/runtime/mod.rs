//! Runtime module - winit/platform integration
//!
//! This module contains platform-specific code for running the editor:
//! - `app` - ApplicationHandler and window management
//! - `input` - Keyboard/mouse event to message mapping
//! - `mouse` - Unified mouse event handling with hit-testing
//! - `webview` - Webview management for markdown preview

pub mod app;
pub mod input;
pub mod mouse;
pub mod webview;

pub use app::App;
