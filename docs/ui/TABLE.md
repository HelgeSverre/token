# Table

## Purpose and current boundary

A table presents two-dimensional records with column semantics. It is neither a
Tree with labels nor a generic List. Token's only verified table-like surface is
CSV viewer/editor mode; there is no reusable `Table`, sortable-header system,
or table toolbar.

## Verified CSV model, rendering, and workflows

CSV is an alternate `EditorState` view sharing the document buffer, built from
parsed `CsvData`, `CsvViewport`, and optional `CellEditState` (`src/csv/mod.rs:1-20`).

| `CsvState` field                      | Owner/use | Verified behavior                                  |
| ------------------------------------- | --------- | -------------------------------------------------- |
| `data`, `delimiter`, `has_header_row` | CSV model | parsed grid and format/header policy               |
| `selected_cell`                       | CSV model | clamped then made visible on selection             |
| `viewport`                            | CSV model | visible rows/columns and scroll                    |
| `column_widths`                       | CSV model | character widths from up to 100 rows, clamped 4–40 |
| `editing`                             | CSV model | isolated editable single-cell state                |

These fields and the selection/edit constructors are defined in
`src/csv/model.rs:388-510`. `CsvMsg` handles navigation, click/double-click,
both scroll axes, edit lifecycle, and in-cell text commands
(`src/update/csv.rs:17-103`). `update::mod` maps ordinary editor/document input
differently in grid-navigation versus cell-edit state (`src/update/mod.rs:450-488`).
Rendering enters at `Renderer::render_csv_grid` and `render_csv_cell_editor`
(`src/view/mod.rs:1483-1708`), which keeps CSV out of text-only paths.

## Anatomy, ownership, transitions

Verified anatomy: grid/body, row/column headers, selected-cell wash/border,
visible viewport, cell content, and in-place cell editor. CSV selection colors
are used by the renderer (`src/view/mod.rs:1506-1507`) and declared in theme
data (`src/theme.rs:752-767`, `CsvThemeData`).

Transition: parse/toggle creates CSV state → selection clamps and reveals → key
or pointer creates `CsvMsg` → update either moves selected cell or edits its
`EditableState` → confirm writes a change back to the document/update path
(`src/update/csv.rs:415-450`);
cancel drops edit state. Layout/render/hit must derive cell bounds from the same
CSV render layout; do not add an independent row/column calculation in input.

**Proposed general Table only if another consumer needs it:** owner supplies
stable row/column IDs, labels/types/alignment, values, capabilities, selection,
viewport, and mutations. Table supplies resolved header/cell rectangles and
ID-based hit/navigation; update owns commands. Preserve CSV's character-grid
sizing and document synchronization as CSV-specific until proven reusable.

## Keyboard, pointer, focus, accessibility

**Verified:** CSV supports arrows, Tab traversal, Home/End/page movement, Enter
confirm/move, typed entry, and Escape cancel through CSV messages
(`src/update/csv.rs:22-50`). No verified sort/reorder/resize header pointer,
multi-cell ranges, copy-as-table, frozen headers, grid accessibility role, or
explicit focus ring exists.

**Proposed:** navigation and text editing remain distinct states; headers gain
sort/resize only with named messages; selection always ensures visible; editing
announces validation/error. Use table/grid semantics with row/column count,
header names, selected cell and edit state. Ordinary tables use UI font and
column alignment; CSV may retain code-grid metrics.

## Layout, gallery, acceptance

Clip cell text, virtualize hidden rows/columns, scale headers/scrollbars, and
give header/data cells a single geometry authority. Primary IntelliJ guidance
calls for short noun headers, header/content alignment, persistent headers on
scroll, meaningful empty state, and toolbars only for data manipulation:
[Table guideline](https://plugins.jetbrains.com/docs/intellij/table.html).

No table/CSV specimen appears in the gallery catalogue (`src/model/gallery.rs:82-145`).
Priority 1: header/no-header, empty parse, long cell, selected/edited cell,
both overflow axes, readonly/error, HiDPI. Priority 2: generalize only with a
second consumer. Acceptance: paint/pointer share coordinates, selection clamps
after data change, edits safely synchronize document, virtualization remains,
and navigation/edit/cancel tests pass.

## Sources

Primary: [IntelliJ Table](https://plugins.jetbrains.com/docs/intellij/table.html).
Secondary theme/DPI context: `temporary-docs/intellij-platform-sdk/references/themes.md`.
