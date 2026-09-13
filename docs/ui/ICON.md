# Icon

## Current representations and ownership

Token has icon paint utilities and several semantic producers, but no `Icon`
component/catalog. Domain code owns meaning; renderer receives a glyph and
fallback only for one paint call.

| Producer          | Durable owner                    | Render representation            | Identity invariant                                                   |
| ----------------- | -------------------------------- | -------------------------------- | -------------------------------------------------------------------- |
| Workspace tree    | `FileNode { is_dir, extension }` | Nerd Font `&'static str`         | folder expansion picks closed/open glyph; files use cached extension |
| Panels            | `PanelId`                        | `panel_icon(PanelId)`            | panel identity, not an icon button                                   |
| Overlay file rows | modal result                     | `RowIcon::Glyph { ch, color }`   | first extension glyph or fallback                                    |
| Completion rows   | completion item                  | `RowIcon::KindBadge`             | badge geometry, documented separately                                |
| Problems tree     | problems row                     | `draw_icon(ch, fallback, color)` | chevron and file cell are separate                                   |

**Current excerpt — semantic source stays outside paint**

```rust
pub struct FileNode {
    pub name: String, pub path: PathBuf, pub is_dir: bool,
    pub children: Vec<FileNode>, pub extension: FileExtension,
}
impl FileNode {
    pub fn icon(&self) -> &'static str {
        if self.is_dir { "\u{F07B}" } else { self.extension.icon() }
    }
    pub fn icon_expanded(&self) -> &'static str {
        if self.is_dir { "\u{F07C}" } else { self.extension.icon() }
    }
}
```

`FileNode` owns paths, children, and cached extension. A scan/reload that changes
path/type must recompute extension before rendering. Returned glyph data are
static Nerd Font strings; consumers deliberately take `.chars().next()` because
`draw_icon` accepts `char`, and provide `?`. A missing semantic glyph selects
that fallback codepoint without changing node identity; if fallback is absent
too, the font may still rasterize its `.notdef` glyph rather than a question mark.

## Ink-fitting algorithm and units

`TextPainter<'a>` borrows active font and mutable `GlyphCache`; a scoped
`with_font` borrow restores its prior Code/UI role on `Drop`, so icon work cannot
leak typography into subsequent paint.

**Current algorithm — `TextPainter::draw_icon`**

```text
if cell.width <= 1 or cell.height <= 1: return       // physical px
font = choose active/fallback font containing glyph, otherwise fallback glyph
bounds = font.metrics(glyph, 1.0).bounds
if bounds.width <= 0 or bounds.height <= 0: return
size = min((cell.width-1)/bounds.width, (cell.height-1)/bounds.height)
bitmap = glyph_cache[(glyph, size.to_bits)] or rasterize(font, glyph, size)
x = round(cell.x + (cell.width-bitmap.width)/2)
y = round(cell.y + (cell.height-bitmap.height)/2)
push cell clip; paint bitmap; pop clip
```

The calculation uses visible `bounds`, not em square or advance. One physical px
is reserved for raster rounding. For cell `(10,10,14,14)` and unit ink bounds
`(7,10)`, `size=min(13/7,13/10)=1.3`; a 9×13 raster starts at
`(round(12.5),round(10.5))=(13,11)`. Ink remains contained even with unusual
bearings/advance.

Problems-tree callers make padded cells first: an `8×scale` chevron cell and a
`14×scale` file cell centered in the row. Chevron hit testing still uses the
tree layout's original indicator column. Paint clipping therefore cannot move
interaction geometry, and `draw_icon` creates no hit/focus target itself.

## Passive projection, invalidation, and cost

Icons have no event/focus/capture/cancel state. Expansion or row selection is
owned by tree/list update; renderer reads node identity and projects glyphs.
Current `Workspace::refresh` calls `file_tree.refresh(&root)`, replacing the
tree while retaining `selected_item` and `expanded_folders`; it does not repair
them against removed paths. Repair by stable identity is desirable future work,
not current behavior.
Glyph bitmap cache key is `(char, physical-size bits)`: font, fallback, glyph,
cell size and scale affect output; color does not. Visible icon cost is metrics
lookup plus cached bitmap paint, with rasterization only on miss. No icon layout
cache exists. Current Token has no accessibility tree; important icon-only
actions must acquire names at their owning control when that tree is added.

## Traces, integration, and tests

A collapsed Problems file projects `▸` in its 8px cell and first extension glyph
in the adjacent 14px cell. If Nerd Font lacks it, its fallback codepoint (usually
`?`, otherwise potentially `.notdef`) still stays within that second cell and
cannot expand/move the chevron target. A one-pixel cell returns before
rasterization. Existing [frame.rs](../../src/view/frame.rs) tests verify centered
contained ink at scales and missing-primary glyph fallback. Add vectors for the
7×10/14px calculation above, nonzero bearings under clip, scale 1→1.25 distinct
cache keys, and chevron hit testing unchanged by a file icon.

Flow is `workspace/modal model → semantic glyph adapter → renderer consumer →
TextPainter::draw_icon`; model/update does mutation and render is projection.

```rust
// proposed API — not implemented
// FileExtension is re-exported from model::workspace; PanelId lives in
// panel::dock. Rect is model::editor_area::Rect with f32 physical-pixel fields.
enum IconName {
    File(crate::model::FileExtension),
    Folder { expanded: bool },
    Panel(crate::panel::dock::PanelId),
}
struct IconPaint {
    cell: crate::model::Rect,    // caller-owned physical-pixel bounds
    fallback: char,              // semantic degradation chosen by consumer
    color: u32,                  // resolved ARGB, not a model theme token
}
fn resolve_icon(name: IconName) -> &'static str { /* model-to-Nerd-glyph adapter */ }
```

`IconName::File` retains extension semantics without a path; `Folder` retains
the expansion bit; `Panel` retains a stable panel identity. `IconPaint::cell` is
not a new layout owner, `fallback` has an intentional degradation codepoint but
not a guarantee of readable ink, and
`color` is a resolved frame value. `resolve_icon` belongs at the view adapter
boundary; model must not depend on fonts or a glyph cache.

Sources: [frame.rs](../../src/view/frame.rs), [workspace.rs](../../src/model/workspace.rs),
[panels.rs](../../src/view/panels.rs), [modal.rs](../../src/view/modal.rs).
