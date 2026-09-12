# Token UI component reference

Research and specification reference, reviewed against the 2026-09-12 codebase. These documents distinguish current
Token implementations from proposed contracts; a documented component is not
automatically an implemented widget or an approved feature.

## Reading the catalog

Start with [Foundations](FOUNDATIONS.md), then the
[implementation roadmap](IMPLEMENTATION-ROADMAP.md). The catalog is organized by
semantic responsibility, not by whichever painter happens to draw a rectangle.
Token retains its model → update → command → render architecture, and its native
CPU renderer. IntelliJ is a vocabulary and interaction reference, not a framework
dependency or a mandate to copy its appearance.

The [native gallery](../dev/ui-gallery.md) currently has 46 static specimens.
Its live shell supports filtering, category navigation, theme selection, width
selection and scrolling. Static visual coverage is not proof of production
keyboard handling, accessibility, focus restoration, or asynchronous lifecycle.

## Component contracts

| Group                      | Contracts                                                                                                                                                                                                                                                    |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Foundations                | [State, layout, themes, typography and accessibility](FOUNDATIONS.md)                                                                                                                                                                                        |
| Actions and values         | [Button](BUTTON.md), [Checkbox](CHECKBOX.md), [Radio group](RADIO-GROUP.md), [Toggle switch](TOGGLE-SWITCH.md), [Select](SELECT.md), [Segmented control](SEGMENTED-CONTROL.md), [Split button](SPLIT-BUTTON.md)                                              |
| Text and forms             | [Text field / text area](TEXT-FIELD.md), [Search field](SEARCH-FIELD.md), [Combo box](COMBOBOX.md), [Form](FORM.md), [Group header / disclosure](GROUP-HEADER.md), [Label / help text](LABEL.md), [Link](LINK.md)                                            |
| Navigation and collections | [Section navigation](SECTION-NAVIGATION.md), [Tabs](TABS.md), [List](LIST.md), [Tree](TREE.md), [Table](TABLE.md), [Menu](MENU.md), [Toolbar](TOOLBAR.md)                                                                                                    |
| Surfaces and feedback      | [Popup](POPUP.md), [Dialog](DIALOG.md), [Documentation card](DOCUMENTATION-CARD.md), [Tooltip](TOOLTIP.md), [Panel](PANEL.md), [Notification / banner](NOTIFICATION.md), [Status bar](STATUS-BAR.md), [Empty state](EMPTY-STATE.md), [Progress](PROGRESS.md) |
| Supporting visuals         | [Icon](ICON.md), [Badge](BADGE.md), [Keycap](KEYCAP.md)                                                                                                                                                                                                      |
| Viewports and editor       | [Scroll area / scrollbar](SCROLL-AREA.md), [Splitter](SPLITTER.md), [Editor surface / gutter / decorations](EDITOR-SURFACE.md)                                                                                                                               |

## Naming decisions

- **Button** invokes an action; **toggle** retains a boolean state. Pointer-down
  is not the same state as selected.
- **Select** chooses an existing value; **combo box** also permits editable
  text. Token's existing `SelectState` is explicitly non-editable.
- **Segmented control** exposes a small value set; **tabs** navigate content.
  Similar borders do not make their keyboard or ownership contracts identical.
- **Section navigation** changes the visible settings/gallery category; it is
  neither a command palette nor an editable collection.
- **Popup** describes placement/lifecycle. Menus, completion, and documentation
  have distinct content and activation rules within that surface.
- **Panel** is Token's term for persistent dock content; IntelliJ calls this a
  tool window. Do not add a second concept named ToolWindow for the same thing.
- **Scroll area** owns the viewport contract; **scrollbar** is its optional
  position indicator and manipulation control.

