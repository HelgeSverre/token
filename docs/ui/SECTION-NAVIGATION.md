# Section navigation implementation manual

Section navigation is Token's small, passive category navigator for Preferences
and the UI Gallery. It is not document/dock/terminal/overlay tabs
([TABS.md](TABS.md)). It receives resolved rectangles, labels, and a selected
index, paints them, and maps a point through those same rectangles. The owner,
not this component, retains selection, focus, commands, filtering, and any
asynchronous work.

## 1. Current representation and responsibility split

```rust
// current excerpt — src/view/section_navigation.rs
pub const ROW_STEP: f32 = 33.0;
pub fn section_at(rows: &[WidgetRect], x: f32, y: f32) -> Option<usize>;
pub fn section_rects(bounds: WidgetRect, count: usize, columns: usize, scale: f64)
    -> Vec<WidgetRect>;
pub struct SectionNavigation<'a> {
    pub rows: &'a [WidgetRect],
    pub divider: Option<WidgetRect>,
    pub selected: usize,
    pub scale: f64,
}
```

Gallery owns category, compact-width choice, focus, and scroll; its click
handler changes the owner state (`src/bin/ui_gallery.rs:92-157`). Settings
derives rectangles, maps a hit to an overlay/modal message, and owns active
page and form state (`src/view/settings_page.rs:376-399`, `:626-632`,
`:883-909`). The renderer does not mutate either owner.

| Data                 | Durable owner             | Frame user             | Invalid-state repair                                                |
| -------------------- | ------------------------- | ---------------------- | ------------------------------------------------------------------- |
| labels/order         | Gallery or Settings model | `render`               | owner recreates labels and rects in matching order                  |
| active category      | owner                     | `selected` paint input | current invalid index paints none; owner repairs semantic selection |
| bounds/columns/scale | layout owner              | `section_rects`        | recompute after any input changes                                   |
| divider              | owner/layout              | renderer               | paint-only; never a selectable row                                  |
| pointer/focus        | runtime/owner             | `section_at`           | gap/outside is `None`; no component capture                         |

There is no current `SectionId`, disabled state, keyboard state machine, focus
ring, or accessibility role. A clickable consumer is not evidence that this
passive primitive owns an interaction contract.

## 2. Shared geometry and hit-test algorithm

All dimensions after `scaled` are physical pixels. `scaled(v,s)` is
`max(1, round(v × s))`; columns clamp to one; width uses integer division.

```text
C = max(columns, 1)
cell_width = floor(bounds.w / C)
col(i) = i mod C; row(i) = floor(i / C)
x(i) = bounds.x + col(i) × cell_width
y(i) = bounds.y + row(i) × scaled(33, scale)
w(i) = cell_width; h(i) = scaled(28, scale)
hit(i,p) = rect.x ≤ p.x < rect.x+rect.w && rect.y ≤ p.y < rect.y+rect.h
```

The 5px logical difference between `ROW_STEP=33` and row height 28 is an
intentional non-interactive gap. `section_at` scans only `rows`; the optional
divider is separately painted. Text uses the overlay palette/UI font, a rounded
selected wash, x/y insets 10/7 logical px, and end ellipsis within
`rect.w - scaled(20,scale)` (`src/view/section_navigation.rs:42-128`).

### Worked numerical trace

For `bounds=(100,50,301,200)`, `count=5`, `columns=2`, `scale=1.25`:

```text
cell_width=floor(301/2)=150; step=round(41.25)=41; height=round(35)=35
0=(100,50,150,35), 1=(250,50,150,35)
2=(100,91,150,35), 3=(250,91,150,35), 4=(100,132,150,35)
```

`(249,50)` hits 0, `(250,50)` hits 1, and `(100,85)`/`(100,90)` hit nothing
because they lie in the gap. `(400,50)` is outside the half-open right edge.
The final unused pixel of width 301 must remain unused: separately distributing
it in a pointer handler would make paint and input disagree.

## 3. Consumer flow, visible projection, cost, and invalidation

```text
owner { categories, active ID, bounds, columns, scale }
 → visible_sections: filter/order owner categories once
 → section_rects(visible_sections.len()) once
 → zip each visible ID/label to its same-index rectangle
 → SectionNavigation::render(rows, selected display index)
 → owner/runtime calls section_at(rows, pointer), then maps index → visible ID
 → owner updates its domain state
 → following frame creates new geometry if needed
```

Define `visible_sections` as the ordered, visible projection
`Vec<VisibleSection<SectionId>>`; it is the shared source for labels, rectangle
count, hit-ID lookup, focus movement, and the derived selected display index.
Keep it and `rows` in the consumer's frame layout, or recreate both together
immediately before paint and input. Recomputing a private grid or filtering a
different category slice in input is incorrect at non-unit scale and narrow
widths.

