//! Runtime module - winit/platform integration
//!
//! This module contains platform-specific code for running the editor:
//! - `app` - ApplicationHandler and window management
//! - `input` - Keyboard/mouse event to message mapping
//! - `perf` - Performance overlay (debug builds only)

pub mod app;
pub mod input;
pub mod perf;

pub use app::App;
