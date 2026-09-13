# Table implementation manual

Token's only table is CSV viewer/editor mode: an alternate `EditorState` view
over the same `Document`, not a reusable sortable grid. Parsed CSV values,
selection, viewport, widths, in-cell editing and document synchronization form
one feature. A generic Table must not be inferred from this implementation.

## 1. Current model and ownership

```rust
// current excerpt — src/csv/model.rs
pub struct CellPosition { pub row: usize, pub col: usize }
pub struct CsvData { rows: Vec<String>, column_count: usize }
pub struct CsvState {
    pub data: CsvData,
    pub selected_cell: CellPosition,
    pub viewport: CsvViewport,
    pub delimiter: Delimiter,
    pub has_header_row: bool,
    pub column_widths: Vec<usize>, // character cells, never pixels
    pub editing: Option<CellEditState>,
}
pub struct CellEditState {
    pub position: CellPosition,
    pub editable: EditableState<StringBuffer>,
    pub original: String,
    pub scroll_x: usize,
    pub original_column_width: usize,
}
```

`CsvData` stores a parsed row as one delimiter-separated internal string, not
`Vec<Vec<String>>`; `get` scans one row and `set` rebuilds one row
(`src/csv/model.rs:290-388`). It belongs to `ViewMode::Csv(Box<CsvState>)`;
the Document buffer/revision remain the authoritative persistent text
(`src/csv/mod.rs:1-20`, `src/model/editor.rs:601-626`).

| State                    | Owner                       | Invariant and repair                                                                      |
| ------------------------ | --------------------------- | ----------------------------------------------------------------------------------------- |
| document buffer/revision | `Document`                  | confirmation syncs a validated cell edit; mismatch retains editor rather than overwriting |
| parsed grid/delimiter    | `CsvState`                  | reparse after document replacement according to owner policy                              |
| selected cell            | `CsvState`                  | clamped to row/column bounds when nonempty                                                |
| viewport                 | `CsvViewport`               | top/left clamped after move, wheel, resize, or shape change                               |
| widths                   | `CsvState`, character units | initial scan first 100 rows, 4–40; edit may grow to 32; cancel restores original          |
| cell editor              | `CsvState`                  | points at current cell; confirm commits/retains; cancel discards                          |
| grid rectangles          | `CsvRenderLayout`           | frame-local: recreate from bounds, metrics, width/viewport inputs                         |

`toggle_csv_mode` rejects an empty or zero-column parse
(`src/update/csv.rs:107-153`). A `saturating_sub(1)` on empty data is never
permission to index row or column zero.

## 2. One geometry authority

`CsvRenderLayout::calculate` is the shared algorithm for row header, grid
origin, header row and visible columns. Renderer, cell editor/caret, and hit
testing independently recompute it from equivalent state/metrics in their own
coordinate spaces; it is not a retained cross-phase layout object
(`src/csv/render.rs:48-237`, `src/view/caret.rs:192-215`).

```text
digits = floor(log10(max(row_count,1))) + 1
row_header_width = trunc(max(digits,3) × char_width + 16)
grid_x = rect_x + row_header_width
grid_width = saturating_sub(rect_w, row_header_width)
data_y = content_y + line_height
column_width_px(c) = ceil(column_widths[c] × char_width + 12)
cell(row,c).y = data_y + (row - top_row) × line_height
```

From `viewport.left_col`, layout appends `(column,x_offset)` until adding the
next would overflow `grid_width` and at least one column has already been
admitted. Thus one oversized first column remains represented. Current
`render_csv_grid` does not establish a dedicated grid/group clip around that
paint, so the intended clipping boundary is a known gap rather than a verified
property; a future clip must cover the same grid geometry. Hit testing
first subtracts `group_rect.x/y`, then rejects negative local coordinates, tab
bar/header areas, missing data rows, and column gaps. It does not independently
enforce the full group rectangle or `viewport.visible_rows`; `hit_test_groups`
performs the outer group routing before runtime delegates the click to renderer
geometry and emits `CsvMsg::ClickCell`
(`src/runtime/mouse.rs:2303-2329`).

