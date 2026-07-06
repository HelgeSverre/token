# Update / View / Render Layer Review

Review of `src/update/`, `src/view/` (core renderer + components), and related render paths for bugs and improvement opportunities.

---

## Executive Summary

The layers are generally well-structured: Elm dispatch is clean, shared geometry helpers exist, and the partial-redraw back-buffer design is sound. However, there are several **Critical** correctness and performance issues, mostly around:

- Rope byte-vs-char indexing in CSV edits
- State/view-mode not resetting on file load
- Split-view cursor synchronization gaps
- Missing clip rects and hit-test/render ordering mismatches
- Per-frame allocations in preview and hit-test paths

---

## Critical

| ID | Area | File | Issue | Impact | Suggested Fix |
|---|---|---|---|---|---|
| C1 | Update | `src/update/csv.rs:804–891` | `sync_cell_edit_to_document` computes **byte** ranges via `find_row_byte_range` / `find_field_byte_range`, then passes them to `Rope::remove(abs_start..abs_end)` which expects **char** offsets. | Corrupts or panics on any CSV containing multi-byte characters. | Operate on the `Rope` with char indices, or convert byte ranges to char offsets before touching the rope. |
| C2 | Update | `src/update/app.rs:102–153` | `FileLoaded` refreshes the focused `Document` but never resets `EditorState::view_mode` / `tab_content`. | Loading text after an image/CSV/binary tab leaves the wrong renderer active. | Set `editor.view_mode = ViewMode::Text` and `editor.tab_content = TabContent::Text` after loading. |
| C3 | Update | `src/update/document.rs:919–1059` | `DeleteForward` records the edit but never calls `sync_other_editor_cursors`. | Split views editing the same document drift out of sync. | Add `sync_other_editor_cursors` for both single- and multi-cursor forward-delete paths. |
| C4 | Update | `src/update/document.rs:129–217` | Multi-cursor `auto_surround` processes selections in reverse without an overlap guard. | Overlapping selections use stale offsets → corruption or panic. | Merge/deduplicate overlapping selections, or reject surround when they overlap. |
| C5 | View / Hit-test | `src/view/hit_test.rs:943–950` | `hit_test_ui` clones the entire `EditorArea` on every mouse move/click. | High-frequency allocation + deep copy on every pointer event. | Pass a cached `&[SplitterBar]` computed once per frame, or store splitters in `AppModel`. |
| C6 | View / Interaction | `src/view/mod.rs:1735–1744` + `src/view/hit_test.rs:838–897` | Dock overlap: renderer paints right dock then bottom dock (bottom visually on top), but hit-test checks right dock then bottom dock. Clicks in the overlap hit the visually behind pane. | Direct violation of the render-order == interaction-order guardrail. | Align order (choose top pane) or eliminate overlap by sizing bottom dock to `window_width - sidebar_width - right_dock_width`. |
| C7 | View / Rendering | `src/view/editor_text.rs:406–711` | `render_text_area` and `render_cursor_lines_only` never call `frame.set_clip(...)`. | Text/selection/cursors can draw over scrollbars, status bar, or adjacent docks/groups. | Set `frame.set_clip(self.ctx.content_rect)` at the start of text rendering and clear on return. |
| C8 | View / Selection | `src/view/editor_text.rs:251–279` | `rectangle_selection_span_for_line` returns `None` when the mouse column exceeds the line length. | Block selection omits shorter lines entirely. | Return selection from `left_visual_col` to `line_visual_len` when `left_visual_col < line_visual_len`. |
| C9 | View / Rendering | `src/view/editor_text.rs:746–780` | `render_cursor_lines_only` does not self-guard with `EditorState::is_plain_text_mode()`. | Helper will render text over image/CSV/binary tabs if called directly. | Add early return / `debug_assert` when `!editor.is_plain_text_mode()`. |

---

## Important

### Update layer

