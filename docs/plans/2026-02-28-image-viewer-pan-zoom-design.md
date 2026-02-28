# Image Viewer with Pan & Zoom

**Date:** 2026-02-28
**Status:** Approved

## Overview

Add an image viewer to the editor using the `ViewMode::Image` pattern (following the CSV precedent). Images open as tabs with pan, zoom, checkerboard transparency, and status bar info.

## Requirements

- **Zoom:** Scroll wheel centered on cursor + pinch-to-zoom on trackpad
- **Pan:** Click and drag anywhere on the image
- **Initial scale:** Auto — fit if image is larger than viewport, actual size if smaller
- **Formats:** PNG, JPEG, GIF (static), BMP, WebP
- **Background:** Checkerboard pattern for transparency, configurable via theme with Photoshop-like fallback
- **Status bar:** Dimensions, zoom %, file size, format
- **Keyboard shortcuts:** Cmd+=/- zoom, Cmd+0 fit, Cmd+1 actual size

## Data Model

### ImageState

```rust
pub struct ImageState {
    pub pixels: Vec<u8>,       // Decoded RGBA pixel data
    pub width: u32,
    pub height: u32,
    pub file_size: u64,        // For status bar display
    pub format: String,        // "PNG", "JPEG", etc.
    pub scale: f64,            // 1.0 = 100%
    pub offset_x: f64,        // Pan offset in image-space pixels
    pub offset_y: f64,
    pub user_zoomed: bool,     // False = auto-fit mode
    pub last_mouse_x: f64,
    pub last_mouse_y: f64,
    pub drag: Option<DragState>,
}

pub struct DragState {
    pub start_mouse_x: f64,
    pub start_mouse_y: f64,
    pub start_offset_x: f64,
    pub start_offset_y: f64,
}
```

Added as `ViewMode::Image(Box<ImageState>)`.

## Messages

```rust
pub enum ImageMsg {
    Zoom { delta: f64, mouse_x: f64, mouse_y: f64 },
    StartPan { x: f64, y: f64 },
    UpdatePan { x: f64, y: f64 },
    EndPan,
    FitToWindow,
    ActualSize,
    MouseMove { x: f64, y: f64 },
    ViewportResized { width: u32, height: u32 },
}
```

Added to `Msg` as `Msg::Image(ImageMsg)`.

## Zoom Behavior

Zoom-toward-cursor: the point under the cursor stays fixed on screen.

```
new_scale = old_scale * (1.0 + delta * 0.1)  // clamped to [0.1, 10.0]
offset_x = mouse_x_in_image - (mouse_x_on_screen / new_scale)
offset_y = mouse_y_in_image - (mouse_y_on_screen / new_scale)
```

Scroll wheel and trackpad pinch both come through winit as `MouseWheel` events.

## Pan Behavior

Click-and-drag anywhere on the image:
- MouseDown → `StartPan` (record start position + current offset)
- MouseMove → `UpdatePan` (offset = start_offset + (current - start) / scale)
- MouseUp → `EndPan`

Offset clamped so at least 10% of the image stays visible.

## Rendering

1. **Checkerboard background** — two colors from theme, configurable cell size, fallback to `#CCCCCC`/`#FFFFFF` 8px
2. **Image pixels** — for each screen pixel, map to image coords via scale + offset, nearest-neighbor sample, alpha-blend over checkerboard
3. **Centering** — when scaled image < viewport, center it

## File Opening Changes

- Add `is_image_file(path)` to `file_validation.rs` checking extensions
- Check `is_image_file()` before `is_likely_binary()` in the file-open path
- Decode with `image` crate → create `ViewMode::Image`
- Update `Cargo.toml` image features: `["png", "jpeg", "gif", "bmp", "webp"]`

## Theme — Checkerboard Config

New `image` section in theme YAML:

```yaml
ui:
  image:
    checkerboard_light: "#FFFFFF"
    checkerboard_dark: "#CCCCCC"
    checkerboard_cell_size: 8
```

All optional with Photoshop-like defaults. Add to all 9 theme files.

## Status Bar

In image mode, `sync_status_bar()` shows:
- Dimensions: `1920×1080`
- Zoom: `100%`
- File size: `2.4 MB`
- Format: `PNG`

## Keyboard Shortcuts

| Action         | Key    |
|----------------|--------|
| Zoom in        | Cmd+=  |
| Zoom out       | Cmd+-  |
| Fit to window  | Cmd+0  |
| Actual size    | Cmd+1  |

## Files to Create/Modify

### New files
- `src/image/mod.rs` — ImageState, ImageMsg, DragState
- `src/image/render.rs` — image rendering + checkerboard
- `src/update/image.rs` — message handlers

### Modified files
- `Cargo.toml` — image crate features
- `src/lib.rs` — add `pub mod image;`
- `src/model/editor.rs` — ViewMode::Image variant
- `src/messages.rs` — ImageMsg enum + Msg::Image variant
- `src/util/file_validation.rs` — is_image_file()
- `src/runtime/app.rs` — route mouse/scroll events for image mode
- `src/view/mod.rs` — call image renderer
- `src/theme.rs` — ImageThemeData, ImageTheme structs
- `src/model/status_bar.rs` — image mode status segments
- `themes/*.yaml` (9 files) — add image section
- `src/keymap/` — image-specific keybindings