### Worked trace

With `rect_x=40`, `rect_w=500`, `content_y=30`, `line_height=20`,
`char_width=8`, 12 rows, widths `[4,10,4]`, and `left_col=0`:

```text
header = trunc(3×8+16) = 40; grid_x=80; data_y=50; grid width=460
pixel widths = [44,92,44]
A spans x=80..124; B=124..216; C=216..260
```

At `top_row=3`, spreadsheet B5 is zero-based `(row=4,col=1)`, hence screen row
1 and rect `(124,70,92,20)`. `(123,70)` is a row-header miss; `(124,70)` is B5
because boundaries are half-open. At a 30px
group width, grid width saturates to zero but the first column still exists.
Current paint may overflow that intended boundary; a future grid clip should
contain it rather than dividing by width or creating a second input formula.

### Prefix geometry for a future horizontally virtual table

Current CSV retains offsets only for the visible columns, building them by an
incremental sum on every `CsvRenderLayout::calculate`. That is the correct
simple path while the renderer needs the sequential run. If a future general
table needs random x-to-column lookup, frozen panes, or a million columns, its
owner should cache a prefix vector—not remeasure cells in every hit path:

```text
P[0] = 0
P[c+1] = P[c] + column_width_px(c)       // P has column_count + 1 entries
visible_start = upper_bound(P, scroll_x) - 1
visible_end = lower_bound(P, scroll_x + viewport_width)
screen_x(c) = grid_x + P[c] - scroll_x
column_at(px) = upper_bound(P, px - grid_x + scroll_x) - 1
```

For widths `[44,92,44]`, `P=[0,44,136,180]`. With `scroll_x=50` and
`grid_x=80`, column B begins at x=74 (partially clipped) and C at x=166;
`px=124` transforms to content x=94 and `upper_bound(P,94)-1=1`, B. Rebuild
`P` when column order, width, scale/character metrics, or column count changes;
row-only scroll and selection changes do not invalidate it. This is proposed
geometry, not a description of a current global CSV prefix cache.

For in-cell text, `CELL_TEXT_PAD_X=4` is shared by draw, caret, and click:

```text
text_x = x_in_cell - 4
column = scroll_x + round(text_x / char_width), clamped to live char count
```

Left of the pad selects the first visible character; a right-half glyph press
selects after it. Current tests prove this inverse caret mapping
(`src/csv/render.rs:244-288`).

## 3. Viewport and navigation transitions

`rows_for_content_height` reserves the letter-header line and returns
`max(1, (content_height-line_height)/line_height)` (`src/csv/viewport.rs:1-25`).
`ensure_visible` is the shared selection repair:

```text
if row < top: top=row
else if row >= top+visible_rows: top=row-(visible_rows-1)
if col < left: left=col
else if col >= left+visible_cols: left=col-(visible_cols-1)
top=min(top, total_rows.saturating_sub(visible_rows))
left=min(left, total_cols.saturating_sub(visible_cols))
```

For capacity 10×5, selection `(15,8)`, totals 100×20 and initial top/left
zero, this produces `(6,4)`: the cell becomes last visible row/column. Current
tests cover this (`src/csv/viewport.rs:78-126`). `visible_cols` is approximate;
the solved column vector remains paint/hit truth for wide columns.

| State           | Input                       | Update result                                          |
| --------------- | --------------------------- | ------------------------------------------------------ |
| grid navigation | arrows, Home/End, Page, Tab | clamp/move selection then ensure visible               |
| grid navigation | printable character         | start editor replacing cell content                    |
| grid navigation | double click                | select, start editor at pointer column                 |
| cell editor     | text/cursor/undo input      | mutate shared `EditableState`; grow width/caret scroll |
| cell editor     | Enter/Tab/Shift+Enter       | confirm then move declared down/right/up               |
| cell editor     | Escape                      | cancel and restore original width                      |
| either          | wheel                       | scroll grid viewport; selection stays put              |

