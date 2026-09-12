# Label

## Purpose and catalog mapping

A **Label** is non-action text that identifies a control, value, section, status, or explanation. It is not an editable Text Field, a Link, a Badge, or arbitrary unmeasured `draw` text. This document maps the IntelliJ text family without inventing files/types for each visual: control label, field label, metadata/detail label, inline help, description text, and section label are Label roles; Link is an inline action subrange described in [LINK.md](LINK.md).

IntelliJ says input labels should be short/descriptive, sentence-cased nouns (colon for noun labels), positioned left/above, and disabled with their field; placeholders do not substitute for labels. [Input Field](https://plugins.jetbrains.com/docs/intellij/input-field.html) is primary guidance. Token must apply its writing rules selectively: overlay section headings and Group Header title have distinct casing roles.

## Current Token contract — high confidence

There is **no generic Label type or painter**. `TextPainter` plus `FontRole::{Ui,Code}`, sized drawing, truncation and clipping are shared primitives. Callers choose text, color, bounds, baseline and role.

| Role/consumer        | Current owner and behavior                                                                                                                                                  |
| -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Form field label     | `overlay_surface::Field { label, trailing, trailing_is_error }` lays label and trailing validation/status; Settings field/text chrome is composed by `settings_page`/modal. |
| List metadata/detail | `overlay_surface::Row` draws label, dim detail, matched runs and optional code chip; truncation prioritizes detail before primary label.                                    |
| Section label        | Overlay `Section.title` renders uppercase dim, non-selectable metadata. Settings headings are feature-local.                                                                |
| Find placeholder     | Find Bar paints `Find`/`Replace` only when empty and unfocused, from its field geometry. Overlay Header has its own placeholder data.                                       |
| Help/description     | Settings has field descriptions/validation composition but no standalone inline-help or multiline description-text component.                                               |
| Gallery              | Form validation, field, search, rows and tabs exercise text roles indirectly; no label-specific states/semantics specimen.                                                  |

`TextPainter::with_font` restores the prior role; UI rendering normally begins in UI role, while editable text is explicitly Code. This is important: a Label contract selects a font role and measures with the same painter that draws it.

## Proposed Token contract — proposed, not implemented

Do not add a universal retained `Label` widget. Extract a small presentation specification only where a second caller needs the same layout/hit/accessibility metadata:

```text
Label { text, role: Control | Section | Metadata | Help | Description | Status, font: Ui | Code, emphasis: Normal | Strong | Dim, truncation: End | Start | Wrap, enabled, inline_actions: [LinkRange] }
LabelLayout { rect, lines, rendered_text, inline_action_rects }
```

Owner supplies localized/semantic text and state; Form supplies label-control association; renderer measures/wraps/clips/draws and returns ranges. A Label has no activation except its explicit [Link](LINK.md) subranges, and it does not own validation, navigation, or focus.

### Inline help and description text

These are related but not interchangeable.

| Role             | Proposed Token contract                                                                                                                                                                                                                         | IntelliJ evidence                                                                                                                                             |
| ---------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Inline help      | Brief context for one control/group; 1–5-word help may sit to the right when space and label are short, otherwise below; no repeated setting name; optional Link at end. It participates in Form flow but is not the control's accessible name. | [Inline Help Text](https://plugins.jetbrains.com/docs/intellij/inline-help-text.html) limits ordinary help to five lines/70 chars and specifies placement.    |
| Description text | Explanatory prose for a group or selected list/tree item; wraps at readable measure, can contain code/links/bullets, has insets but no card border. It is its own content region, not gray placeholder/help tooltip.                            | [Description Text](https://plugins.jetbrains.com/docs/intellij/description-text.html) recommends default text, readable width, no border and separate insets. |

Until a real Settings consumer needs either, keep existing feature composition. Placeholder is never Label/help: it disappears after editing and cannot carry durable semantics.

### Geometry, font, theme, focus, accessibility

Measure UTF-8/glyphs with `TextPainter`; never use byte/character counts for field alignment. Clip one-line roles inside exact owner bounds. Wrapped Description computes line plan and returns height before downstream rows, uses shared viewport/clip when overflowing, and caps/help text by content policy rather than silently truncating legal/error text. Form label and control share an association even when responsive layout moves label above; disabled control dims its label and help.

UI font is default; Code role is only literal source/config/value snippets. Reuse resolved overlay `text_primary`, `text_secondary`, `text_dim`, `text_bright` and severity text for semantic status; do not encode role solely by opacity or color. Static Labels are not tab stops. Selectable-copy behavior and platform accessibility are unimplemented today; proposed accessible name/description links control label/help/validation, and only inline Link ranges are focusable/actionable.

## Consumers, gallery, acceptance

Existing overlay Form/Row is evidence for primitives, not a generic Label consumer. First extraction candidates are a Settings field label plus inline help and a multi-line group description, provided Form owns the association. Add gallery specimens: left/above responsive field labels; disabled association; end/start ellipsis; matched row label/detail; inline help beside/below; Description wrap/code/link/scroll; section uppercase distinct from Group Header; and mixed UI/Code fonts.

Acceptance: same measured plan paints/hits inline links and drives form height; labels never disappear behind a filled field; text does not overflow clips at scale; errors/status are semantically distinct; disabled association is consistent; descriptions remain readable; and gallery uses production text measurement/drawing.

## Evidence

- Token: [TextPainter/font roles](../../src/view/frame.rs), [overlay fields/rows/sections](../../src/view/overlay_surface.rs), [Find Bar](../../src/view/find_bar.rs), [Settings page](../../src/view/settings_page.rs), [gallery catalog](../../src/model/gallery.rs).
- Primary: [Input Field](https://plugins.jetbrains.com/docs/intellij/input-field.html), [Inline Help Text](https://plugins.jetbrains.com/docs/intellij/inline-help-text.html), [Description Text](https://plugins.jetbrains.com/docs/intellij/description-text.html), [Components](https://plugins.jetbrains.com/docs/intellij/components.html).
