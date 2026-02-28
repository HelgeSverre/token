# Image Viewer with Pan & Zoom — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add an image viewer (PNG/JPEG/GIF/BMP/WebP) to the editor with pan, zoom, checkerboard transparency, and status bar info, following the existing `ViewMode` pattern.

**Architecture:** New `ViewMode::Image(Box<ImageState>)` variant following the CSV pattern. Image files bypass binary rejection, are decoded with the `image` crate, and rendered via CPU blitting with nearest-neighbor scaling. Scroll wheel zooms toward cursor, click-drag pans.

**Tech Stack:** `image` crate (decode), softbuffer (render), fontdue (existing), winit (existing input events)

**Design doc:** `docs/plans/2026-02-28-image-viewer-pan-zoom-design.md`

---

### Task 1: Enable image format decoding in Cargo.toml

**Files:**
- Modify: `Cargo.toml:64`

**Step 1: Update image crate features**

Change:
```toml
image = { version = "0.25", default-features = false, features = ["png"] }
```
To:
```toml
image = { version = "0.25", default-features = false, features = ["png", "jpeg", "gif", "bmp", "webp"] }
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles successfully

**Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "feat(image): enable jpeg, gif, bmp, webp decoding in image crate"
```

---

### Task 2: Add ImageState, DragState, and ImageMsg types

**Files:**
- Create: `src/image/mod.rs`
- Modify: `src/lib.rs:10` (add `pub mod image;`)
- Modify: `src/messages.rs` (add `ImageMsg` enum and `Msg::Image` variant)

**Step 1: Create `src/image/mod.rs` with core types**

```rust
//! Image viewer module
//!
//! Provides image viewing with pan and zoom support.
//! Images are decoded into RGBA pixel buffers and rendered
//! with nearest-neighbor scaling.

pub mod render;

/// State for the image viewer mode
#[derive(Debug, Clone)]
pub struct ImageState {
    /// Decoded RGBA pixel data (4 bytes per pixel)
    pub pixels: Vec<u8>,
    /// Image width in pixels
    pub width: u32,
    /// Image height in pixels
    pub height: u32,
    /// File size in bytes (for status bar)
    pub file_size: u64,
    /// Image format name (e.g. "PNG", "JPEG")
    pub format: String,
    /// Current zoom level (1.0 = 100%)
    pub scale: f64,
    /// Pan offset X in image-space pixels
    pub offset_x: f64,
    /// Pan offset Y in image-space pixels
    pub offset_y: f64,
    /// Whether the user has manually zoomed (disables auto-fit on resize)
    pub user_zoomed: bool,
    /// Last known mouse position (screen coords, for zoom-toward-cursor)
    pub last_mouse_x: f64,
    /// Last known mouse position (screen coords, for zoom-toward-cursor)
    pub last_mouse_y: f64,
    /// Active drag state for panning
    pub drag: Option<DragState>,
}

/// Drag state for click-and-drag panning
#[derive(Debug, Clone)]
pub struct DragState {
    /// Mouse position when drag started (screen coords)
    pub start_mouse_x: f64,
    pub start_mouse_y: f64,
    /// Image offset when drag started
    pub start_offset_x: f64,
    pub start_offset_y: f64,
}

impl ImageState {
    /// Create a new ImageState from decoded image data.
    ///
    /// Computes initial scale: fit-to-viewport if image is larger,
    /// actual size (1.0) if image fits.
    pub fn new(
        pixels: Vec<u8>,
        width: u32,
        height: u32,
        file_size: u64,
        format: String,
        viewport_width: u32,
        viewport_height: u32,
    ) -> Self {
        let scale = Self::compute_fit_scale(width, height, viewport_width, viewport_height);
        Self {
            pixels,
            width,
            height,
            file_size,
            format,
            scale,
            offset_x: 0.0,
            offset_y: 0.0,
            user_zoomed: false,
            last_mouse_x: 0.0,
            last_mouse_y: 0.0,
            drag: None,
        }
    }

    /// Compute scale to fit image within viewport.
    /// Returns 1.0 if image already fits, otherwise scales down.
    pub fn compute_fit_scale(
        img_width: u32,
        img_height: u32,
        viewport_width: u32,
        viewport_height: u32,
    ) -> f64 {
        if viewport_width == 0 || viewport_height == 0 {
            return 1.0;
        }
        let scale_x = viewport_width as f64 / img_width as f64;
        let scale_y = viewport_height as f64 / img_height as f64;
        let fit_scale = scale_x.min(scale_y);
        // Only scale down, never scale up for auto-fit
        fit_scale.min(1.0)
    }

    /// Get the zoom level as a percentage integer (e.g. 100 for 1.0)
    pub fn zoom_percent(&self) -> u32 {
        (self.scale * 100.0).round() as u32
    }

    /// Format file size for display (e.g. "2.4 MB", "128 KB")
    pub fn file_size_display(&self) -> String {
        if self.file_size >= 1_048_576 {
            format!("{:.1} MB", self.file_size as f64 / 1_048_576.0)
        } else if self.file_size >= 1024 {
            format!("{:.0} KB", self.file_size as f64 / 1024.0)
        } else {
            format!("{} B", self.file_size)
        }
    }
}
```

**Step 2: Create placeholder `src/image/render.rs`**

```rust
//! Image rendering for the view layer
//!
//! Handles checkerboard background, image blitting with scaling,
//! and centering within the viewport.
```

