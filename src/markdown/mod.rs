//! Markdown preview module
//!
//! Provides live Markdown preview with webview rendering and scroll synchronization.

mod preview;
mod renderer;
mod theme;

pub use preview::{MarkdownStyle, PreviewPane, RenderedLine, StyledSegment};
pub use renderer::{content_to_preview_html, markdown_to_html};
pub use theme::PreviewTheme;
