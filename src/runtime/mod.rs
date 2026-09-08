//! Runtime module - winit/platform integration
//!
//! This module contains platform-specific code for running the editor:
//! - `app` - ApplicationHandler and window management
//! - `input` - Keyboard/mouse event to message mapping
//! - `mouse` - Unified mouse event handling with hit-testing
//! - `webview` - Webview management for markdown preview

pub mod app;
mod clipboard;
mod configuration;
mod file_io;
mod file_watch;
mod find_worker;
mod inline_cache;
mod inline_context;
mod inline_retrieval;
mod inline_server;
mod inline_statistics;
pub mod inline_worker;
pub mod input;
mod keymap_settings;
mod latest_worker;
mod lsp_slot;
#[cfg(target_os = "macos")]
mod macos_menu;
pub mod mouse;
mod path_completion;
pub mod webview;

pub use app::{App, AppPreparation};

/// Shared external navigation boundary for preview and terminal links.
fn open_web_url(url: String) {
    if token::util::is_web_url(&url) {
        std::thread::spawn(move || {
            if let Err(error) = open::that(url) {
                tracing::warn!("Failed to open browser: {error}");
            }
        });
    }
}