**Step 3: Add module declaration to `src/lib.rs`**

After line `pub mod csv;` (line 10), add:
```rust
pub mod image;
```

**Step 4: Add `ImageMsg` to `src/messages.rs`**

After the `CsvMsg` enum (after line 522), add:

```rust
/// Image viewer messages (pan, zoom)
#[derive(Debug, Clone)]
pub enum ImageMsg {
    /// Zoom in/out by delta, centered on cursor position
    Zoom { delta: f64, mouse_x: f64, mouse_y: f64 },
    /// Start drag-pan at screen position
    StartPan { x: f64, y: f64 },
    /// Update drag-pan to new screen position
    UpdatePan { x: f64, y: f64 },
    /// End drag-pan
    EndPan,
    /// Fit image to viewport
    FitToWindow,
    /// Show at actual size (1:1 pixels)
    ActualSize,
    /// Track mouse position for zoom-toward-cursor
    MouseMove { x: f64, y: f64 },
    /// Viewport was resized — recalculate fit if in auto-fit mode
    ViewportResized { width: u32, height: u32 },
}
```

**Step 5: Add `Msg::Image` variant**

In the `Msg` enum (around line 652), add after `Csv(CsvMsg)`:
```rust
    /// Image viewer messages
    Image(ImageMsg),
```

Update the import at the top of `messages.rs` is not needed since `ImageMsg` is defined in the same file.

**Step 6: Verify it compiles**

Run: `cargo check`
Expected: warnings about unused code, but no errors

**Step 7: Commit**

```bash
git add src/image/mod.rs src/image/render.rs src/lib.rs src/messages.rs
git commit -m "feat(image): add ImageState, DragState, ImageMsg types"
```

---

### Task 3: Add ViewMode::Image variant

**Files:**
- Modify: `src/model/editor.rs:272-299`

**Step 1: Add Image variant to ViewMode enum**

In the `ViewMode` enum (line 273), add after `Csv(Box<CsvState>)`:
```rust
    /// Image viewer mode (decoded image with pan/zoom)
    Image(Box<crate::image::ImageState>),
```

**Step 2: Add helper methods in the `impl ViewMode` block**

After the `as_csv_mut()` method, add:
```rust
    /// Check if in image mode
    pub fn is_image(&self) -> bool {
        matches!(self, ViewMode::Image(_))
    }

    /// Get image state if in image mode
    pub fn as_image(&self) -> Option<&crate::image::ImageState> {
        match self {
            ViewMode::Image(state) => Some(state),
            _ => None,
        }
    }

    /// Get mutable image state if in image mode
    pub fn as_image_mut(&mut self) -> Option<&mut crate::image::ImageState> {
        match self {
            ViewMode::Image(state) => Some(state),
            _ => None,
        }
    }
```

**Step 3: Verify it compiles**

Run: `cargo check`
Expected: compiles, possible warnings about unused methods

**Step 4: Commit**

```bash
git add src/model/editor.rs
git commit -m "feat(image): add ViewMode::Image variant with helper methods"
```

---

### Task 4: Add image file detection and bypass binary rejection

**Files:**
- Modify: `src/util/file_validation.rs`
- Modify: `src/util/mod.rs:11` (re-export)

**Step 1: Add `is_image_file()` function to `src/util/file_validation.rs`**

After the `is_likely_binary()` function (after line 116), add:

```rust
/// Image file extensions supported by the viewer
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "bmp", "webp"];

/// Check if a file path has an image extension
pub fn is_image_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}
```

**Step 2: Add re-export in `src/util/mod.rs`**

Add `is_image_file` to the re-export line (line 11):
```rust
    filename_for_display, is_image_file, is_likely_binary, validate_file_for_opening, FileOpenError, MAX_FILE_SIZE,
```

**Step 3: Write tests**

Add to the `#[cfg(test)] mod tests` block at the bottom of `file_validation.rs`:

```rust
    #[test]
    fn test_is_image_file_png() {
        assert!(is_image_file(Path::new("photo.png")));
        assert!(is_image_file(Path::new("photo.PNG")));
    }

    #[test]
    fn test_is_image_file_jpeg() {
        assert!(is_image_file(Path::new("photo.jpg")));
        assert!(is_image_file(Path::new("photo.jpeg")));
    }

    #[test]
    fn test_is_image_file_other_formats() {
        assert!(is_image_file(Path::new("image.gif")));
        assert!(is_image_file(Path::new("image.bmp")));
        assert!(is_image_file(Path::new("image.webp")));
    }

    #[test]
    fn test_is_not_image_file() {
        assert!(!is_image_file(Path::new("code.rs")));
        assert!(!is_image_file(Path::new("data.csv")));
        assert!(!is_image_file(Path::new("readme.md")));
        assert!(!is_image_file(Path::new("noextension")));
    }
```

**Step 4: Run tests**

Run: `cargo test --lib util::file_validation`
Expected: all tests pass

**Step 5: Commit**

```bash
git add src/util/file_validation.rs src/util/mod.rs
git commit -m "feat(image): add is_image_file() detection for image extensions"
```

---

### Task 5: Add image update handler

**Files:**
- Create: `src/update/image.rs`
- Modify: `src/update/mod.rs`

**Step 1: Create `src/update/image.rs`**

