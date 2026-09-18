# Token UI component reference

Implementation and state-modeling reference for the 2026-09-13 codebase, with
separately marked prototype-adoption proposals added on 2026-09-16.
The component chapters follow the
[technical standard](TECHNICAL-STANDARD.md): concrete representations and ownership,
event transitions, layout/index algorithms, worked traces, invalidation, runtime
integration, and verification cases. These documents distinguish current
Token implementations from proposed contracts; a documented component is not
automatically an implemented widget or an approved feature.

## Reading the catalog

Start with [Foundations](FOUNDATIONS.md), then the
[implementation roadmap](IMPLEMENTATION-ROADMAP.md). The catalog is organized by
semantic responsibility, not by whichever painter happens to draw a rectangle.
Token retains its model → update → command → render architecture, and its native
CPU renderer. IntelliJ is a vocabulary and interaction reference, not a framework
dependency or a mandate to copy its appearance.

The [native gallery](../dev/ui-gallery.md) currently has 56 static specimens.
Its live shell supports filtering, category navigation, theme selection, width
selection and scrolling. Static visual coverage is not proof of production
keyboard handling, accessibility, focus restoration, or asynchronous lifecycle.

Each of the 41 component chapters starts with a Default Dark visual study under
review. Its emphasised image preserves the component and fades everything else
to 25% by default; links and the viewer toggle expose the normal composition
as well. The viewer also provides adjustable context opacity and optional
offset inspection guides, which are hidden in the default renders.
Browse the [HTML mockup gallery](mockups/index.html), then open any specimen at
its fixed 1200 × 760 logical-pixel size. These use the Performance prototype's
typography and spacing. They are draft implementation targets with existing and
proposed states distinguished in the chapter, not an approved fidelity baseline.
The next design pass should refine a small set of existing primitives before
carrying their treatment through the catalog. The
[mockup style guide](mockups/STYLE-GUIDE.md) defines context, palette, fonts, and
state presentation, and the [renderer guide](mockups/README.md) explains how to
regenerate both 2400 × 1520 PNG variants and their chapter references.

## Performance-study proposals

Start with the [prototype component decisions](PROTOTYPE-COMPONENTS.md) for the
new vocabulary, what should remain an existing component, and implementation
order. The proposed contracts are [BreadcrumbBar](BREADCRUMBS.md),
[PaneHeader / PaneFooter](PANE-CHROME.md), [DockablePanel](DOCKABLE-PANEL.md),
and [ActivityRail](ACTIVITY-RAIL.md). Activity rails remain deferred, including
both left and right placements.

The [editor visual-polish plan](../feature/editor-visual-polish.md) turns the
font, spacing, tab, and status-bar findings into controlled native experiments.
The separate [performance-panel plan](../feature/performance-panel.md) defines
the right-docked and in-window floating feature, without the prototype's
live/pause/reload row or decorative header icon. These are plans, not new native
gallery specimens or implemented controls.

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
| Proposed pane and shell concepts | [Breadcrumb bar](BREADCRUMBS.md), [Pane chrome / footer](PANE-CHROME.md), [Dockable panel](DOCKABLE-PANEL.md), [Activity rail — deferred](ACTIVITY-RAIL.md) |

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
- **Dockable panel** adds an in-window floating placement to the same panel
  identity. **PaneHeader** and **PaneFooter** are optional chrome composition
  helpers; they do not own docking, feature data, or global status messages.
- **Breadcrumb bar** locates one editor group's document/symbol; **activity
  rail** provides panel access at a workspace edge. Both remain proposed.
- **Scroll area** owns the viewport contract; **scrollbar** is its optional
  position indicator and manipulation control.

These distinctions align with the official [IntelliJ component catalog](https://plugins.jetbrains.com/docs/intellij/components.html)
and [UI overview](https://plugins.jetbrains.com/docs/intellij/ui-overview.html).
The [full crosswalk](INTELLIJ-CROSSWALK.md) accounts for every entry in that
component index and relevant application surfaces, including intentionally
deferred variants. The archived [research brief](../archived/ui-RESEARCH-BRIEF.md) is the historical charter recording the questions,
sources, confidence rules and division of work.

The research brief records the user-supplied local research snapshot's provenance;
that snapshot is no longer present in the worktree. Official JetBrains links are the
durable external references; Token implementation links point into this repository.

## What exists, and what is missing

| Capability            | IntelliJ reference                                              | Token now                                                                  | Recommendation                                                    |
| --------------------- | --------------------------------------------------------------- | -------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| Named actions/values  | Distinct buttons, checkbox, dropdown, combo and radio semantics | Shared painters and typed Settings choice presentation                     | Extend explicit descriptors only for concrete consumers           |
| Input/forms           | Bound form controls with validation                             | Shared editing, typed Settings drafts, feature-owned validation            | Consolidate field presentation and semantic states                |
| Popup composition     | Menus, completion and documentation in lightweight surfaces     | Substantial shared `OverlaySpec`/layout/paint with feature-owned lifecycle | Reuse it; add missing documentation/gallery compositions          |
| Persistent navigation | Tool windows, tabs and toolbars                                 | Dock model, four tab families, shared section navigation                   | Share subparts; retain domain selection and effect owners         |
| Scrolling/layout      | Scroll containers and component guidelines                      | Existing pixel/editor and row/list geometry helpers                        | Keep coordinate units and one geometry authority explicit         |
| Theme/typography      | Semantic themed components                                      | Resolved theme families and scoped Code/UI painters                        | Add only necessary roles with compatibility fallbacks             |
| Accessibility         | Semantic and keyboard guidance                                  | Some keyboard paths; no general platform accessibility tree                | Specify roles/focus/announcements as gaps, not implemented claims |
| Gallery               | Reference illustrations/sample components                       | 56 static production-painter specimens plus an interactive shell           | Add more production collections and scoped interaction verification |

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
5. **Styling/accessibility:** current editable text, explorer and
   document/dock/terminal tab text use Code; overlay tabs and section navigation
   use UI typography. The [visual-polish plan](../feature/editor-visual-polish.md)
   explicitly proposes a later measured UI-font trial for non-editable chrome;
   it does not change the current source-text/input contract. Coordinate and semantic
   accessibility contracts are mandatory for new work, not retroactive claims.
6. **Gallery:** named states must render production helpers. Static tiles prove
   appearance only; focus, cancellation and async identity require real owners.
7. **Next additions:** explicit form-control metadata, documentation overlays and
   the first tree/list/panel compositions are implemented. Real editor
   compositions are next. New combo/split controls wait for concrete consumers.

## Implementation boundaries

This reference does not approve every proposed widget. Preserve Settings as its
separate preferences page. Keep `Renderer` as orchestrator. Do not invent a
retained widget hierarchy, move I/O into painters, or duplicate editor visual-row
loops. Native OS dialogs and webview content remain external rendering surfaces.

Use the [editor reference](../EDITOR_UI_REFERENCE.md) for the extensive editor
geometry treatment; `EDITOR-SURFACE.md` maps that vocabulary to current code
rather than copying its mathematics into a competing source of truth.
