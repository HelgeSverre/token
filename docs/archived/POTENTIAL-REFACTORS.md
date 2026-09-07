# Potential Refactors

Refactoring opportunities identified in the view/UI rendering layer.

## High Severity

### 1. Split `render_modals()` into per-modal functions

**File:** `src/view/modal.rs:50-594`

The `render_modals()` function is a single 544-line match statement with 6+ arms. Each arm repeats the same patterns: border drawing, title rendering, input field setup, and list rendering with scroll offsets.

**Fix:** Extract each arm into its own function (`render_command_palette_modal()`, `render_goto_line_modal()`, `render_find_replace_modal()`, `render_theme_picker_modal()`, `render_file_finder_modal()`, `render_recent_files_modal()`), plus shared helpers for the common header/list rendering.

### 2. Deduplicate modal text field rendering

**File:** `src/view/modal.rs:148-531`

`TextFieldOptions` is constructed identically 7 times across different modals — same padding calculation, same centering logic, same options struct.

**Fix:** Create a `render_modal_input()` helper in `text_field.rs` that takes the input rect, colors, and editable state.

### 3. Unify sidebar and outline tree rendering (partially done)

**File:** `src/view/panels.rs`

Context structs (`SidebarRenderContext`, `OutlineRenderContext`) and a shared `TreeListLayout` were introduced, reducing parameter passing. However, the two rendering paths (`render_node` and `render_outline_node`) remain separate with duplicated scroll/clip/selection logic.

**Remaining work:** Extract a generic `render_tree()` function parameterized over a `TreeNode` trait to unify the traversal and rendering logic.

### 4. Extract `column_to_pixel_x()` helper

**Files:** `src/view/editor.rs`, `src/view/panels.rs`

The pattern `visual_col → saturating_sub(viewport.left_column) → multiply by char_width → round` appears 15+ times across the rendering code.

**Fix:** Add a single `column_to_pixel_x(visual_col, viewport_left, text_start_x, char_width) -> usize` function in `geometry.rs`.

## Medium Severity

### 5. Introduce `RenderContext` struct for editor rendering

**File:** `src/view/editor.rs:24-759`

`render_text_area()` takes 7 parameters, `render_gutter()` takes 6. All extract the same metrics (`char_width`, `line_height`) from `painter` and `layout`.

**Fix:** Create an `EditorRenderContext` struct bundling `frame`, `painter`, `model`, `layout`, `editor`, and `document`.

### 6. Centralize theme color extraction

**Files:** `src/view/editor.rs`, `src/view/modal.rs`, `src/view/panels.rs`

Calls like `model.theme.editor.foreground.to_argb_u32()` appear 20+ times. Each rendering function independently extracts the same colors.

**Fix:** Create an `EditorColors` palette struct computed once per frame and passed through the rendering pipeline.

### 7. Split `geometry.rs` into submodules

**File:** `src/view/geometry.rs` (1795 lines)

Mixes layout helpers (~60 functions), modal spacing constants, tree layout details, and hit-test logic in a single file.

**Fix:** Split into submodules: `layout.rs`, `modal_layout.rs`, `tree_layout.rs`, `constants.rs`.

### 8. Reduce preview rendering parameter count

**File:** `src/view/mod.rs:412-530`

Both `render_native_html_preview()` and `render_native_markdown_preview()` take 8 parameters each (already suppressed with `#[allow(clippy::too_many_arguments)]`).

**Fix:** Create a `PreviewRenderContext` struct wrapping the shared parameters.