| ID | File | Issue | Impact | Suggested Fix |
|---|---|---|---|---|
| I1 | `src/update/ui.rs:838–869` | GotoLine clamps column with `len_chars().saturating_sub(1)` instead of `line_length()`. | Last character of a file without trailing newline is unreachable. | Use `model.document().line_length(clamped_line)`. |
| I2 | `src/update/app.rs:219–229` | `OpenFileDialogResult` drops commands from each `OpenFileInNewTab` call. | Opening multiple files skips syntax parsing/recent-files updates. | Collect returned commands and return `Cmd::Batch`. |
| I3 | `src/update/document.rs:1299–1321` | Undo/Redo unconditionally set `is_modified = true`. | Dirty indicator is wrong after undoing to saved state. | Track saved-revision pointer and recompute `is_modified`. |
| I4 | `src/update/app.rs:365–404` | `CopyAbsolutePath` / `CopyRelativePath` call `arboard` directly inside update. | Side effects in the pure update layer; clipboard failures ignored. | Add `Cmd::CopyToClipboard(text)` and handle in command executor. |
| I5 | `src/update/editor.rs:191–202` | `SetCursorPosition` assigns `line`/`column` without bounds checks. | Out-of-range coordinates can crash later code. | Clamp to document bounds. |
| I6 | `src/update/outline.rs:30–39` | `JumpToSymbol` writes `editor.cursors[0]` directly and does not clamp coordinates. | Stale outline can crash the editor. | Use active-cursor helpers and clamp to buffer bounds. |
| I7 | `src/update/layout.rs:790–853` | `move_tab` closes the source group even when `source == target`. | Dragging a tab onto its own group destroys it. | Early-return if `source_group_id == to_group`. |
| I8 | `src/update/document.rs:1918–2041` | `IndentLines`, `UnindentLines`, `Duplicate`, single-cursor `PasteText` lack `sync_other_editor_cursors`. | Split views drift after these operations. | Sync peer cursors with correct deltas. |
| I9 | `src/update/text_edit.rs:265–272` | `InsertText` is split into a loop of `InsertChar` calls. | Large inserts are slow and create many undo records. | Add `DocumentMsg::InsertText(String)` for atomic insertion. |
| I10 | `src/update/app.rs:158–181` | `ReloadConfiguration` returns only `Cmd::redraw_status_bar()`. | Theme changes don't appear until the next event. | Return `Cmd::Redraw` or a batch with full redraw. |
| I11 | `src/update/ui.rs:1186–1207` | `ReplaceAll` sets cursor to `line 0, column = replacement.chars().count()`. | Cursor jumps to nonsensical location. | Place cursor at end of last replacement, or document start. |
| I12 | `src/update/document.rs:1278–1283` | `DeleteLine` always decrements `viewport.top_line`. | Deleting a line below the viewport scrolls the view. | Only adjust when `line_idx < viewport.top_line`. |
| I13 | `src/update/document.rs:109–114` | `compute_matched_brackets` runs even for image/binary/CSV tabs. | Wasted work; latent panic source. | Move call inside `update_document_inner` text-only path. |
| I14 | `src/update/mod.rs:184–227` | `update_traced` clones every message for tracing. | Large messages like `PasteText` are cloned on every update. | Trace by reference or only when tracing is enabled. |

### View / Renderer core

| ID | File | Issue | Impact | Suggested Fix |
|---|---|---|---|---|
| I15 | `src/view/mod.rs:788–807` | `render_editor_area` iterates groups/previews in `HashMap` order. | Non-deterministic z-order/flicker across launches. | Sort keys or use ordered storage. |
| I16 | `src/view/geometry.rs:728–739` | `GroupLayout::visible_columns` ignores vertical scrollbar width. | Horizontal thumb size/position and hit-test geometry are subtly wrong. | Subtract `scrollbar_width` from text area width. |
| I17 | `src/view/hit_test.rs:797–807` | CSV hit-test always returns `CsvCell { row: 0, col: 0 }`. | Mouse interaction with CSV cells is broken. | Compute actual row/col from click coordinates using CSV layout helpers. |
| I18 | `src/view/mod.rs:923–973, 2017–2049` | Native HTML preview does `document.buffer.to_string()` and `html.to_lowercase()` every frame. | O(n) allocation per frame for large HTML. | Cache extracted content and use case-insensitive check without full copy. |
| I19 | `src/view/mod.rs:1004–1059` | Native markdown preview iterates all document lines. | O(document_lines) per frame. | Iterate only visible lines starting near `preview.scroll_offset`. |
| I20 | `src/view/mod.rs:675–692` | `Renderer::render` / `build_render_plan` mutates model via `sync_all_viewports`. | Render is not pure; non-idempotent and harder to test. | Move viewport sync into update layer; make render take `&AppModel`. |
| I21 | `src/view/helpers.rs:28–30` | `trim_line_ending` only strips `\n`, leaving `\r` on CRLF files. | Cursor placement off by one on CRLF lines. | Strip `\r\n` then `\n`. |

### View components