```rust
//! Image viewer update handlers
//!
//! Processes ImageMsg messages to update pan/zoom state.

use crate::commands::Cmd;
use crate::messages::ImageMsg;
use crate::model::AppModel;

/// Minimum zoom level (10%)
const MIN_SCALE: f64 = 0.1;
/// Maximum zoom level (1000%)
const MAX_SCALE: f64 = 10.0;
/// Zoom sensitivity per scroll tick
const ZOOM_FACTOR: f64 = 0.1;

pub fn update_image(model: &mut AppModel, msg: ImageMsg) -> Option<Cmd> {
    // Get the focused editor's image state
    let editor_id = model.editor_area.focused_editor_id()?;

    match msg {
        ImageMsg::Zoom { delta, mouse_x, mouse_y } => {
            let editor = model.editor_area.editors.get_mut(&editor_id)?;
            let state = editor.view_mode.as_image_mut()?;

            // Compute the image-space point under the cursor before zoom
            let img_x = state.offset_x + mouse_x / state.scale;
            let img_y = state.offset_y + mouse_y / state.scale;

            // Apply zoom
            let factor = 1.0 + delta * ZOOM_FACTOR;
            let new_scale = (state.scale * factor).clamp(MIN_SCALE, MAX_SCALE);
            state.scale = new_scale;

            // Adjust offset so the cursor-point stays stationary
            state.offset_x = img_x - mouse_x / new_scale;
            state.offset_y = img_y - mouse_y / new_scale;

            state.user_zoomed = true;
            Some(Cmd::redraw_editor())
        }

        ImageMsg::StartPan { x, y } => {
            let editor = model.editor_area.editors.get_mut(&editor_id)?;
            let state = editor.view_mode.as_image_mut()?;

            state.drag = Some(crate::image::DragState {
                start_mouse_x: x,
                start_mouse_y: y,
                start_offset_x: state.offset_x,
                start_offset_y: state.offset_y,
            });
            Some(Cmd::redraw_editor())
        }

        ImageMsg::UpdatePan { x, y } => {
            let editor = model.editor_area.editors.get_mut(&editor_id)?;
            let state = editor.view_mode.as_image_mut()?;

            if let Some(drag) = &state.drag {
                let dx = (x - drag.start_mouse_x) / state.scale;
                let dy = (y - drag.start_mouse_y) / state.scale;
                state.offset_x = drag.start_offset_x - dx;
                state.offset_y = drag.start_offset_y - dy;
            }
            Some(Cmd::redraw_editor())
        }

        ImageMsg::EndPan => {
            let editor = model.editor_area.editors.get_mut(&editor_id)?;
            let state = editor.view_mode.as_image_mut()?;
            state.drag = None;
            Some(Cmd::redraw_editor())
        }

        ImageMsg::FitToWindow => {
            let editor = model.editor_area.editors.get_mut(&editor_id)?;
            let state = editor.view_mode.as_image_mut()?;
            // Use editor viewport dimensions (approximate from group rect)
            let group_id = model.editor_area.focused_group_id;
            let group = model.editor_area.groups.get(&group_id)?;
            let vw = group.rect.width as u32;
            let vh = group.rect.height.saturating_sub(model.metrics.tab_bar_height as f32) as u32;
            state.scale = crate::image::ImageState::compute_fit_scale(state.width, state.height, vw, vh);
            state.offset_x = 0.0;
            state.offset_y = 0.0;
            state.user_zoomed = false;
            Some(Cmd::redraw_editor())
        }

        ImageMsg::ActualSize => {
            let editor = model.editor_area.editors.get_mut(&editor_id)?;
            let state = editor.view_mode.as_image_mut()?;
            state.scale = 1.0;
            state.offset_x = 0.0;
            state.offset_y = 0.0;
            state.user_zoomed = true;
            Some(Cmd::redraw_editor())
        }

        ImageMsg::MouseMove { x, y } => {
            let editor = model.editor_area.editors.get_mut(&editor_id)?;
            let state = editor.view_mode.as_image_mut()?;
            state.last_mouse_x = x;
            state.last_mouse_y = y;
            None // No redraw needed for mouse tracking
        }

        ImageMsg::ViewportResized { width, height } => {
            let editor = model.editor_area.editors.get_mut(&editor_id)?;
            let state = editor.view_mode.as_image_mut()?;
            if !state.user_zoomed {
                state.scale = crate::image::ImageState::compute_fit_scale(
                    state.width, state.height, width, height,
                );
                state.offset_x = 0.0;
                state.offset_y = 0.0;
            }
            Some(Cmd::redraw_editor())
        }
    }
}
```

**Step 2: Register in `src/update/mod.rs`**

Add to module declarations (after `mod csv;` on line 6):
```rust
mod image;
```

Add to public re-exports (after `pub use csv::update_csv;` on line 29):
```rust
pub use image::update_image;
```

**Step 3: Wire up message dispatch**

In `update_inner()` (line 58), add after the `Msg::Csv(m)` arm (line 96):
```rust
        Msg::Image(m) => image::update_image(model, m),
```

Also add to the message interceptors for image mode. In the `Msg::Editor(m)` match arm (around line 60), after the CSV mode check, add:

```rust
            // When in image mode, block all editor messages
            let in_image_mode = model
                .editor_area
                .focused_editor()
                .map(|e| e.view_mode.is_image())
                .unwrap_or(false);

            if in_image_mode {
                return None;
            }
```

Similarly, in the `Msg::Document(m)` match arm (around line 76), after the CSV check:

