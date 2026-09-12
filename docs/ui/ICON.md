# Icon

An icon is a compact visual sign with a stable semantic role. It is not merely a
Unicode character, nor is an icon-only control self-describing. This document
separates current glyph/badge practices from the proposed asset and accessibility
contract.

## Current implementation — verified

Token has several icon-like render paths but no unified Icon component.

| Consumer                | Current representation                      | Ownership/limitations                                        |
| ----------------------- | ------------------------------------------- | ------------------------------------------------------------ |
| workspace files/folders | Nerd Font string from FileType              | requires compatible glyph; file-type semantics live in model |
| panel helper            | Nerd Font glyph or empty string             | panel identity helper, not a painter/registry                |
| command/file modal rows | RowIcon::Glyph                              | overlay surface measures/draws a character                   |
| completion rows         | RowIcon::KindBadge                          | painted letter badge with MenuItemKind color                 |
| overlay/header/severity | single glyph/badge                          | feature-local semantic/palette mapping                       |
| fold gutter             | disclosure glyph                            | text editor renderer and GutterLayout geometry               |
| gallery icon button     | glyph label through standard button painter | visual specimen only                                         |

[Workspace icon mapping](../../src/model/workspace.rs) explicitly states its
Nerd Font v3 dependency. [Panel helper](../../src/panels/mod.rs) is likewise a
Nerd Font helper, and many panel arms are still empty. The bundled primary code
font is JetBrains Mono, while the renderer supplies fallback behavior for missing
glyphs. Therefore an icon can be unavailable, have different advance/ink bounds
than an ordinary code cell, or silently look unlike its intended symbol.

[TextPainter](../../src/view/frame.rs) does have a specialized draw_icon path
that fits visible glyph ink into a caller-supplied cell and falls back when the
active font lacks the character. This is a rendering utility, not an icon
catalog, semantic label system, hit rectangle policy or SVG loader. Overlay
rows reserve icon width when a RowIcon is present and use actual painter
measurement for relevant layout.

## Anatomy and naming

Use the following terms.

| Term        | Contract                                                                               |
| ----------- | -------------------------------------------------------------------------------------- |
| icon asset  | source artwork/glyph plus declared intrinsic logical size                              |
| icon glyph  | an icon rendered from a font code point                                                |
| noun icon   | identifies an object, such as a file kind; it does not invoke an action                |
| action icon | represents an available command; it has an accessible label/tooltip                    |
| status icon | represents state; shape/text communicates state in addition to color                   |
| icon button | action control whose visible label is an icon; button owns hit/focus/press state       |
| badge       | compact metadata label/chip, not a substitute for a universal icon                     |
| gutter icon | line-anchored editor affordance; its lane and source position are part of the contract |

Do not call a glyph-only tab title, file name or keycap an icon component. An
icon button is still a button and uses the shared button interaction contract;
do not fork its hover/pressed/disabled/focus geometry because its label happens
to be symbolic.

## Proposed Token Icon contract

No Rust implementation is implied. Before adding broadly reusable icons, specify
a small semantic interface equivalent to:

```text
Icon {
  semantic_id, category: Action | Noun | Status | Gutter,
  representation: Glyph | Asset,
  intrinsic_logical_size, fallback,
  accessible_label (required for action/status)
}
IconPaint {
  rect, visual_state, palette_role, scale_factor
}
```

The owner chooses semantic ID and action/status meaning; a shared resolver
selects supported representation and fallback; a painter fits within the
provided rectangle; the parent component owns click/focus/tooltip. Geometry
uses a rect and design size scaled once, not character count. The resolver must
not perform I/O during paint. If external SVG/bitmap assets are introduced, load
and validate them through a runtime/prepared asset path and cache rasterization
by asset, scale and theme as appropriate.

The proposal does not require converting established text glyphs. Keep a glyph
representation where its font support, fallback and semantics are deliberately
tested. Use assets where visual identity, sharp HiDPI scaling or theme variants
need guarantees a font cannot provide.

## Theme, font and scaling

Current icon color is feature-specific: sidebar file/folder colors, overlay
text/severity colors, editor/gutter colors and completion kind colors. Reuse
those semantic roles. A general icon palette is **not implemented**; do not add
one solely because several pixels happen to be icons. Add a role only for a
concrete state distinction with a fallback for existing theme YAML.