| ID | File | Issue | Impact | Suggested Fix |
|---|---|---|---|---|
| I22 | `src/view/editor_text.rs:89–102, 494–540` | Cursor/selection math assumes one visual column per char. | CJK/emoji cursor and selection drift from glyphs. | Adopt `unicode-width` or use glyph advances consistently. |
| I23 | `src/view/editor_special_tabs.rs:70–84` | Binary button hover uses global `HoverRegion::Button`; `focused` is hardcoded `true`. | Wrong button can highlight; focus ring is always on. | Compute hover from actual button rect; pass real focus state. |
| I24 | `src/view/modal.rs:52–119` | Theme picker draws every row without clipping or scroll. | Long theme lists overflow the window. | Cap visible rows, add `scroll_offset`, and clip. |
| I25 | `src/view/panels.rs:329–459` | Outline panel does not auto-scroll to selected item. | Keyboard-navigated selections can be off-screen. | Compute scroll offset from `selected_index` and `visible_capacity`. |
| I26 | `src/view/text_field.rs:189–206` | Modal inputs hardcode `scroll_x: 0`. | Long queries overflow and cursor is invisible. | Add horizontal scroll and keep cursor visible. |
| I27 | `src/view/panels.rs:185–311` | Sidebar and outline duplicate tree-row rendering logic. | Future changes must be made twice; risk of drift. | Extract shared `render_tree_row` helper. |

---

## Minor

| ID | File | Issue | Suggested Fix |
|---|---|---|---|
| M1 | `src/update/layout.rs:826` | Unnecessary `unwrap` after `contains_key` check. | Use `if let Some(source) = ...`. |
| M2 | `src/update/text_edit.rs:302–305` | `TextEditMsg::Paste(text)` maps to `DocumentMsg::Paste` and discards `text`. | Use provided text directly or rename the variant. |
| M3 | `src/update/ui.rs:30–59` | `BlinkCursor` uses `Vec::contains` inside loop for union. | Use `HashSet` or sorted merge. |
| M4 | `src/update/document.rs`, `csv.rs`, `syntax.rs` | Several places allocate whole buffer as `String`. | Work on rope slices for large files. |
| M5 | `src/update/editor.rs:156–189` | `top_line + visible_lines` uses plain `usize` addition. | Use `saturating_add`. |
| M6 | `src/update/outline.rs:183–225` | `offset + lines as usize` can overflow before clamp. | Use `saturating_add`. |
| M7 | `src/view/mod.rs:738–755` | `clear_back_buffer` fills full `content_rect` for `EditorArea` damage. | Clear only the editor-area sub-rect. |
| M8 | `src/view/frame.rs:104–109` | `x1` casts `rect.x + rect.width` to `usize` before clamping. | Clamp in `f32` before casting. |
| M9 | `src/view/hit_test.rs:484–535` | `hit_test_modal` recomputes filtered lists on every call. | Cache filtered list in modal state. |
| M10 | `src/view/geometry.rs:1477–1479` | Theme picker Y position uses `window_height / 4` without `min(100)` cap. | Apply the same cap as other modals. |
| M11 | `src/view/frame.rs:283–296, 367–374` | `blend_rect` / `dim` re-extract alpha per pixel. | Extract alpha once and loop over raw buffer. |
| M12 | `src/view/selectable_list.rs:28–46` | Viewport scroll anchors selection at bottom of visible window. | Use minimal reveal to avoid jumping. |
| M13 | `src/view/button.rs:57–70` | Focus ring is thin, inset, and skipped for tiny buttons. | Render an outset ring or higher-contrast indicator. |
| M14 | `src/view/modal.rs:617–650` | Width approximated with `chars().count() * char_width`. | Use `TextPainter::measure_width`. |
| M15 | `src/view/panels.rs:336` | `render_outline_panel` ignores `_bg_color` parameter. | Use passed color or remove parameter. |
| M16 | `src/view/editor_text.rs:450–451, 522–523` | Line text and char count recomputed multiple times per line. | Cache in `VisibleTextLine`. |

---

## Top 10 Priority Fixes

1. **CSV cell sync uses byte offsets as char indices** (`update/csv.rs:804–891`) — data corruption / crash on non-ASCII.
2. **File load does not reset editor view mode** (`update/app.rs:102–153`) — wrong renderer after tab type changes.
3. **`DeleteForward` does not sync split views** (`update/document.rs:919–1059`) — split-view drift.
4. **Editor text lacks a clip rect** (`view/editor_text.rs:406–711`) — overdraw into adjacent UI.
5. **`hit_test_ui` clones `EditorArea` per event** (`view/hit_test.rs:943–950`) — per-mouse-event allocation.
6. **Dock overlap render/hit-test mismatch** (`view/mod.rs:1735–1744`, `view/hit_test.rs:838–897`) — clicks hit the wrong pane.
7. **Rectangle selection broken on short lines** (`view/editor_text.rs:251–279`) — block selection is incomplete.
8. **Undo always marks document modified** (`update/document.rs:1299–1321`) — dirty indicator lies.
9. **Native preview per-frame allocations** (`view/mod.rs:923–973`, `1004–1059`) — large-file performance cliff.
10. **Cursor-line fast path not self-guarding** (`view/editor_text.rs:746–780`) — violates non-text tab invariant.