```rust
            // When in image mode, block all document messages
            let in_image_mode = model
                .editor_area
                .focused_editor()
                .map(|e| e.view_mode.is_image())
                .unwrap_or(false);

            if in_image_mode {
                return None;
            }
```

**Step 4: Add `Msg::Image` to debug message name formatter**

In `msg_type_name()` (around line 213), add after the `Msg::Csv` arm:
```rust
        Msg::Image(m) => format!("Image::{:?}", m),
```

**Step 5: Verify it compiles**

Run: `cargo check`
Expected: compiles successfully

**Step 6: Commit**

```bash
git add src/update/image.rs src/update/mod.rs
git commit -m "feat(image): add update handler with zoom-toward-cursor and pan logic"
```

---

### Task 6: Add image theme configuration

**Files:**
- Modify: `src/theme.rs`
- Modify: all 9 theme YAML files in `themes/`

**Step 1: Add `ImageThemeData` (raw YAML) to `src/theme.rs`**

After the `CsvThemeData` struct definition (search for `pub struct CsvThemeData`), add:

```rust
/// Image viewer theme data (raw strings from YAML)
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ImageThemeData {
    /// Light checkerboard color (default: #FFFFFF)
    #[serde(default)]
    pub checkerboard_light: Option<String>,
    /// Dark checkerboard color (default: #CCCCCC)
    #[serde(default)]
    pub checkerboard_dark: Option<String>,
    /// Checkerboard cell size in pixels (default: 8)
    #[serde(default)]
    pub checkerboard_cell_size: Option<u32>,
}
```

**Step 2: Add `image` field to `UiThemeData`**

In `UiThemeData` struct, add after the `csv` field:
```rust
    #[serde(default)]
    pub image: ImageThemeData,
```

**Step 3: Add resolved `ImageTheme` struct**

After the `CsvTheme` struct definition (search for `pub struct CsvTheme`), add:

```rust
/// Image viewer theme (resolved colors)
#[derive(Debug, Clone)]
pub struct ImageTheme {
    pub checkerboard_light: Color,
    pub checkerboard_dark: Color,
    pub checkerboard_cell_size: u32,
}

impl Default for ImageTheme {
    fn default() -> Self {
        Self {
            checkerboard_light: Color::rgb(255, 255, 255),
            checkerboard_dark: Color::rgb(204, 204, 204),
            checkerboard_cell_size: 8,
        }
    }
}
```

**Step 4: Add `image` field to `Theme` struct**

In the `Theme` struct, add after the `csv` field:
```rust
    pub image: ImageTheme,
```

**Step 5: Resolve `ImageTheme` in `Theme::from_yaml()`**

Find the `from_yaml()` or `from_data()` method that converts `ThemeData` → `Theme`. Add the image theme resolution:

```rust
            image: ImageTheme {
                checkerboard_light: data.ui.image.checkerboard_light
                    .as_deref()
                    .map(Color::from_hex)
                    .transpose()
                    .unwrap_or(None)
                    .unwrap_or(Color::rgb(255, 255, 255)),
                checkerboard_dark: data.ui.image.checkerboard_dark
                    .as_deref()
                    .map(Color::from_hex)
                    .transpose()
                    .unwrap_or(None)
                    .unwrap_or(Color::rgb(204, 204, 204)),
                checkerboard_cell_size: data.ui.image.checkerboard_cell_size.unwrap_or(8),
            },
```

Also add `image: ImageTheme::default()` to `Theme::default()`.

**Step 6: Add image section to all 9 theme YAML files**

For each file in `themes/`: `dark.yaml`, `fleet-dark.yaml`, `github-dark.yaml`, `github-light.yaml`, `dracula.yaml`, `mocha.yaml`, `nord.yaml`, `tokyo-night.yaml`, `gruvbox-dark.yaml`.

Add at the end of the `ui:` section:

```yaml
  image:
    checkerboard_light: "#FFFFFF"
    checkerboard_dark: "#CCCCCC"
    checkerboard_cell_size: 8
```

For `github-light.yaml` (light theme), use:
```yaml
  image:
    checkerboard_light: "#FFFFFF"
    checkerboard_dark: "#E5E5E5"
    checkerboard_cell_size: 8
```

**Step 7: Verify it compiles and themes load**

Run: `cargo test --lib theme`
Expected: existing theme tests pass

**Step 8: Commit**

```bash
git add src/theme.rs themes/
git commit -m "feat(image): add checkerboard theme config with Photoshop-like defaults"
```

---

### Task 7: Implement image rendering (checkerboard + blitting)

**Files:**
- Modify: `src/image/render.rs`
- Modify: `src/view/mod.rs:923-935`

**Step 1: Implement `src/image/render.rs`**