| Output         | Invalidate for                   | Cost                                              |
| -------------- | -------------------------------- | ------------------------------------------------- |
| rectangles     | bounds/count/columns/scale       | `O(count)` placement/allocation                   |
| selected paint | active category/theme            | `O(count)` draw; geometry stable                  |
| truncation     | label/font/available width/scale | one truncation per row                            |
| point lookup   | changed rectangles               | current linear `O(count)` scan; bounded local set |

If categories arrive asynchronously, the owner accepts results only for its
current surface and generation, repairs active identity, then derives the
index. This passive component never owns a task result or caches rectangles
across a bounds/scale change.

## 4. Proposed ID-based owner adapter

The current selection is positional. If items can reorder/filter/become
unavailable, keep a stable ID in the owner and derive the index for painting.

```rust
// proposed API; SectionNavigation remains geometry + paint only.
struct Section<Id> { id: Id, label: String, enabled: bool, visible: bool }
struct SectionState<Id> { selected: Option<Id>, focused: bool }
struct VisibleSection<Id> { id: Id, label: String, enabled: bool }
fn visible_sections<Id: Copy>(items: &[Section<Id>]) -> Vec<VisibleSection<Id>> {
    items.iter().filter(|s| s.visible).map(|s| VisibleSection {
        id: s.id, label: s.label.clone(), enabled: s.enabled,
    }).collect()
}
fn repair<Id: Copy + Eq>(state: &mut SectionState<Id>, items: &[Section<Id>]) {
    if !state.selected.is_some_and(|id| items.iter().any(|s| s.id == id && s.visible && s.enabled)) {
        state.selected = items.iter().find(|s| s.visible && s.enabled).map(|s| s.id);
    }
}
```

Call repair after replacement, reorder, visibility/enablement change, removal,
or accepted async result; then derive `visible_sections`, labels, rectangles,
and selected index from that one projection. Map `section_at`'s visible index
back to its `VisibleSection.id` before storing selection. Never retain index 2
after a filter; it may be a new category. The owner can use `usize::MAX` to
make today's painter show no selection when no usable section exists.

## 5. Proposed focus and input machine

The current primitive has no input machine. An owner that enables keyboard use
should make the container one focus stop:

| State     | Event                     | Result                                                 |
| --------- | ------------------------- | ------------------------------------------------------ |
| unfocused | press enabled section     | focus container and activate its ID                    |
| focused   | arrows                    | use the deterministic visual-grid search below         |
| focused   | Home/End                  | first/last enabled visible ID                          |
| focused   | Enter/Space               | activate selected ID                                   |
| any       | gap/divider/outside press | no activation or capture                               |
| any       | change/removal/resize     | repair by ID before next key/hit                       |
| any       | focus loss/modal dismiss  | clear transient hover/press; durable selection remains |

No pointer capture is needed absent a consumer-added drag feature. Role choice
depends on owner intent: a local replacement surface can use tablist/tab
semantics; application navigation can use navigation/link semantics. Both need
label, current/disabled state, and visible focus separate from selection color.

For `C=max(columns,1)` and visible index `i`, define `(r,c)=(i/C,i%C)`.
Arrow navigation searches rows in the requested direction and uses the nearest
valid column, so a ragged final row is deterministic:

```text
Left/Right: scan c-1..0 or c+1..C-1 in row r; skip index ≥ len or disabled.
Up/Down: for r-1,r-2,... or r+1,r+2,..., test c first, then c-1,c+1,c-2,c+2...
         within 0..C; skip index ≥ len or disabled; stop at first enabled ID.
No eligible candidate: retain selection.
```

Thus in a two-column projection `[a(enabled), b(disabled), c(enabled)]`, where
`c` is the ragged final row at `(1,0)`, Right from `a` retains `a` (b is
disabled), Down from `a` selects `c`, and Down from `c` retains `c`. This is
proposed behavior, not a current helper.

## 6. Verification cases

| Case                   | Initial/action                                  | Expected                                    |
| ---------------------- | ----------------------------------------------- | ------------------------------------------- |
| scale                  | `(0,0,200,100)`, scale 2, 2 rows                | h=56, origins y=0 and 66                    |
| grid gap               | worked trace                                    | x=249→0, x=250→1, y=85/90→none              |
| zero columns           | columns 0                                       | one full-width row; no division by zero     |
| narrow                 | width 1, two columns                            | zero-width cells never activate; text clips |
| reorder                | select ID editor; reorder                       | ID remains editor, derived index changes    |
| removal                | selected item becomes disabled/removed          | first enabled visible ID or explicit none   |
| stale result           | owner generation 4, result generation 3         | no label/selection mutation                 |
| dismissal              | press then modal dismissal                      | no deferred release activation              |
| ragged/disabled arrows | `[a enabled,b disabled,c enabled]`, two columns | Right(a)=a; Down(a)=c; Down(c)=c            |

Gallery fixtures should show vertical/grid, long labels, focused versus
selected, gap miss and HiDPI. They do not prove owner ID repair or key routing.