These distinctions align with the official [IntelliJ component catalog](https://plugins.jetbrains.com/docs/intellij/components.html)
and [UI overview](https://plugins.jetbrains.com/docs/intellij/ui-overview.html).
The [full crosswalk](INTELLIJ-CROSSWALK.md) accounts for every entry in that
component index and relevant application surfaces, including intentionally
deferred variants. The [research brief](RESEARCH-BRIEF.md) records the questions,
sources, confidence rules and division of work.

Links into `temporary-docs/` identify the user-supplied local research snapshot,
which is not distributed with this catalog. Official JetBrains links are the
durable external references; Token implementation links point into this repository.

## What exists, and what is missing

| Capability            | IntelliJ reference                                              | Token now                                                                  | Recommendation                                                    |
| --------------------- | --------------------------------------------------------------- | -------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| Named actions/values  | Distinct buttons, checkbox, dropdown, combo and radio semantics | Shared painters; Settings still identifies some kinds from display labels  | Explicit control descriptors, not a new framework                 |
| Input/forms           | Bound form controls with validation                             | Shared editing, typed Settings drafts, feature-owned validation            | Consolidate field presentation and semantic states                |
| Popup composition     | Menus, completion and documentation in lightweight surfaces     | Substantial shared `OverlaySpec`/layout/paint with feature-owned lifecycle | Reuse it; add missing documentation/gallery compositions          |
| Persistent navigation | Tool windows, tabs and toolbars                                 | Dock model, four tab families, shared section navigation                   | Share subparts; retain domain selection and effect owners         |
| Scrolling/layout      | Scroll containers and component guidelines                      | Existing pixel/editor and row/list geometry helpers                        | Keep coordinate units and one geometry authority explicit         |
| Theme/typography      | Semantic themed components                                      | Resolved theme families and scoped Code/UI painters                        | Add only necessary roles with compatibility fallbacks             |
| Accessibility         | Semantic and keyboard guidance                                  | Some keyboard paths; no general platform accessibility tree                | Specify roles/focus/announcements as gaps, not implemented claims |
| Gallery               | Reference illustrations/sample components                       | 46 static production-painter specimens plus an interactive shell           | Add production compositions and scoped interaction verification   |

The IntelliJ column describes reference guidance, not a source-code audit of its
entire toolkit. Token claims link to implementation evidence in the family docs;
the recommendations are proposed synthesis.

## Answers to the design questions

1. **Names:** use action/value/navigation/surface distinctions above. Variants
   share a family only when their ownership and interaction contracts agree.
2. **Existing reuse:** Button, field editing, overlay layout, scrollbar geometry,
   section navigation and tree traversal are the strongest seams. The roadmap
   identifies remaining feature-local overlap and each owner to preserve.
3. **Behavior:** component contracts separate immutable presentation, transient
   interaction and committed domain state; effects remain commands/runtime.
4. **Subcomponents:** share typography, accessories, geometry and surface paint;
   do not merge editor, terminal, dock and overlay tab behavior.
5. **Styling/accessibility:** use current theme roles and explicit font scopes;
   all editable text, explorer and document/dock/terminal tab text retain Code;
   overlay tabs and section navigation use UI typography. Coordinate and semantic
   accessibility contracts are mandatory for new work, not retroactive claims.
6. **Gallery:** named states must render production helpers. Static tiles prove
   appearance only; focus, cancellation and async identity require real owners.
7. **Next additions:** explicit form-control metadata first; documentation and
   completion cards next; then trees/collections/panel content and real editor
   compositions. New combo/split controls wait for concrete consumers.

## Implementation boundaries

This reference does not approve every proposed widget. Preserve Settings as its
separate preferences page. Keep `Renderer` as orchestrator. Do not invent a
retained widget hierarchy, move I/O into painters, or duplicate editor visual-row
loops. Native OS dialogs and webview content remain external rendering surfaces.

Use the [editor reference](../EDITOR_UI_REFERENCE.md) for the extensive editor
geometry treatment; `EDITOR-SURFACE.md` maps that vocabulary to current code
rather than copying its mathematics into a competing source of truth.