```rust
//! Image rendering for the view layer
//!
//! Renders checkerboard background and scaled image pixels
//! into the framebuffer.

use crate::image::ImageState;
use crate::theme::ImageTheme;
use crate::view::frame::Frame;

/// Render an image in the given screen rectangle.
///
/// 1. Fills the area with checkerboard pattern
/// 2. Blits visible portion of the image with nearest-neighbor scaling
/// 3. Centers the image if it's smaller than the viewport
pub fn render_image(
    frame: &mut Frame,
    image: &ImageState,
    theme: &ImageTheme,
    area_x: u32,
    area_y: u32,
    area_width: u32,
    area_height: u32,
) {
    let cell = theme.checkerboard_cell_size.max(1);
    let light = theme.checkerboard_light.to_argb_u32();
    let dark = theme.checkerboard_dark.to_argb_u32();

    // Compute how big the image is on screen
    let scaled_width = (image.width as f64 * image.scale) as u32;
    let scaled_height = (image.height as f64 * image.scale) as u32;

    // Center offset when image is smaller than viewport
    let center_x = if scaled_width < area_width {
        (area_width - scaled_width) as f64 / 2.0
    } else {
        0.0
    };
    let center_y = if scaled_height < area_height {
        (area_height - scaled_height) as f64 / 2.0
    } else {
        0.0
    };

    let buf_width = frame.width();

    for sy in 0..area_height {
        let screen_y = area_y + sy;
        if screen_y >= frame.height() {
            break;
        }

        for sx in 0..area_width {
            let screen_x = area_x + sx;
            if screen_x >= frame.width() {
                break;
            }

            // Map screen pixel to image coordinates
            let img_x_f = (sx as f64 - center_x) / image.scale + image.offset_x;
            let img_y_f = (sy as f64 - center_y) / image.scale + image.offset_y;

            // Checkerboard for background
            let checker_col = (sx / cell) % 2;
            let checker_row = (sy / cell) % 2;
            let bg = if (checker_col ^ checker_row) == 0 { light } else { dark };

            let pixel_idx = (screen_y * buf_width + screen_x) as usize;

            // Check if this screen pixel maps to a valid image pixel
            let img_x = img_x_f as i64;
            let img_y = img_y_f as i64;

            if img_x >= 0
                && img_y >= 0
                && (img_x as u32) < image.width
                && (img_y as u32) < image.height
            {
                let ix = img_x as u32;
                let iy = img_y as u32;
                let src_idx = ((iy * image.width + ix) * 4) as usize;

                if src_idx + 3 < image.pixels.len() {
                    let r = image.pixels[src_idx] as u32;
                    let g = image.pixels[src_idx + 1] as u32;
                    let b = image.pixels[src_idx + 2] as u32;
                    let a = image.pixels[src_idx + 3] as u32;

                    if a == 255 {
                        // Fully opaque — write directly
                        frame.set_pixel(pixel_idx, 0xFF000000 | (r << 16) | (g << 8) | b);
                    } else if a == 0 {
                        // Fully transparent — show checkerboard
                        frame.set_pixel(pixel_idx, bg);
                    } else {
                        // Alpha blend over checkerboard
                        let inv_a = 255 - a;
                        let bg_r = (bg >> 16) & 0xFF;
                        let bg_g = (bg >> 8) & 0xFF;
                        let bg_b = bg & 0xFF;
                        let out_r = (r * a + bg_r * inv_a) / 255;
                        let out_g = (g * a + bg_g * inv_a) / 255;
                        let out_b = (b * a + bg_b * inv_a) / 255;
                        frame.set_pixel(
                            pixel_idx,
                            0xFF000000 | (out_r << 16) | (out_g << 8) | out_b,
                        );
                    }
                } else {
                    frame.set_pixel(pixel_idx, bg);
                }
            } else {
                // Outside image bounds — show checkerboard
                frame.set_pixel(pixel_idx, bg);
            }
        }
    }
}
```

**Note:** The `Frame` struct's API for setting pixels needs to be checked. The `frame.set_pixel(idx, color)` call may need to use `frame.buffer_mut()[idx] = color` or similar. Check `src/view/frame.rs` for the actual API and adapt accordingly.

**Step 2: Add render dispatch to `src/view/mod.rs`**

At lines 923-935, change the view mode dispatch:

```rust
        // Check view mode and dispatch to appropriate renderer
        if let Some(image_state) = editor.view_mode.as_image() {
            // Image mode: render image with checkerboard
            let tab_h = model.metrics.tab_bar_height as u32;
            let area_x = group_rect.x as u32;
            let area_y = group_rect.y as u32 + tab_h;
            let area_w = group_rect.width as u32;
            let area_h = (group_rect.height as u32).saturating_sub(tab_h);
            crate::image::render::render_image(
                frame,
                image_state,
                &model.theme.image,
                area_x,
                area_y,
                area_w,
                area_h,
            );
        } else if let Some(csv_state) = editor.view_mode.as_csv() {
            // CSV mode: render grid
            Self::render_csv_grid(frame, painter, model, csv_state, &layout, is_focused);
        } else {
            // Text mode: render normal text area

            // Text area (background highlights, text, cursors)
            Self::render_text_area(frame, painter, model, editor, document, &layout, is_focused);

            // Gutter (line numbers, border) - drawn on top of text area background
            Self::render_gutter(frame, painter, model, editor, document, &layout);
        }
```

**Step 3: Verify it compiles**

Run: `cargo check`
Expected: compiles (may need to adapt Frame API based on actual implementation)

**Step 4: Commit**

```bash
git add src/image/render.rs src/view/mod.rs
git commit -m "feat(image): implement checkerboard + nearest-neighbor image rendering"
```

---

### Task 8: Wire up file opening to load images

**Files:**
- Modify: `src/update/layout.rs:236-244` (OpenFileInNewTab path)
- Modify: `src/model/mod.rs:140-214` (initial session path)

This is the critical integration: when the user opens a file with an image extension, decode it and set `ViewMode::Image` instead of rejecting it as binary.

**Step 1: Add image loading helper function to `src/image/mod.rs`**

Add at the bottom of `src/image/mod.rs`:

```rust
/// Load and decode an image file into an ImageState.
///
/// Returns None if the file can't be read or decoded.
pub fn load_image(path: &std::path::Path, viewport_width: u32, viewport_height: u32) -> Option<ImageState> {
    let file_size = std::fs::metadata(path).ok()?.len();

    let format = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| match e.to_lowercase().as_str() {
            "jpg" | "jpeg" => "JPEG".to_string(),
            "png" => "PNG".to_string(),
            "gif" => "GIF".to_string(),
            "bmp" => "BMP".to_string(),
            "webp" => "WebP".to_string(),
            other => other.to_uppercase(),
        })
        .unwrap_or_else(|| "Unknown".to_string());

    let img = image::open(path).ok()?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let pixels = rgba.into_raw();

    Some(ImageState::new(pixels, width, height, file_size, format, viewport_width, viewport_height))
}
```

**Step 2: Modify `src/update/layout.rs` — OpenFileInNewTab**

At line 239, before the `is_likely_binary` check, add image file handling:

```rust
            // Image files: load as image viewer instead of text
            if is_image_file(&path) {
                let group_id = model.editor_area.focused_group_id;
                let group = model.editor_area.groups.get(&group_id);
                let (vw, vh) = group
                    .map(|g| (g.rect.width as u32, g.rect.height as u32))
                    .unwrap_or((800, 600));

                if let Some(image_state) = crate::image::load_image(&path, vw, vh) {
                    let doc_id = model.editor_area.next_document_id();
                    let mut doc = Document::new_with_path(path.clone());
                    doc.id = Some(doc_id);

                    let mut editor = EditorState::with_viewport(
                        model.viewport_geometry.visible_lines,
                        model.viewport_geometry.visible_columns,
                    );
                    editor.view_mode = ViewMode::Image(Box::new(image_state));

                    // Add as tab (reuse existing tab-creation logic)
                    // ... wire into editor_area
                    model.ui.set_status(format!("Opened image: {}", filename));
                    return Some(Cmd::Redraw);
                } else {
                    model.ui.set_status(format!("Failed to decode image: {}", filename));
                    return Some(Cmd::Redraw);
                }
            }
```

**Important:** The exact tab-creation code depends on how `editor_area.add_tab()` / `editor_area.open_document_in_group()` works. Study the existing code in this function (lines 232-275) for the exact pattern of adding a document + editor to a group, and replicate it. The key difference is setting `editor.view_mode = ViewMode::Image(...)` after creating the editor.

Also add the import at the top of the file:
```rust
use crate::util::is_image_file;
use crate::model::editor::ViewMode;
```

**Step 3: Handle initial session (CLI argument)**

In `src/model/mod.rs`, at lines 145 and 193 where `is_likely_binary()` rejects files, add image checks before:

```rust
                if is_image_file(first_path) {
                    // Load image for initial session
                    // ... similar to above
                } else if is_likely_binary(first_path) {
```

**Step 4: Verify it compiles**

Run: `cargo check`
Expected: compiles

**Step 5: Manual test**

Run: `cargo run -- path/to/test-image.png`
Expected: image displays with checkerboard background

**Step 6: Commit**

```bash
git add src/image/mod.rs src/update/layout.rs src/model/mod.rs
git commit -m "feat(image): load image files into ViewMode::Image on file open"
```

---

### Task 9: Wire up mouse events for pan and scroll-wheel zoom

**Files:**
- Modify: `src/runtime/app.rs:857-894` (scroll wheel routing)
- Modify: `src/runtime/mouse.rs:322-346` (click handling)
- Modify: `src/runtime/app.rs:620-690` (cursor moved / drag)
- Modify: `src/runtime/app.rs:726-740` (mouse release)
- Modify: `src/view/hit_test.rs:556-566` (hit target for image mode)

**Step 1: Add `HitTarget::ImageContent` variant**

In `src/view/hit_test.rs`, after `CsvCell` (line 186), add:

```rust
    /// Image content area (for pan/zoom)
    ImageContent {
        group_id: GroupId,
        editor_id: EditorId,
    },
```

Add it to `group_id()` and `suggested_focus()` match arms alongside `CsvCell`.

**Step 2: Return `ImageContent` from hit test**

In `hit_test_groups()` (around line 556), before the CSV check:

```rust
    // Check if in image mode
    if editor.view_mode.is_image() {
        return Some(HitTarget::ImageContent {
            group_id,
            editor_id,
        });
    }
```

**Step 3: Handle scroll wheel for image zoom**

In `src/runtime/app.rs`, in the `HoverRegion::EditorText` arm (line 857), add image mode check before CSV check:

```rust
                    HoverRegion::EditorText => {
                        // Check if focused editor is in image mode
                        let in_image_mode = self
                            .model
                            .editor_area
                            .focused_editor()
                            .map(|e| e.view_mode.is_image())
                            .unwrap_or(false);

                        if in_image_mode {
                            // Scroll wheel = zoom, centered on mouse position
                            let (mx, my) = self.mouse_position.unwrap_or((0.0, 0.0));
                            // Use vertical delta for zoom
                            if v_delta != 0 {
                                return update(
                                    &mut self.model,
                                    Msg::Image(ImageMsg::Zoom {
                                        delta: v_delta as f64,
                                        mouse_x: mx,
                                        mouse_y: my,
                                    }),
                                );
                            }
                            return None;
                        }

                        // Check if focused editor is in CSV mode
                        // ... (existing CSV code)
```

Add import: `use token::messages::ImageMsg;`

