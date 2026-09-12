# Search field

## Purpose and boundary

A **search field** edits a query that locates objects, actions, or text. It is a semantic specialization of [Text Field](TEXT-FIELD.md), not merely a text box with a magnifier. Token has two compositions: the docked document Find/Replace bar and overlay headers for command/file/symbol search. Their domains, result models, and effects remain feature-owned.

IntelliJ recommends search fields where objects are hard to find, scope hints instead of a redundant “Search” label, visibly selected option buttons, and Clear that restores default state. [Search Field](https://plugins.jetbrains.com/docs/intellij/search-field.html) is high-confidence primary guidance.

## Current Token contract — high confidence

| Surface        | Owner and behavior                                                                                                                                                                                                                                                                                             |
| -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Find/Replace   | `FindReplaceState` owns query/replacement `EditableState`, focused field, case/word/regex/selection flags, scope and result status. `FindBarLayout` makes shared rectangles for paint, caret, hit test and pointer column mapping. `update/ui.rs` owns open, close, focus, toggles, find/replace and commands. |
| Overlay header | `overlay_surface::Header` carries display text, placeholder, caret, selection and optional scope; palette/picker owner supplies matching/results/activation while the overlay owns measurement and paint.                                                                                                      |
| Text           | `TextFieldRenderer` supplies Code-font text, caret, selection and horizontal scrolling. Surrounding surfaces own chrome, focus and blink.                                                                                                                                                                      |
| Gallery        | `search-field.focused` is a populated overlay header/result specimen; it does not simulate options, Clear, history, IME, keyboard dispatch or Find/Replace.                                                                                                                                                    |

Find Bar controls are Expand, Close, Case, Whole word, Regex, Selection-only, Previous/Next and replacement actions. Its responsive layout moves controls to a second row before sacrificing query width, reserves editor inset only for supported text tabs, and uses `SearchQuery` as the one compiled source for navigation and decoration semantics.

### Current gaps

There is no reusable `SearchField` data/event API, search-icon/clear subcomponent, history, accessibility tree, or interaction gallery fixture. Generic overlay headers are presentation data, not editable event owners. Find option controls use [Button](BUTTON.md) selected state but are not a generic roving/tab-focus group.

## Proposed Token contract — proposed, not implemented

Do not unify Find Bar and overlay search into one state machine. Extract only shared presentation/input routing when a second caller actually needs the same affordances.

```text
SearchFieldModel { editable, placeholder, scope_hint, focused, enabled, invalid: Option<message>, clear_visible, accessory_slots: [SearchAccessory], mode: Instant | ExplicitCommit }
events: Edit | Focus | Clear | Accessory(id) | Submit | Cancel
```

Each owner maps `Edit` to deterministic local work (Find compiles/refreshes matches; a picker filters results), may schedule debounced work through a command, and decides whether Enter submits or activates a result. The component never searches a document or mutates a modal.

### Anatomy and invariant geometry

Order: optional semantic scope/leading search icon; clipped editable text or placeholder; Clear only when non-empty; stable measured trailing option/action slots; optional status outside the editable rectangle. Every action slot is a Button/IconButton-like subcomponent with own hit rect, focus and accessible name. Clear goes before options so options do not jump.

Use the owner rectangle and `TextFieldOptions` for text/caret/pointer projection. Find Bar already provides the invariant: `FindBarLayout` drives drawing, pointer hit testing and caret services. Overlay headers use `overlay_surface::layout_measured`; their field, result list, clip and anchor must remain one plan. Above a list/tree, align to content width; under narrow pressure retain usable field width before compressing accessories.

Use UI font for labels/status/actions and existing Code role for current editable query text. Find chrome currently uses `overlay.input_background`, `foreground`, `highlight`, `selection_background`, tab border and `button.focus_ring`; overlay headers resolve the overlay palette. New color tokens are not justified. Selected option, focus and invalid states must distinguish at all themes/scales.

### Events, keyboard, pointer, focus, accessibility

- `Ctrl/Cmd+F` opens/focuses Find; Escape closes it and restores editor focus. Find Bar Enter goes next; Tab toggles query/replacement only in replace mode. Preserve feature shortcuts.
- Pointer sets caret; drag extends selection; double/triple clicks select word/all. Modal state blocks Find input; special tabs hide it.
- Clear resets only query-owned state/result presentation. Option toggles change semantic flags and remain focusable in an extracted component.
- Popup search Escape remains the popup's existing dismissal path. A future history menu is separate from query syntax (IntelliJ documents `Alt+Down`).
- Proposed semantics: `searchbox`, name from scope/context not redundant literal label, value/invalid message, and names plus pressed state for accessories. Token does not expose this platform tree yet.

## Consumers, gallery, acceptance

Keep Find/Replace and overlay searching as different production families. A visible list filter becomes a consumer only if it shares chrome and event rules rather than just accepting text. On extraction, gallery must cover empty scope hint, focused selection, populated/Clear, each enabled option, no-result/invalid-regex status, narrow wrapped Find Bar, replace mode, and scaled clipped overlay header. Existing `search-field.focused` covers only one state.

Acceptance: field/caret/pointer/paint share geometry; options/Clear never overlap text; Clear, Close and Cancel have distinct effects; special tabs never run document search; search owner remains sole source of query semantics; and tests cover focus, dismissal, selection, toggles and narrow geometry.

## Evidence

- Token: [Find model](../../src/model/ui.rs), [Find layout/render](../../src/view/find_bar.rs), [UI update](../../src/update/ui.rs), [field renderer](../../src/view/text_field.rs), [hit test](../../src/view/hit_test.rs), [search engine](../../src/search.rs), [gallery catalog](../../src/model/gallery.rs), [Find tests](../../tests/find_bar.rs).
- Primary: [Search Field](https://plugins.jetbrains.com/docs/intellij/search-field.html), [Input Field](https://plugins.jetbrains.com/docs/intellij/input-field.html), [Components](https://plugins.jetbrains.com/docs/intellij/components.html).
