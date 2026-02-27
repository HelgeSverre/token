# Image Viewer & Binary Placeholder Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** When opening files from the sidebar, display images inline in a tab and show a "Open with Default Application" placeholder for unsupported binary files, instead of silently rejecting them.

**Architecture:** Extend `TabContent` on `EditorState` to support `Image` and `BinaryPlaceholder` variants alongside existing `Text` mode. The open pipeline in `open_file_in_new_tab()` checks image extensions before binary detection, dispatching to the appropriate tab content type. Rendering dispatches on `TabContent` before `ViewMode`. A minimal `Document` (empty buffer) is still created for non-text tabs to reuse tab naming, "already open" checks, and file path tracking.

**Tech Stack:** `image` crate (already a dep, needs `jpeg`/`gif`/`bmp`/`webp`/`ico` features enabled), `Frame` for CPU blitting, existing `TextPainter` for placeholder text, `Cmd::OpenInExplorer` for the "open externally" action.

---

## Task 1: Add image format features to `image` crate dependency

**Files:**
- Modify: `Cargo.toml`

**Step 1: Update the image dependency to include more format decoders**

In `Cargo.toml`, change:
```toml
image = { version = "0.25", default-features = false, features = ["png"] }
```
to:
```toml
image = { version = "0.25", default-features = false, features = ["png", "jpeg", "gif", "bmp", "webp", "ico"] }
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: Compiles successfully

**Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "feat: enable additional image format decoders"
```

---

## Task 2: Add `TabContent` enum and state structs to model

**Files:**
- Modify: `src/model/editor.rs` (add `TabContent`, `ImageTabState`, `BinaryPlaceholderState`)

**Step 1: Add the new types**

Add these types near the `ViewMode` enum (around line 270):

```rust
/// What kind of content this tab displays
#[derive(Debug, Clone, Default)]
pub enum TabContent {
    /// Normal text/code editing (uses Document rope + ViewMode)
    #[default]
    Text,
    /// Image viewer (decoded RGBA pixels)
    Image(ImageTabState),
    /// Placeholder for unsupported binary files
    BinaryPlaceholder(BinaryPlaceholderState),
}

/// State for an image viewer tab
#[derive(Debug, Clone)]
pub struct ImageTabState {
    /// Path to the image file
    pub path: std::path::PathBuf,
    /// Decoded RGBA8 pixel data
    pub pixels: Vec<u8>,
    /// Image width in pixels
    pub width: u32,
    /// Image height in pixels
    pub height: u32,
}

/// State for a binary file placeholder tab
#[derive(Debug, Clone)]
pub struct BinaryPlaceholderState {
    /// Path to the binary file
    pub path: std::path::PathBuf,
    /// File size in bytes
    pub size_bytes: u64,
}
```

**Step 2: Add `tab_content` field to `EditorState`**

Add to the `EditorState` struct (after `view_mode` field, line ~336):
```rust
/// What kind of content this tab displays (text, image, binary placeholder)
pub tab_content: TabContent,
```

Update both `EditorState::new()` and `EditorState::with_viewport()` to initialize:
```rust
tab_content: TabContent::default(),
```

**Step 3: Export the new types from `src/model/mod.rs`**

Add `TabContent`, `ImageTabState`, `BinaryPlaceholderState` to the re-exports from `editor` module.

**Step 4: Verify it compiles**

Run: `cargo check`
Expected: Compiles successfully (no references to new types yet)

**Step 5: Commit**

```bash
git add src/model/editor.rs src/model/mod.rs
git commit -m "feat: add TabContent enum for image and binary placeholder tabs"
```

---

## Task 3: Add `is_supported_image` helper to file validation

**Files:**
- Modify: `src/util/file_validation.rs`
- Modify: `src/util/mod.rs` (re-export)

**Step 1: Add the helper function**

Add to `src/util/file_validation.rs`:

```rust
/// Check if a file path has a supported image extension
pub fn is_supported_image(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase());
    matches!(
        ext.as_deref(),
        Some("png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "ico")
    )
}
```

**Step 2: Add a test**

```rust
#[test]
fn test_is_supported_image() {
    assert!(is_supported_image(Path::new("photo.png")));
    assert!(is_supported_image(Path::new("photo.JPG")));
    assert!(is_supported_image(Path::new("photo.jpeg")));
    assert!(is_supported_image(Path::new("animation.gif")));
    assert!(is_supported_image(Path::new("icon.ico")));
    assert!(is_supported_image(Path::new("photo.webp")));
    assert!(is_supported_image(Path::new("photo.bmp")));
    assert!(!is_supported_image(Path::new("code.rs")));
    assert!(!is_supported_image(Path::new("doc.pdf")));
    assert!(!is_supported_image(Path::new("noext")));
}
```

