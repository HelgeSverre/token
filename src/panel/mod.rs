//! Panel system - unified docking panel abstraction
//!
//! This module provides a flexible docking panel system inspired by VS Code and IntelliJ.
//! Panels can be docked in left, right, or bottom positions, with tabs for multiple panels
//! per dock.
//!
//! ## Architecture
//!
//! - `DockPosition`: Left, Right, or Bottom dock location
//! - `PanelId`: Unique identifier for panel types (file explorer, terminal, etc.)
//! - `Dock`: State for a single dock container (panels, active tab, size, visibility)
//! - `DockLayout`: Complete layout state with all three docks
//! - `Panel` trait: Interface for implementing new panel types (future)
//!
//! ## Integration
//!
//! The dock system integrates with:
//! - Hit-testing via `HitTarget::Dock*` variants in `view/hit_test.rs`
//! - Mouse handling via `Msg::Dock` dispatch in `runtime/mouse.rs`
//! - Layout geometry via `compute_dock_rects()` in `view/geometry.rs`

mod dock;

pub use dock::{Dock, DockLayout, DockPosition, PanelId};
