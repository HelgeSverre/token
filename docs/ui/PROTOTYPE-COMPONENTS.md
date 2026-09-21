# Performance study: component decisions and implementation tracks

**Decision record, 2026-09-16.** The user selected the performance study as a
direction for native editor polish and requested component specifications before
implementation. This inventory connects the [prototype](../../prototypes/debug-performance.html),
[fidelity findings](../../prototypes/debug-performance-fidelity.md), existing
component reference, and two implementation plans. It does not describe new
native components as already shipped.

The only immediate visual change in this documentation pass is the prototype's
subtle breadcrumb bottom border. Native breadcrumbs, pane footers, floating
panels, font changes, and activity rails remain proposed. Rails are explicitly
deferred.

## What the unfamiliar strips mean

Use **BreadcrumbBar** for the path/symbol navigation below one editor group's
document tabs. Its scope is that group's active document, including when another
group has keyboard focus. It has its own bottom separator and reserved height.
It is not dock navigation and should not be called a Docker component.

Use **PaneFooter** for an optional fixed band at the bottom of one content
pane. It has leading and trailing content slots; text, an optional icon or
status dot, and compact metadata can be arranged without each feature deriving
its own geometry. A hint wrapper supplies an optional icon and explanatory text;
a status wrapper supplies named state and optional trailing detail. Footer
content stays owned by the feature. A pane with no useful footer content reserves
no footer height.

The sentence “Keep the document in view. Inspect rendering alongside it.” is
design explanation in the mockup. It describes why docking is useful, rather
than a native command or a permanent message to add to every document. Real
footer examples include the scope of an inspection, the mode of a preview, or
the number of captured render samples. A temporary navigation hint is useful
only while its associated interaction is available. The synthetic-data label
and seed belong to the demonstration and are not native performance metrics.

A footer is distinct from the application-wide **StatusBar** and from an
actionable **Notification**. A warning that needs attention retains its warning
semantics; it should not be hidden in a quiet metadata strip.

## Component inventory

| Prototype area | Canonical contract | Decision and owner |
| --- | --- | --- |
| Path/symbol strip above source | [BreadcrumbBar](BREADCRUMBS.md) | New optional editor-group navigation. Reuse document/symbol identity and the existing group viewport/layout authority. |
| Performance title and optional buttons | [PaneHeader / PaneChrome](PANE-CHROME.md) | New shared chrome composition. Title is supplied by the panel; icons and feature actions are optional. Reuse button and toolbar/menu behavior. |
| Editor hint strip and compact performance footer | [PaneFooter](PANE-CHROME.md) | New optional leading/trailing passive-content composition, with small hint/status adapters. No second global status model. |
| The same panel docked or floating | [DockablePanel](DOCKABLE-PANEL.md) | New placement/lifecycle support around one domain owner. Performance is the first eligible consumer; floating stays inside Token's window. |
| Thin icon strip at the workspace edge | [ActivityRail](ACTIVITY-RAIL.md) | New deferred panel-access navigation. Left, right, or both edges may be configured; never duplicate a panel's selection/placement state. |
| Taller document tabs and active treatment | [Document tabs](TABS.md) | Polish the existing family. Keep stable TabId, close/save semantics, overflow, drag, and reveal behavior. |
| Taller application-wide lower bar | [StatusBar](STATUS-BAR.md) | Polish existing height, alignment, and overflow. It remains separate from PaneFooter; the prototype's F2 button does not automatically authorize making native segments interactive. |
| Frame/cache number, unit, and secondary description | [Label](LABEL.md) and [GroupHeader](GROUP-HEADER.md) composition | Feature-local metric readout initially. Align numeric values consistently; no general dashboard/card framework is required. |
| Frame history and miniature stage histories | Performance time-series helpers | Start inside the [performance feature](../feature/performance-panel.md). Share pure plot geometry among its charts; promote a general TimeSeriesPlot only with another real consumer. |
| Stacked stage contribution strip | Performance accounting visualization | Feature-specific composition. This is not Progress: it does not represent completed/total work. Stage sums and measured frame boundaries must agree. |
| Small dot plus a state label | [Icon](ICON.md), [Label](LABEL.md), optional [Badge](BADGE.md) | Reuse a labelled status accessory. A dot conveys no state without accompanying text; a count badge and a chart-series key have different meanings. |
| Section dividers, headings, and small metadata | [GroupHeader](GROUP-HEADER.md), pane/section layout | Reuse spacing and semantic border/text roles. A border alone does not justify a new component. |
| Expand the remaining stage rows | Group disclosure / [List](LIST.md) composition | Optional later interaction with stable stage IDs. Reuse the existing disclosure behavior instead of making a new button family. |
| History-range choices | [SegmentedControl](SEGMENTED-CONTROL.md) | Reuse when actual selectable history semantics exist. Do not label render-count samples as elapsed seconds. |
| Chart inspection tooltip | [Tooltip](TOOLTIP.md) or owner-specific inspection overlay | Optional later chart interaction. It needs a real sampled-point identity and keyboard alternative before becoming a native feature. |
| Panel resize boundary | [Splitter](SPLITTER.md) / floating resize handles | Reuse capture/cancel and shared solved geometry; dock and floating resize have distinct owners and constraints. |
| Rounded floating surface and shadow | Existing frame shapes + DockablePanel | Styling of a placement host, not a new modal, popup manager, or generic Card state machine. |
| Live/pause/reload row and decorative performance icon | Prototype controls | Excluded from the initial native performance plan. Generic host Float/Dock/Close commands are separate. |
| Workload, seed, budget, theme, and typography lab controls | Prototype-only evaluation harness | Preserve the useful design experiment; these controls are not a native performance toolbar. Native themes/preferences retain their existing owners. |
| Fake application titlebar and window buttons | Operating-system chrome reference | Do not copy HTML traffic lights into Token's CPU component system. |