**Step 3: Re-export from `src/util/mod.rs`**

Add `is_supported_image` to the re-export line.

**Step 4: Run the test**

Run: `cargo test test_is_supported_image`
Expected: PASS

**Step 5: Commit**

```bash
git add src/util/file_validation.rs src/util/mod.rs
git commit -m "feat: add is_supported_image helper"
```

---

## Task 4: Update `open_file_in_new_tab()` to handle images and binary files

**Files:**
- Modify: `src/update/layout.rs`

This is the core change. The current flow rejects binary files with a status message. The new flow:
1. Validate file (exists, permissions, size) — unchanged
2. Check if it's a supported image → open as `TabContent::Image`
3. Check if it's binary → open as `TabContent::BinaryPlaceholder`
4. Otherwise → open as text (existing behavior)

**Step 1: Add imports**

Add to imports at top of `src/update/layout.rs`:
```rust
use crate::model::editor::{ImageTabState, BinaryPlaceholderState, TabContent};
use crate::util::is_supported_image;
```

**Step 2: Replace the binary file handling in `open_file_in_new_tab()`**

Replace the section after `Ok(())` in `validate_file_for_opening` match (lines ~237-258) with:

```rust
Ok(()) => {
    if is_supported_image(&path) {
        // Image file - decode and open as image tab
        match image::open(&path) {
            Ok(img) => {
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                let pixels = rgba.into_raw();

                let mut doc = Document::new();
                doc.id = Some(doc_id);
                doc.file_path = Some(path.clone());
                model.editor_area.documents.insert(doc_id, doc);
                model.record_file_opened(path.clone());

                let editor_id = model.editor_area.next_editor_id();
                let mut editor = EditorState::new();
                editor.id = Some(editor_id);
                editor.document_id = Some(doc_id);
                editor.tab_content = TabContent::Image(ImageTabState {
                    path: path.clone(),
                    pixels,
                    width: w,
                    height: h,
                });
                model.editor_area.editors.insert(editor_id, editor);

                let tab_id = model.editor_area.next_tab_id();
                let tab = Tab {
                    id: tab_id,
                    editor_id,
                    is_pinned: false,
                    is_preview: false,
                };
                if let Some(group) = model.editor_area.groups.get_mut(&group_id) {
                    group.tabs.push(tab);
                    group.active_tab_index = group.tabs.len() - 1;
                }
                model.ui.set_status(format!("Opened image: {}", filename));
                return Some(Cmd::Redraw);
            }
            Err(e) => {
                model.ui.set_status(format!("Error opening image {}: {}", filename, e));
                return Some(Cmd::Redraw);
            }
        }
    }

    if is_likely_binary(&path) {
        // Binary file - open as placeholder tab
        let size_bytes = std::fs::metadata(&path)
            .map(|m| m.len())
            .unwrap_or(0);

        let mut doc = Document::new();
        doc.id = Some(doc_id);
        doc.file_path = Some(path.clone());
        model.editor_area.documents.insert(doc_id, doc);
        model.record_file_opened(path.clone());

        let editor_id = model.editor_area.next_editor_id();
        let mut editor = EditorState::new();
        editor.id = Some(editor_id);
        editor.document_id = Some(doc_id);
        editor.tab_content = TabContent::BinaryPlaceholder(BinaryPlaceholderState {
            path: path.clone(),
            size_bytes,
        });
        model.editor_area.editors.insert(editor_id, editor);

        let tab_id = model.editor_area.next_tab_id();
        let tab = Tab {
            id: tab_id,
            editor_id,
            is_pinned: false,
            is_preview: false,
        };
        if let Some(group) = model.editor_area.groups.get_mut(&group_id) {
            group.tabs.push(tab);
            group.active_tab_index = group.tabs.len() - 1;
        }
        model.ui.set_status(format!("Binary file: {}", filename));
        return Some(Cmd::Redraw);
    }

    // Text file - load normally (existing code)
    match Document::from_file(path.clone()) {
        Ok(mut doc) => {
            doc.id = Some(doc_id);
            model.ui.set_status(format!("Opened: {}", path.display()));
            doc
        }
        Err(e) => {
            model.ui.set_status(format!("Error opening {}: {}", path.display(), e));
            return Some(Cmd::Redraw);
        }
    }
}
```

