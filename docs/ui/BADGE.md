# Badge

## Purpose and catalog mapping

A **Badge** is compact, non-primary metadata that identifies a kind, count, state, or severity in constrained UI. It is not a Button, status-bar segment, Keycap, icon, or arbitrary colored decoration. Token currently has one concrete badge family: completion/list **kind badges**. Fold continuation pills and status/severity text are related visuals but use different geometry/interaction and must not be forced into one type.

The IntelliJ catalog does not define one universal Badge component; its component guidance instead separates icons, notification/severity surfaces, labels and list/table metadata. This proposed Token vocabulary is therefore an implementation-grounded consolidation, not a claim of an IntelliJ API. Use [Icon](ICON.md) when the mark itself is the semantic pictogram; use Badge when compact container plus label/kind mapping is required.

## Current Token contract — high confidence

`overlay_surface::RowIcon::KindBadge(MenuItemKind)` is a real, narrowly scoped completion/list painter.

| Concern           | Current behavior                                                                                                                                                                                                                     |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Data/owner        | `MenuItemKind` is produced by completion sources/LSP mapping; view owns private glyph/color mapping. Kinds include Function/Method, Variable, Type, Keyword, Field, Module/Folder, File, Constant and Other.                         |
| Anatomy           | One 16×16 logical rounded-square (`r4`) at row leading icon slot; a centered 11px metadata glyph. Method/Function share source color but retain glyph distinction.                                                                   |
| Theme             | Background is syntax/overlay semantic source pre-blended 20% over panel, preventing alpha stacking on selection. Glyph uses `overlay.text_bright`; sources use accent, accent-bright, severity roles, dim text or keycap foreground. |
| Geometry          | `ROW_ICON_W` reserves 18 logical px before label. Overlay row measurement/hit geometry is shared, centered vertically in row; badge itself has no separate hit target.                                                               |
| State/input       | It remains passive under normal/hover/selected parent row. Parent row owns selection/activation/focus.                                                                                                                               |
| Consumers/gallery | Completion and dummy completion/menu rows in modal, Context/overlay rows, and `list-row.kind-badge` gallery specimen. Tests exhaust glyph/color mapping.                                                                             |

Other badge-like elements are intentionally not generic Badge: folded-region `⏎ +N lines` uses editor geometry/hit behavior; Keycaps use their own tokens and binding grammar; severity banners use `Severity` plus larger message surface; status labels are feature compositions.

## Proposed Token contract — proposed, limited extraction

Do not create a generic colored-pill API. Extract only if a second passive compact-metadata consumer shares semantic kinds and row layout. Preserve semantic data rather than `Color + String`:

```text
Badge { kind: CompletionKind | FileKind | Severity | Count | Custom, label: ShortText, presentation: SquareGlyph | PillText, emphasis: Subtle | Strong }
BadgeLayout { outer_rect, glyph_or_text_rect }
```

The domain model owns meaning/count/severity; an adapter maps domain type to Badge kind; painter selects visual recipe and returns passive bounds. `Custom` is deliberately deferred until a named consumer proves that arbitrary text/color cannot be a [Label](LABEL.md) or [Icon](ICON.md). Badge never invokes actions, applies filters, or doubles as a status control.

### Geometry, theme, font and accessibility

Each presentation has fixed logical minimum touch-independent visual dimensions and a measured label; reserve its width before parent label truncation, as Rows already do for accessories. For compact list kind badges retain 16×16/r4/18px slot. Pill/count badge must define max digits, truncation/overflow (`99+` policy), and never alter row height silently. Use UI/meta font and fallback glyph measurement. Scale all constants through existing overlay/metrics helpers; mask cache and painter measurement remain shared.

Use semantic source roles with preblending only over known opaque surface; do not nest translucent fills or use color as sole meaning. Ensure glyph/text contrast and provide text alternative through parent row accessible name (for example “method, foo”). Passive Badge is not tab-focusable. If it conveys error/warning independently of nearby text, proposed platform accessibility must expose that severity; current Token has no platform tree.

### State and edge cases

Badge inherits parent hover/selection background but must remain legible; it does not gain its own pressed/selected state. Unknown LSP kind maps predictably to Other rather than panics. Long labels/counts use a defined text/pill policy, not overflow. A badge must not be painted for a zero-width clipped row, and stale LSP result mapping must not outlive row data. Icon-only semantic status should remain Icon when no container/label/count is required.

## Consumers, gallery, acceptance

Existing kind badge is the only current consumer and should remain in `overlay_surface`; no new generic implementation slice is authorized. A proposed File/Folder badge or diagnostic-count pill needs a concrete row/status consumer plus semantic mapping review first. Do not merge fold pill, terminal glyphs, Keycaps or severity banner just to increase reuse.

Maintain the existing `list-row.kind-badge` specimen and add states only with shared painter: every kind including Other; normal/hover/selected row; narrow row preserving accessory width; light/dark/scale contrast; unknown kind; and any future count overflow/severity mapping. Acceptance: exhaustive kind mapping test; row layout reserves badge before text; badge rectangles derive from same layout as paint; contrast/non-color semantic alternative verified; passive badge creates no hit/focus action; gallery is real production paint.

## Evidence

- Token: [kind badge dimensions/RowIcon/paint](../../src/view/overlay_surface.rs), [completion kind mapping](../../src/completion/menu.rs), [LSP mapping](../../src/completion/lsp.rs), [fold badge geometry](../../src/view/geometry.rs), [editor fold paint](../../src/view/editor_text.rs), [gallery catalog](../../src/model/gallery.rs).
- Token inventory: [component inventory](../dev/ui-component-inventory.md) and [research brief](RESEARCH-BRIEF.md).
- Primary catalog boundary: [IntelliJ Components](https://plugins.jetbrains.com/docs/intellij/components.html), [Notifications](https://plugins.jetbrains.com/docs/intellij/notifications.html), [Icon Button](https://plugins.jetbrains.com/docs/intellij/icon-button.html).