**Step 4: Handle left-click for pan start**

In `src/runtime/mouse.rs`, in the `handle_left_click` function, add a handler for `HitTarget::ImageContent`:

```rust
        HitTarget::ImageContent { group_id, .. } => {
            if *group_id != model.editor_area.focused_group_id {
                update(model, Msg::Layout(LayoutMsg::FocusGroup(*group_id)));
            }
            let (x, y) = (event.pos.x, event.pos.y);
            update(
                model,
                Msg::Image(token::messages::ImageMsg::StartPan { x, y }),
            );
            EventResult::consumed_with_focus_and_drag(FocusTarget::Editor)
        }
```

Note: Check if `EventResult` has a method/field for requesting drag tracking. If not, use `EventResult::consumed_with_focus(FocusTarget::Editor)` and set `start_drag_tracking` to true.

**Step 5: Handle cursor moved for pan update**

In `src/runtime/app.rs`, in the `CursorMoved` handler (line 620), after drag checks, add:

```rust
                // Handle image panning drag
                if self.left_mouse_down {
                    let in_image_mode = self
                        .model
                        .editor_area
                        .focused_editor()
                        .map(|e| e.view_mode.as_image().map(|s| s.drag.is_some()).unwrap_or(false))
                        .unwrap_or(false);

                    if in_image_mode {
                        return update(
                            &mut self.model,
                            Msg::Image(ImageMsg::UpdatePan {
                                x: position.x,
                                y: position.y,
                            }),
                        );
                    }
                }
```

**Step 6: Handle mouse release for pan end**

In the `MouseButton::Left Released` handler (around line 730), add:

```rust
                // End image pan if active
                let in_image_panning = self
                    .model
                    .editor_area
                    .focused_editor()
                    .and_then(|e| e.view_mode.as_image())
                    .map(|s| s.drag.is_some())
                    .unwrap_or(false);

                if in_image_panning {
                    return update(&mut self.model, Msg::Image(ImageMsg::EndPan));
                }
```

**Step 7: Verify it compiles**

Run: `cargo check`
Expected: compiles

**Step 8: Commit**

```bash
git add src/runtime/app.rs src/runtime/mouse.rs src/view/hit_test.rs
git commit -m "feat(image): wire mouse events for pan (click-drag) and zoom (scroll wheel)"
```

---

### Task 10: Add status bar info for image mode

**Files:**
- Modify: `src/model/status_bar.rs:9-24` (add new SegmentIds)
- Modify: `src/model/status_bar.rs:361-420` (sync_status_bar)

**Step 1: Add new SegmentId variants**

In `SegmentId` enum, add:

```rust
    /// Image dimensions (e.g. "1920×1080")
    ImageDimensions,
    /// Image zoom level (e.g. "100%")
    ImageZoom,
    /// Image file size (e.g. "2.4 MB")
    ImageFileSize,
    /// Image format (e.g. "PNG")
    ImageFormat,
```

**Step 2: Update `sync_status_bar()`**

Add image-specific segments. After the existing segments, add:

```rust
    // Image mode segments
    let image_info = model
        .editor_area
        .focused_editor()
        .and_then(|e| e.view_mode.as_image());

    if let Some(img) = image_info {
        // Show image-specific info, hide text-specific info
        model.ui.status_bar.update_segment(
            SegmentId::CursorPosition,
            SegmentContent::Text(format!("{}×{}", img.width, img.height)),
        );
        model.ui.status_bar.update_segment(
            SegmentId::LineCount,
            SegmentContent::Text(format!("{}%", img.zoom_percent())),
        );
        model.ui.status_bar.update_segment(
            SegmentId::Selection,
            SegmentContent::Text(img.file_size_display()),
        );
        model.ui.status_bar.update_segment(
            SegmentId::CaretCount,
            SegmentContent::Text(img.format.clone()),
        );
    }
```

Note: This reuses existing segment slots rather than adding new ones, which avoids changing the status bar layout. The CursorPosition slot shows dimensions, LineCount shows zoom %, Selection shows file size, and CaretCount shows format. This is simpler and works with the existing rendering.

**Step 3: Verify it compiles**

Run: `cargo check`
Expected: compiles

**Step 4: Commit**

```bash
git add src/model/status_bar.rs
git commit -m "feat(image): show dimensions, zoom %, file size, format in status bar"
```

---

### Task 11: Add keyboard shortcuts for zoom/fit/actual-size

**Files:**
- Modify: `src/keymap/command.rs` (add commands)
- Modify: `keymap.yaml` (add bindings)

**Step 1: Add image commands to `Command` enum**

After the CSV commands (around line 462), add:

```rust
    // Image viewer
    /// Zoom in (image mode)
    ImageZoomIn,
    /// Zoom out (image mode)
    ImageZoomOut,
    /// Fit image to window
    ImageFitToWindow,
    /// Show image at actual size (1:1)
    ImageActualSize,
```

**Step 2: Add message mappings in `to_msgs()`**

After the CSV mappings (line 462), add:

```rust
            // Image viewer
            ImageZoomIn => vec![Msg::Image(ImageMsg::Zoom { delta: 1.0, mouse_x: 0.0, mouse_y: 0.0 })],
            ImageZoomOut => vec![Msg::Image(ImageMsg::Zoom { delta: -1.0, mouse_x: 0.0, mouse_y: 0.0 })],
            ImageFitToWindow => vec![Msg::Image(ImageMsg::FitToWindow)],
            ImageActualSize => vec![Msg::Image(ImageMsg::ActualSize)],
```