Runtime routes CSV edit keys before normal editor input and `update_csv` owns
the deterministic transitions (`src/runtime/input.rs:247-255`,
`src/runtime/input.rs:833-1145`, `src/update/csv.rs:18-103`). Text-only paths
must remain gated by `EditorState::is_plain_text_mode()`.

## 4. Pointer, focus, and document transaction

`ClickCell` owns all branch decisions (`src/update/csv.rs:300-365`):

| Press condition              | Result                                                  |
| ---------------------------- | ------------------------------------------------------- |
| edited cell                  | place/extend caret; double click word; triple click all |
| different cell while editing | commit old edit first, then select new cell             |
| unedited cell                | select it                                               |
| unedited double click        | select and edit at pointer column                       |

Clicking away never silently discards. Confirmation escapes/synchronizes the
specific `CellEdit` into the document, schedules syntax/LSP work, and redraws.
If the document no longer matches the expected old cell, it leaves `editing`
alive and reports status instead of overwriting replacement content
(`src/update/csv.rs:437-465`, `:630-675`). Reparse/external replacement must
likewise repair selection or retain a visible conflict; it cannot commit a
stale row/column into new data.

The editor group owns keyboard focus. A CSV cell editor does not need pointer
capture, but focus-loss/tab-switch/view-exit/document-close paths must
explicitly commit, cancel, or block; they cannot accidentally drop `editing`.
CSV rendering must remain excluded from text-only paths.

## 5. Invalidation, cost, and proposed boundary

| Derived output             | Invalidate for                                                        |
| -------------------------- | --------------------------------------------------------------------- |
| parsed data/initial widths | document content, delimiter, mode entry/reparse                       |
| viewport/selection         | dimensions, move/wheel/page/resize, shape change                      |
| `CsvRenderLayout`          | group rect, tab/header height, char/line metrics, left column, widths |
| visible cells              | top row, capacity, visible column vector, data mutation               |
| caret/editor               | edit buffer/cursor/scroll, cell rect, metrics                         |
| sync command               | changed confirmation; revision mismatch rejects write                 |

Initial width calculation visits at most 100 rows; painting is proportional to
visible rows × visible solved columns. `CsvData::get` scans delimiters in a row,
so avoid retrieving offscreen cells or recomputing widths for pointer movement.
These are complexity properties, not timing claims.

A future generic table needs `RowId`/`ColumnId`, typed values, ordering/sort
policy, header/resize geometry, ID selection, and explicit messages. CSV's
delimiter escaping, character metrics, document transaction and editor remain
CSV-specific until another consumer proves compatibility.

## 6. Verification cases

| Case         | Setup/action                             | Expected                                                                                                                          |
| ------------ | ---------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| trace        | geometry above                           | B5 `(124,70,92,20)`; x=123 miss; x=124 maps B5                                                                                    |
| reveal       | 10×5, select `(15,8)`                    | top/left `(6,4)`                                                                                                                  |
| ragged row   | select missing later cell                | valid empty value; bounds use global column count                                                                                 |
| resize       | selected last cell, capacity shrinks     | selection stays in bounds and viewport repairs                                                                                    |
| click-away   | modify A1 then press B2                  | A1 sync before B2 selection                                                                                                       |
| mismatch     | edit A1 then replace document            | edit/status retained; no overwrite                                                                                                |
| cancel width | grow width 4→20 then Escape              | editor none and width restored to 4                                                                                               |
| focus loss   | active cell editor then tab close/switch | explicit shared commit/cancel policy                                                                                              |
| narrow       | width below row header                   | no divide by zero; current first-column paint overflow is characterized and future grid clip contains it; headers never hit cells |

Gallery coverage should show headers, long values, selected/editing cell, both
axes, conflict/error and HiDPI. It cannot prove document transaction safety.