Code versus Ui role matters: workspace/file glyphs and editor/fold glyphs use
the contexts that currently paint them, while proportional Ui text metrics are
not interchangeable with code-grid placement. Use draw_icon's ink fitting for a
fixed glyph cell when appropriate; use ordinary text measurement for inline
label glyphs. Physical size must derive from intrinsic logical size times display
scale, then fit and clip to its provided rectangle. Never hard-code a 1x pixel
box or assume every glyph has monospaced visual ink.

The primary IntelliJ guidance is useful reference: use simple shapes, distinguish
status by more than color, and tailor default/gutter/tool-window sizes to their
context. It recommends SVG for scalable, HiDPI-friendly custom icon assets.
Those are design inputs, not Token asset-format requirements:
[IntelliJ icon style](https://plugins.jetbrains.com/docs/intellij/icons-style.html)
and [IntelliJ icon implementation guide](https://plugins.jetbrains.com/docs/intellij/icons.html).

## Pointer, keyboard, focus and accessibility

### Existing behavior

Icon-like output has no common pointer contract. Row icons activate through their
row; fold disclosure activates its gutter lane; gallery's close icon activates a
normal button painter. Tooltip/accessible label behavior is feature-specific or
absent. An empty panel icon must not generate an empty hit target.

### Required proposal

- A noun icon has no action hit target unless its containing row has one.
- An action icon has a visible or programmatic text name, tooltip/description,
  keyboard-equivalent command and standard button focus/disabled behavior.
- A status icon has adjacent text, a discoverable description or another
  non-color signal; do not communicate error versus success only by red/green.
- A gutter icon names its document position/lane and obeys editor group focus,
  clipping, folding and keyboard alternatives.
- Icon hit rectangles come from parent component layout, never from glyph ink.
- Pointer hover alone must not be the sole way to discover a control.

Token currently lacks an accessibility tree and general tooltip manager, so these
are requirements for future components rather than claims about shipped behavior.

## Edge cases

- Missing Nerd Font glyph, fallback replacement, emoji substitution or unsuitable
  advance/ink must preserve layout and expose meaningful text where necessary.
- High display scale, light/dark themes and disabled/inactive window state need
  visual verification.
- A file icon in a dense tree may be noun metadata; it must not be confused with
  a row action.
- A status/badge can be colorblind-inaccessible even if it uses a theme role.
- Icon-only labels can be too small for pointer acquisition; parent button
  geometry supplies an adequate target, independent of icon art bounds.
- Gutter icons must not overpaint text/gutter lanes or remain clickable when
  their line is folded/hidden.
- Do not add icon assets to user themes without a resolved fallback and a clear
  security/performance loading boundary.

## Gallery coverage and sequence

Gallery has only icon-button.close, and it is a glyph label painted through the
shared button. It has no noun/status/gutter glyph specimens, fallback/missing
glyph state, fixed-cell fit/crop, dark/light contrast matrix, tooltip/label
proof, disabled icon action, or production file-tree/icon-row composition.

The recommended sequence is:

1. Inventory actual icon consumers and establish semantic IDs plus fallback
   policy, beginning with the production workspace/file and panel gaps.
2. Add production-painter gallery specimens for icon button states, noun file
   row, completion badge, status/severity, and fold gutter at 1x/2x light/dark.
3. Only then decide whether an SVG resolver/asset registry has two real
   consumers; keep icons out of a generalized widget framework.
4. Add semantic names/tooltips/accessibility adapter with the action-control
   work, not as an afterthought.

## Acceptance criteria and evidence

An icon change is ready when semantic category and owner are explicit; its
representation/fallback/font dependency is tested; the parent supplies hit/focus;
color role and non-color status cue work in light/dark/scale variants; clipping
is correct; and the gallery calls production paint code.

High-confidence sources are [workspace model](../../src/model/workspace.rs),
[panel helper](../../src/panels/mod.rs), [frame painter](../../src/view/frame.rs),
[overlay surface](../../src/view/overlay_surface.rs),
[editor text](../../src/view/editor_text.rs),
[gallery model](../../src/model/gallery.rs), and
[gallery guide](../dev/ui-gallery.md), reviewed 2026-09-12. The unified icon
contract, asset resolver and accessibility behavior are proposed, not implemented.