## Composition and reuse boundaries

The editor and tool panels share selected chrome helpers, not a universal pane
state machine:

```text
Application shell
  Optional left ActivityRail                         [deferred]
  Existing left dock / EditorArea / right dock
    Each editor group
      Existing DocumentTabBar
      Optional BreadcrumbBar
      Existing editor viewport / specialized tab content
      Optional PaneFooter
    Each dock
      Existing DockTabBar + active panel's optional actions
      Active panel body
      Optional PaneFooter
  Optional right ActivityRail                        [deferred]
  Existing bottom dock
  Existing global StatusBar

Floating layer, inside the same window
  PaneHeader(title, optional icon, optional actions)
  The same panel body
  Optional PaneFooter
```

The dock header already identifies its active panel. Do not stack another
identical title row directly beneath it. Shared header layout may reserve a
trailing action group beside dock tabs; the floating host uses the title form.
Document, dock, terminal, and overlay tabs keep their distinct behavior.

The native equivalents use the existing Clay-inspired `UiTree` and
`LayoutSnapshot` where they own geometry. Extend the existing editor-group and
viewport calculations for breadcrumb/footer height; do not subtract those
heights independently in paint and input code. Header actions, body clips,
splitters, and floating hit order must consume the same solved rectangles.

## Work tracks and dependency order

Two implementation plans can progress independently after these contracts are
reviewed:

1. [Editor visual polish](../feature/editor-visual-polish.md): establish native
   comparison fixtures; refine existing tab/status spacing; trial measured UI
   typography; test source line pitch separately; then adopt optional context
   chrome. Preserve user-selected source size and all theme palettes.
2. [Performance panel](../feature/performance-panel.md): define one completed
   frame projection from the existing collector; share pane chrome; implement
   the right-dock body; add generic floating placement; validate actual sampling
   and observer cost. This does not wait for a global editor typography rollout.

The first reusable implementation slice should be pane header/footer geometry
and their production-backed gallery specimens. Existing-surface typography and
spacing experiments can run alongside it. Breadcrumb navigation is a separate
editor integration because it changes viewport origin and needs document/symbol
identity. Activity rails do not block either track.

"Pane header/footer" names one proposed primitive family, not one atomic slice:
[editor-visual-polish.md](../feature/editor-visual-polish.md) ships `PaneFooter`
in Phase 4 (with breadcrumbs) and `PaneHeader` in Phase 5 (with the dockable
panel), since a footer has no docked-vs-floating title-composition rule to
resolve first.

## Gallery and verification

The [native gallery](../dev/ui-gallery.md) remains the place to compare named
visual states in actual Token fonts and themes. Add specimens only when they
call production helpers. The names below are planned families, not claims about
the current specimen catalog:

| Family | Useful first states |
| --- | --- |
| `pane-header.*` | title-only, optional icon, actions, long title, overflow, narrow, focused action |
| `pane-footer.*` | absent, hint without icon, hint with icon, status/dot plus trailing detail, clipped long content |
| `breadcrumbs.*` | file only, directory/file/symbol, optional icons, overflow, focused item, two groups |
| `dockable-panel.*` | docked, floating, title-only, actions overflow, small window, overlapping editor |
| `performance.*` | empty, partially filled history, populated, narrow, zero lookups, accounting mismatch |
| `activity-rail.*` | left, right, both edges, selected/focused, overflow; deferred until implementation |

Static gallery tiles verify appearance and clipping. Reducer/layout/runtime
checks verify navigation, active-document changes, float/dock/close/reopen,
focus return, pointer cancellation, and stale data. Native screenshot fixtures
must preserve font sizes, source, theme, scale, and window/dock dimensions for
meaningful comparisons. See the component contracts and feature plans for
numeric traces and acceptance cases.