Add import: `use crate::messages::ImageMsg;`

**Step 3: Add `display_name()` entries**

In the `display_name()` method, add:

```rust
            ImageZoomIn => "Image: Zoom In",
            ImageZoomOut => "Image: Zoom Out",
            ImageFitToWindow => "Image: Fit to Window",
            ImageActualSize => "Image: Actual Size",
```

**Step 4: Add keybindings to `keymap.yaml`**

Add image viewer section:

```yaml
# Image viewer
- key: "="
  modifiers: [logo]
  command: ImageZoomIn
- key: "-"
  modifiers: [logo]
  command: ImageZoomOut
- key: "0"
  modifiers: [logo]
  command: ImageFitToWindow
- key: "1"
  modifiers: [logo]
  command: ImageActualSize
```

Note: Check for conflicts with existing Cmd+0 and Cmd+1 bindings. If they conflict (e.g., Cmd+1 switches to tab 1), these bindings will need to be context-dependent — only active in image mode. The keymap system may need a guard, or the commands can be mapped through the update dispatcher to check if the editor is in image mode.

**Step 5: Verify it compiles**

Run: `cargo check`
Expected: compiles

**Step 6: Commit**

```bash
git add src/keymap/command.rs keymap.yaml
git commit -m "feat(image): add keyboard shortcuts for zoom in/out, fit, actual size"
```

---

### Task 12: Integration testing and polish

**Files:**
- Create: `tests/image_viewer.rs`

**Step 1: Write integration tests**

```rust
use std::path::Path;
use token::image::ImageState;
use token::util::is_image_file;

#[test]
fn test_image_file_detection() {
    assert!(is_image_file(Path::new("test.png")));
    assert!(is_image_file(Path::new("test.jpg")));
    assert!(is_image_file(Path::new("test.jpeg")));
    assert!(is_image_file(Path::new("test.gif")));
    assert!(is_image_file(Path::new("test.bmp")));
    assert!(is_image_file(Path::new("test.webp")));
    assert!(!is_image_file(Path::new("test.rs")));
    assert!(!is_image_file(Path::new("test.txt")));
}

#[test]
fn test_compute_fit_scale_large_image() {
    // Image larger than viewport should be scaled down
    let scale = ImageState::compute_fit_scale(1920, 1080, 800, 600);
    assert!(scale < 1.0);
    assert!((scale - 800.0 / 1920.0).abs() < 0.01);
}

#[test]
fn test_compute_fit_scale_small_image() {
    // Image smaller than viewport should stay at 1.0
    let scale = ImageState::compute_fit_scale(100, 100, 800, 600);
    assert!((scale - 1.0).abs() < f64::EPSILON);
}

#[test]
fn test_compute_fit_scale_zero_viewport() {
    let scale = ImageState::compute_fit_scale(100, 100, 0, 0);
    assert!((scale - 1.0).abs() < f64::EPSILON);
}

#[test]
fn test_file_size_display() {
    let state = ImageState::new(vec![0; 400], 10, 10, 2_500_000, "PNG".into(), 800, 600);
    assert_eq!(state.file_size_display(), "2.4 MB");

    let state = ImageState::new(vec![0; 400], 10, 10, 150_000, "PNG".into(), 800, 600);
    assert_eq!(state.file_size_display(), "146 KB");

    let state = ImageState::new(vec![0; 400], 10, 10, 500, "PNG".into(), 800, 600);
    assert_eq!(state.file_size_display(), "500 B");
}

#[test]
fn test_zoom_percent() {
    let mut state = ImageState::new(vec![0; 400], 10, 10, 100, "PNG".into(), 800, 600);
    state.scale = 1.0;
    assert_eq!(state.zoom_percent(), 100);
    state.scale = 0.5;
    assert_eq!(state.zoom_percent(), 50);
    state.scale = 2.0;
    assert_eq!(state.zoom_percent(), 200);
}
```

**Step 2: Run tests**

Run: `cargo test image_viewer`
Expected: all tests pass

**Step 3: Run full test suite**

Run: `make test`
Expected: all existing tests still pass, new tests pass

**Step 4: Run linter**

Run: `make lint`
Expected: no new warnings

**Step 5: Commit**

```bash
git add tests/image_viewer.rs
git commit -m "test(image): add integration tests for image viewer"
```

---

### Task 13: Manual testing checklist

This is a manual verification task — no code changes.

**Test with real images:**

1. Open a PNG file: `cargo run -- path/to/image.png`
   - Image should display with checkerboard behind transparent areas
   - Auto-fit if image is larger than window
2. Scroll wheel: zoom in and out, verify it zooms toward cursor position
3. Click and drag: pan the image around
4. Cmd+= / Cmd+-: keyboard zoom
5. Cmd+0: fit to window
6. Cmd+1: actual size
7. Status bar: verify it shows dimensions, zoom %, file size, format
8. Open multiple images as tabs (via sidebar or Cmd+O)
9. Resize window: verify auto-fit recalculates if user hasn't manually zoomed
10. Test formats: PNG, JPEG, GIF, BMP, WebP

**Step 1: Run with sample images**

Run: `cargo run --release -- path/to/test.png`

**Step 2: Fix any issues found**

Address bugs discovered during manual testing.

**Step 3: Final commit**

```bash
git add -A
git commit -m "fix(image): polish image viewer based on manual testing"
```