Note: The rest of the function (inserting document, creating editor/tab for text, scheduling syntax parse) remains unchanged and handles the text case.

**Step 3: Verify it compiles**

Run: `cargo check`
Expected: Compiles (rendering won't use the new types yet)

**Step 4: Commit**

```bash
git add src/update/layout.rs
git commit -m "feat: open images and binary files as specialized tabs"
```

---

## Task 5: Add `blit_rgba_scaled` method to `Frame`

**Files:**
- Modify: `src/view/frame.rs`

**Step 1: Add the image blitting method**

Add to `impl Frame`:

```rust
/// Blit an RGBA8 image into the frame, scaled to fit within the given rect
/// while preserving aspect ratio. Centers the image within the rect.
/// Uses nearest-neighbor scaling for simplicity.
pub fn blit_rgba_scaled(
    &mut self,
    pixels: &[u8],
    img_width: u32,
    img_height: u32,
    dest_x: usize,
    dest_y: usize,
    dest_w: usize,
    dest_h: usize,
) {
    if img_width == 0 || img_height == 0 || dest_w == 0 || dest_h == 0 {
        return;
    }

    // Calculate scale to fit while preserving aspect ratio
    let scale_x = dest_w as f64 / img_width as f64;
    let scale_y = dest_h as f64 / img_height as f64;
    let scale = scale_x.min(scale_y);

    let scaled_w = (img_width as f64 * scale) as usize;
    let scaled_h = (img_height as f64 * scale) as usize;

    // Center within destination rect
    let offset_x = dest_x + (dest_w.saturating_sub(scaled_w)) / 2;
    let offset_y = dest_y + (dest_h.saturating_sub(scaled_h)) / 2;

    for dy in 0..scaled_h {
        let py = offset_y + dy;
        if py >= self.height {
            break;
        }
        let src_y = ((dy as f64 / scale) as u32).min(img_height - 1);
        let row_start = py * self.width;

        for dx in 0..scaled_w {
            let px = offset_x + dx;
            if px >= self.width {
                break;
            }
            let src_x = ((dx as f64 / scale) as u32).min(img_width - 1);
            let src_idx = ((src_y * img_width + src_x) * 4) as usize;

            if src_idx + 3 >= pixels.len() {
                continue;
            }

            let r = pixels[src_idx] as u32;
            let g = pixels[src_idx + 1] as u32;
            let b = pixels[src_idx + 2] as u32;
            let a = pixels[src_idx + 3] as f32 / 255.0;

            let argb = 0xFF000000 | (r << 16) | (g << 8) | b;

            if a >= 1.0 {
                self.buffer[row_start + px] = argb;
            } else if a > 0.0 {
                self.buffer[row_start + px] =
                    blend_colors(self.buffer[row_start + px], argb, a);
            }
        }
    }
}
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: Compiles successfully

**Step 3: Commit**

```bash
git add src/view/frame.rs
git commit -m "feat: add blit_rgba_scaled to Frame for image rendering"
```

---

## Task 6: Add image tab and binary placeholder renderers

**Files:**
- Modify: `src/view/mod.rs`

**Step 1: Update `render_editor_group()` to dispatch on `TabContent`**

In `render_editor_group()` (around line 923), replace the existing view mode dispatch:

```rust
// Check view mode and dispatch to appropriate renderer
if let Some(csv_state) = editor.view_mode.as_csv() {
    // CSV mode: render grid
    Self::render_csv_grid(frame, painter, model, csv_state, &layout, is_focused);
} else {
    // Text mode: render normal text area
    Self::render_text_area(frame, painter, model, editor, document, &layout, is_focused);
    Self::render_gutter(frame, painter, model, editor, document, &layout);
}
```

With:

```rust
// Dispatch based on tab content type
match &editor.tab_content {
    crate::model::editor::TabContent::Image(img_state) => {
        Self::render_image_tab(frame, painter, model, img_state, &layout);
    }
    crate::model::editor::TabContent::BinaryPlaceholder(placeholder) => {
        Self::render_binary_placeholder(frame, painter, model, placeholder, &layout);
    }
    crate::model::editor::TabContent::Text => {
        if let Some(csv_state) = editor.view_mode.as_csv() {
            Self::render_csv_grid(frame, painter, model, csv_state, &layout, is_focused);
        } else {
            Self::render_text_area(frame, painter, model, editor, document, &layout, is_focused);
            Self::render_gutter(frame, painter, model, editor, document, &layout);
        }
    }
}
```

**Step 2: Add `render_image_tab()` method**

Add a new method to the `Renderer` impl (after `render_editor_group`):

```rust
/// Render an image viewer tab
fn render_image_tab(
    frame: &mut Frame,
    _painter: &mut TextPainter,
    model: &AppModel,
    img_state: &crate::model::editor::ImageTabState,
    layout: &geometry::GroupLayout,
) {
    let content_rect = layout.content_rect();
    let bg = model.theme.editor.background.to_argb_u32();
    frame.fill_rect(content_rect, bg);

    // Add padding around the image
    let padding = model.metrics.padding_large * 2;
    let dest_x = content_rect.x as usize + padding;
    let dest_y = content_rect.y as usize + padding;
    let dest_w = (content_rect.width as usize).saturating_sub(padding * 2);
    let dest_h = (content_rect.height as usize).saturating_sub(padding * 2);

    if dest_w > 0 && dest_h > 0 {
        // Draw checkerboard pattern for transparency
        let check_size = 8;
        let light = 0xFF_CCCCCC_u32;
        let dark = 0xFF_999999_u32;
        for cy in 0..dest_h {
            for cx in 0..dest_w {
                let px = dest_x + cx;
                let py = dest_y + cy;
                if px < frame.width() && py < frame.height() {
                    let checker = ((cx / check_size) + (cy / check_size)) % 2 == 0;
                    frame.set_pixel(px, py, if checker { light } else { dark });
                }
            }
        }

        frame.blit_rgba_scaled(
            &img_state.pixels,
            img_state.width,
            img_state.height,
            dest_x,
            dest_y,
            dest_w,
            dest_h,
        );
    }
}
```

**Step 3: Add `render_binary_placeholder()` method**

```rust
/// Render a binary file placeholder tab with "Open with Default Application" action
fn render_binary_placeholder(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    placeholder: &crate::model::editor::BinaryPlaceholderState,
    layout: &geometry::GroupLayout,
) {
    let content_rect = layout.content_rect();
    let bg = model.theme.editor.background.to_argb_u32();
    let fg = model.theme.editor.foreground.to_argb_u32();
    let dim_fg = model.theme.editor.line_number.to_argb_u32();
    frame.fill_rect(content_rect, bg);

    let char_width = painter.char_width();
    let line_height = painter.line_height();
    let center_x = content_rect.x as usize + content_rect.width as usize / 2;
    let center_y = content_rect.y as usize + content_rect.height as usize / 2;

    // File icon (large)
    let icon = "󰈔"; // nf-md-file
    let icon_x = center_x.saturating_sub(char_width as usize / 2);
    let icon_y = center_y.saturating_sub((line_height * 4.0) as usize);
    painter.draw_text(frame, icon_x, icon_y, icon, fg);

    // Filename
    let filename = placeholder
        .path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let name_x = center_x.saturating_sub((filename.len() as f32 * char_width / 2.0) as usize);
    let name_y = icon_y + (line_height * 2.0) as usize;
    painter.draw_text(frame, name_x, name_y, &filename, fg);

    // File size
    let size_str = format_file_size(placeholder.size_bytes);
    let size_x = center_x.saturating_sub((size_str.len() as f32 * char_width / 2.0) as usize);
    let size_y = name_y + (line_height * 1.5) as usize;
    painter.draw_text(frame, size_x, size_y, &size_str, dim_fg);

    // "Open with Default Application" hint
    let hint = "Press Enter or double-click to open with default application";
    let hint_x = center_x.saturating_sub((hint.len() as f32 * char_width / 2.0) as usize);
    let hint_y = size_y + (line_height * 3.0) as usize;
    painter.draw_text(frame, hint_x, hint_y, hint, dim_fg);
}
```

**Step 4: Add the `format_file_size` helper** (at the bottom of the file or as a free function near the renderers):

```rust
fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
```

**Step 5: Verify it compiles**

Run: `cargo check`
Expected: Compiles successfully

**Step 6: Commit**

```bash
git add src/view/mod.rs
git commit -m "feat: add image viewer and binary placeholder renderers"
```

---

## Task 7: Add "Open with Default App" action for binary placeholder tabs

**Files:**
- Modify: `src/messages.rs` (add `LayoutMsg::OpenWithDefaultApp`)
- Modify: `src/update/layout.rs` (handle the message)
- Modify: `src/runtime/keyboard.rs` or equivalent keyboard handler (wire Enter key)

**Step 1: Add the message variant**

Find `LayoutMsg` in `src/messages.rs` and add:

```rust
/// Open the current file with the system's default application
OpenWithDefaultApp(PathBuf),
```

**Step 2: Handle the message in `update_layout()`**

In `src/update/layout.rs`, in the `update_layout()` match, add:

```rust
LayoutMsg::OpenWithDefaultApp(path) => {
    Some(Cmd::OpenInExplorer { path })
}
```

**Step 3: Wire up Enter key for binary placeholder tabs**

Find where Enter key is handled in the editor context (likely keyboard handler). When the focused editor has `tab_content == TabContent::BinaryPlaceholder`, dispatch `LayoutMsg::OpenWithDefaultApp` instead of the normal text editing Enter behavior.

The exact file location needs investigation — look for where `KeyCode::Enter` is matched for the editor, and add an early check:

```rust
// If focused editor is a binary placeholder, Enter opens with default app
if let TabContent::BinaryPlaceholder(ref state) = editor.tab_content {
    return Some(Msg::Layout(LayoutMsg::OpenWithDefaultApp(state.path.clone())));
}
```

**Step 4: Wire up double-click on binary placeholder content area**

In the mouse handler (`src/runtime/mouse.rs`), when a double-click lands on `HitTarget::EditorContent` and the editor has `TabContent::BinaryPlaceholder`, dispatch `OpenWithDefaultApp`.

**Step 5: Verify it compiles**

Run: `cargo check`
Expected: Compiles successfully

**Step 6: Commit**

```bash
git add src/messages.rs src/update/layout.rs src/runtime/keyboard.rs src/runtime/mouse.rs
git commit -m "feat: wire up open-with-default-app for binary placeholder tabs"
```

---

## Task 8: Guard text editing actions against non-text tabs

**Files:**
- Modify: `src/update/text_edit.rs` or wherever `DocumentMsg` / `EditorMsg` handlers are
- Modify: `src/update/editor.rs` if separate

**Step 1: Add early returns in editing handlers**

In the update functions that handle `DocumentMsg` (insert, delete, etc.) and cursor `EditorMsg` (move, select, etc.), add an early guard:

```rust
// Skip text operations for non-text tabs
if !matches!(editor.tab_content, TabContent::Text) {
    return None;
}
```

This prevents crashes or nonsensical operations on image/placeholder tabs where the document buffer is empty.

**Step 2: Verify it compiles and existing tests pass**

Run: `make test`
Expected: All existing tests pass

**Step 3: Commit**

```bash
git add src/update/
git commit -m "feat: guard text editing actions for non-text tabs"
```

---

## Task 9: Test end-to-end and polish

**Step 1: Build and run**

Run: `make build && make run`

Test the following scenarios:
- Open a `.png` file from the sidebar → should show image inline in a tab
- Open a `.jpg` file → should show image
- Open a binary file (e.g., compiled executable) → should show placeholder with filename, size, and hint
- Press Enter on the placeholder → should open file with system default app
- Switch tabs between text, image, and placeholder → should render correctly
- Close image/placeholder tabs → should work without errors
- Re-open an already-open image → should focus existing tab (deduplication check)

**Step 2: Run full test suite**

Run: `make test`
Expected: All tests pass

**Step 3: Run lints**

Run: `make lint`
Expected: No warnings

**Step 4: Final commit**

```bash
git add -A
git commit -m "feat: image viewer and binary placeholder for unsupported files"
```

---

## Notes

- **SVG is NOT supported** by the `image` crate's rasterizer. SVG files will fall through to the text editor (they're XML, not binary), which is fine — they'll show the SVG source code.
- **Memory consideration:** Decoded RGBA pixels for a 4K image ≈ 32MB. The existing 50MB file size limit provides a natural guard. For extremely high-resolution images, consider adding a pixel count limit (e.g., 100 megapixels) in a future iteration.
- **Scaling quality:** Nearest-neighbor is used for simplicity. Bilinear/bicubic can be added later if quality matters for zoomed views.
- **No zoom/pan yet:** This implementation shows the image fitted to the pane. Zoom and pan can be added as a follow-up by extending `ImageTabState` with scale/offset fields and wiring mouse events.
