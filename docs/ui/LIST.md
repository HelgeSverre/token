# List

## Purpose and boundary

A list shows one-dimensional ordered rows. It is not a hierarchy (Tree), a
multi-column grid (Table), a transient command surface (Menu), or a peer-surface
selector (Tabs). Token has reusable row layout/traversal primitives, but no
single public `List` widget or shared selection state machine.

## Verified implementation and consumers

`RowListDecl`/`RowListView` provide virtualized rows in the layout snapshot.
`render_tree` consumes the supplied `RowListView.drawn_range()` and
`row_rect(index)`, walking prior ancestors but painting only visible indices
(`src/view/tree_view.rs:1-87`). Chrome creates `PanelRows` row lists for active
panel content such as Problems and Outline (`src/layout/chrome.rs:250-280`);
overlay layout exposes resolved display rectangles through
`OverlayLayout.rows` (`src/view/overlay_surface.rs:850-880`, `:1018-1040`,
`:1354-1450`).

| Row family                        | Data/selection owner                  | Why it is not one generic List yet                     |
| --------------------------------- | ------------------------------------- | ------------------------------------------------------ |
| Palette/file/reference/completion | corresponding modal or cursor overlay | filtering, text query, acceptance/dismiss rules differ |
| Settings records                  | settings modal                        | forms/actions and record viewport                      |
| Problems/Usages/Outline           | domain panels                         | domain scrolling, open/jump commands                   |
| File tree projection              | workspace                             | hierarchy/expansion semantics                          |

The gallery includes visual `ListRow` and `MenuRows` previews, not an
interactive data-backed generic list (`src/model/gallery.rs:45-57`).

## Anatomy, ownership, transitions

**Verified anatomy:** consumer-specific icon, primary label, detail/accessory,
section header, hover/selected presentation, `RowListView` viewport and
scrollbar. In overlay lists, `OverlayLayout.rows` contains visible display rows
including headers; headers are not ordinary selectable values
(`src/view/overlay_surface.rs:860-880`).

**Proposed contract:** owner supplies stable row IDs, current ordered visible
data, selectable/enabled state, selection/focus ID, row metadata, viewport
scroll, and messages. List supplies resolved row geometry, `row_at`,
visible-range iteration and optional ID-based selection movement; it never owns
I/O or effects. Transition is: replace/filter → preserve selection by ID or
nearest enabled visible row → ensure visible → layout once → paint/hit using the
same snapshot → emit ID → owner update invokes/selects/dismisses as declared.

## Keyboard, pointer, focus, accessibility

**Verified:** runtime handles cursor overlays before editor input and gives dock
focus exclusive domain keyboard routes (`src/runtime/input.rs:220-285`,
`:1155-1185`). No common list key handler, type-ahead, list accessibility role,
or shared focus ring exists.

**Proposed defaults:** Up/Down, Home/End, PageUp/PageDown move selection;
Enter invokes; click selects; double click invokes only when consumer declares
it; wheel changes viewport but not selection. Disabled rows are never invoked.
Use `listbox/option` semantics (or native equivalent) only for selectable data,
with count/position/selected/disabled accessible state and visible focus.

## Geometry, visual contract, edge cases

Use `RowListView` and shared scrollbar geometry; do not add a local logical-row
loop that assumes all rows are drawn. Paint and hit-test resolved row rects,
clip label/detail/accessory, and let the surface choose theme roles (`overlay`,
`sidebar`, or domain palette). UI font is normal; terminal/code outline may
intentionally select Code. Validate empty/loading/error states, long details,
partly visible endpoint rows, reorder/filter during focus, and zero-size
viewport. Empty state must explain why and offer an action where useful, per
[IntelliJ table guidance](https://plugins.jetbrains.com/docs/intellij/table.html).

## Gallery, priorities, acceptance

Priority 1: gallery fixtures for empty, selected, hover, disabled, long
detail/accessory, focus, scroll endpoints and narrow/HiDPI. Priority 2: extract
an internal shared list contract only when a second domain can share geometry
and keyboard tests without losing its semantics. Acceptance: virtualized
iteration remains; one snapshot drives paint/hits; selection stays stable by ID
through filtering; pointer/keyboard/activation behavior is tested.

## Sources

Primary: [IntelliJ list and tree controls](https://plugins.jetbrains.com/docs/intellij/lists-and-trees.html).
Local secondary context: `temporary-docs/intellij-platform-sdk/references/ui-settings-and-toolwindows.md`.
