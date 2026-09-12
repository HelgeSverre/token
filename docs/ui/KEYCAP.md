# Keycap

## Purpose and boundary

A **Keycap** visually represents one key or modifier; a **binding sequence** is ordered chord steps made of keycaps. It is passive instructional/accessory content, not an interactive Button, Badge, or text substitute. Its scope is Token's platform-formatted keybindings in overlay/menu/settings contexts.

## Current Token contract — high confidence

This is a real reusable painter, though not a named widget. `frame::draw_keycap` draws rounded bordered chip and centered label; `keycap_width` is the matching measurement API. `overlay_surface::binding_chips` parses a display binding into `Vec<Vec<Chip>>`: outer chord steps, inner modifier/key chips. It handles macOS glyph modifiers, textual Ctrl/Alt/Shift/Win prefixes, multi-character keys like F12, plus key, and chord spaces.

| Contract          | Current behavior                                                                                                                                                                                |
| ----------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Data/owner        | `Chip { label }`; callers own binding formatting and decide `Accessory::Keycaps` versus dim text. `chip_count` supports the current >4-chip fallback.                                           |
| Geometry          | 11 logical px text, minimum 17 logical px width, 4px horizontal padding, rounded radius 4, extra bottom border, height from measured line height plus scaled vertical padding.                  |
| Composition       | Overlay Rows measure accessories first, reserve their full width, paint keycaps at centered row height, with 4px intra-step/6px inter-step gaps. Settings page shares equivalent width/drawing. |
| Theme/font        | `overlay.keycap_bg`, `keycap_border`, `keycap_fg`; label uses painter sizing/fallback glyph behavior. Theme resolution enforces keycap foreground contrast.                                     |
| Input/a11y        | Passive: no focus/hit/keyboard/a11y semantics. It communicates a shortcut also available through command/keymap infrastructure.                                                                 |
| Consumers/gallery | Command palette, context menu, Settings Keymap, overlay menu rows, and Gallery menu specimens use it; no dedicated keycap gallery specimen exists.                                              |

## Proposed Token contract — proposed extraction, not new widget framework

The current primitive is enough. Clarify the data boundary before adding variants:

```text
KeycapSequence { steps: &[KeycapStep { keys: &[Keycap { label }] }], presentation: Full | Compact }
KeycapLayout { chip_rects, step_rects, total_rect }
```

The keymap/platform formatter owns canonical binding-to-display conversion. The sequence helper owns parse/measurement/layout/paint only. It must return rects only if a future interactive keybinding capture explicitly needs them; passive display must never become a tab stop. Keep >4-chip fallback at the caller/presentation policy boundary, because a context menu has different space than Keymap settings.

### Invariants, edge cases, acceptance

Always measure/draw with the same scale/font/fallback; keep modifiers separate and a non-modifier multi-glyph key intact; preserve chord order; ensure width reservation prevents label truncation; clip whole sequence at owner boundary rather than overlapping row label; and do not recolor keycaps as selected-row text (their backgrounds are opaque/pre-blended by overlay convention). At narrow/scale changes, fallback to textual binding before breaking chips across unrelated row content.

Gallery should add `keycap.single`, `keycap.modifiers`, `keycap.function-key`, `keycap.two-step-chord`, `keycap.textual-platform`, `keycap.narrow-fallback`, and selected/hovered parent row. Acceptance: parser tests for all forms; 1x/non-1x measure=paint width; contrast tests; row reservation; and no focus/hit target introduced accidentally.

## Evidence

- Token: [keycap painter/measure](../../src/view/frame.rs), [Chip parser/accessory/layout](../../src/view/overlay_surface.rs), [modal consumers](../../src/view/modal.rs), [Settings consumer](../../src/view/settings_page.rs), [theme contrast tests](../../tests/theme.rs), [gallery catalog](../../src/model/gallery.rs).
- Token inventory: [component inventory](../dev/ui-component-inventory.md).
