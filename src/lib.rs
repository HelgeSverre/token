//! Rust Editor - Elm-style text editor
//!
//! This crate provides the core types and logic for a minimal text editor
//! implementing the Elm Architecture pattern.

pub mod cli;
pub mod commands;
pub mod config;
pub mod config_paths;
pub mod csv;
#[cfg(debug_assertions)]
pub mod debug_overlay;
pub mod keymap;
pub mod messages;
pub mod model;
pub mod overlay;
pub mod syntax;
pub mod theme;
pub mod tracing;
pub mod update;
pub mod util;

pub mod rendering {
    //! Rendering utilities exposed for benchmarks

    /// Blend a foreground color onto a background color with a separate alpha value.
    ///
    /// This is the integer-based blend used for glyph rendering where the alpha
    /// comes from the glyph bitmap coverage value.
    #[inline]
    pub fn blend_pixel_u8(bg: u32, fg: u32, alpha: u8) -> u32 {
        let a = alpha as u32;
        let inv_a = 255 - a;

        let bg_r = (bg >> 16) & 0xFF;
        let bg_g = (bg >> 8) & 0xFF;
        let bg_b = bg & 0xFF;

        let fg_r = (fg >> 16) & 0xFF;
        let fg_g = (fg >> 8) & 0xFF;
        let fg_b = fg & 0xFF;

        let r = (fg_r * a + bg_r * inv_a) / 255;
        let g = (fg_g * a + bg_g * inv_a) / 255;
        let b = (fg_b * a + bg_b * inv_a) / 255;

        0xFF000000 | (r << 16) | (g << 8) | b
    }
}

// Re-export commonly used types
pub use commands::Cmd;
pub use config::EditorConfig;
pub use messages::Msg;
pub use model::AppModel;
pub use theme::Theme;
