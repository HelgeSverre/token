# Component consolidation and implementation order

This is a research recommendation, not approval to implement every component.
The objective is consistent application controls with trustworthy gallery
coverage, not a second UI framework. Current evidence is the 2026-09-12 source
tree; detailed contracts distinguish existing support from proposed behavior.

## Existing overlap worth consolidating

| Seam       | What already overlaps                                                                                                             | What should remain owned by the feature                                           |
| ---------- | --------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| Button     | Settings, Find and gallery share `render_button`                                                                                  | Action messages, toggle value, enabled policy and effect execution                |
| Field      | Single/multiline text, selection and caret use the shared editable renderer                                                       | Draft validation, submit policy, persistence and search requests                  |
| Select     | Gallery has `SelectState` + measured `SelectLayout`; Settings uses shared low-level geometry but feature-local selection handling | Stable record IDs, committed vs draft value and save semantics                    |
| Choices    | Settings uses `Accessory::Choices`; gallery segmented width selector has explicit semantics                                       | Preference value and navigation policy                                            |
| Overlay    | Menu, picker, completion, documentation and forms share `OverlaySpec`, layout and paint                                           | Request identity, action activation, dismissal policy and content model           |
| Navigation | Settings and gallery share `SectionNavigation` geometry and paint                                                                 | Selected category, filter interpretation and focus routing                        |
| Rows       | Overlay lists and shared tree traversal already establish ordering/visibility                                                     | Domain row payload, expansion storage, open-file or diagnostic actions            |
| Tabs       | Four real painters/layouts already appear in the gallery                                                                          | Document close/save, terminal session lifetime, panel visibility, search category |
| Scrolling  | Shared scrollbar geometry and viewport helpers                                                                                    | Surface-specific units, content extent, scroll-to-selection policy                |

Evidence: [button](../../src/view/button.rs),
[controls](../../src/view/controls.rs), [text field](../../src/view/text_field.rs),
[select state](../../src/model/select.rs), [select view](../../src/view/select.rs),
[Settings](../../src/view/settings_page.rs),
[segmented control](../../src/view/segmented_control.rs),
[overlay surface](../../src/view/overlay_surface.rs),
[section navigation](../../src/view/section_navigation.rs),
[tree traversal](../../src/view/tree_view.rs),
[gallery chrome fixtures](../../src/view/gallery_chrome.rs).

## Recommended slice 1: explicit form-control semantics

**Implemented:** Settings now carries explicit checkbox, select, disclosure and
button presentation through its choice projection. Display labels no longer
select the painter or hit-testing behavior, and the gallery includes a production
Settings composition.

**Original motivation:** the production Settings painter recognized checkbox and
disclosure presentation by literal label arrays (`Off`/`On`, `Show`/`Hide`). That
coupled presentation to wording and made named gallery states less useful for
adjusting the real UI. The typed presentation implemented above removes that
coupling.

**Deliver:** explicit semantic descriptors for boolean choice, disclosure,
fixed-value choice and field validation. Preserve `SettingsForm`, typed setting
changes, draft state and the separate preferences page. Build on the existing
control painters; do not create parallel checkbox, select or editing engines.

**Sequence:**

1. Record current consumers and visual output before changing metadata.
2. Replace label-sensitive presentation selection with typed control intent.
3. Carry enabled/read-only/invalid/focused state explicitly where consumers need
   it; keep selected and pointer-pressed independent.
4. Share a labelled-field/validation layout contract, not a universal form model.
5. Add a real Settings form composition to the gallery alongside focused states.

**Exit criteria:** changing display labels cannot change control kind; paint and
pointer targets use the same rectangles; keyboard traversal and draft Save/Cancel
still work; focus/error help remains legible at narrow widths, light/dark themes
and display scale changes. Keep tests targeted to those actual contracts.

## Recommended slice 2: documentation and contextual overlays

**Implemented gallery coverage:** completion with documentation, hover
documentation and signature help now render through the production overlay
surface. Existing runtime/update tests remain the interaction authority for
selection changes, stale replies, scrolling and dismissal.

**Original motivation:** documentation was a substantial existing application
surface absent from the original 46-specimen catalog. Its mixed code/UI
typography, anchoring, scrolling and async ownership made real fixtures more
valuable than another generic button variant.

**Deliver:** production hover/documentation and completion-with-documentation
fixtures, short/long/error/loading contexts where supported, signature help and
edge-placement examples. Add named tooltip states separately; a documentation
card is not a large tooltip.

**Exit criteria:** production content/layout produces the fixture; code header,
prose and inline code retain their font roles; scrollbar and clipping agree;
changing selected completion cannot leave mismatched docs. Interaction checks
exercise real owners rather than simulating behavior solely in a static tile.

## Recommended slice 3: navigation, collections and panel content

**Implemented collection slice:** the gallery now includes a deterministic Outline
tree, a populated Problems dock with grouped severities and collapse/selection
states, grouped/loading/empty Search Everywhere collections, and selected/empty
Settings record collections. Each fixture enters through its production model,
spec builder and painter. The actual Usages dock remains a distinct future fixture;
Search Everywhere covers the grouped search-result portion of this slice.

**Deliver:** explorer/outline tree rows, grouped Problems/Usages rows, actual
Settings record list, search/picker empty and loading states, and panel toolbar
composition. Reuse traversal/row geometry while keeping distinct domain models.

**Exit criteria:** expanded/selected/focused/hovered are named independently;
keyboard selection and scroll-to-reveal use the same visible order; long names,
empty collections, removed selected records and narrow accessory layout work.
Do not migrate the CSV editor into a generic settings table as collateral work.

## Recommended slice 4: editor composition specimens

**Deliver:** real editor render fixtures for Find, folding, selection/caret,
indent guides, diagnostics and inlays, then terminal selection/link/cursor
specimens. Use existing editor viewport and special-mode rendering paths.

**Exit criteria:** fixtures expose visual-row and hit-test boundaries, not a
gallery-local approximation of a document. Tie back to the
[editor geometry reference](../EDITOR_UI_REFERENCE.md). A special-mode preview
must not enter plain-text fast paths.

## Performance-study adoption: proposed parallel tracks

The 2026-09-16 [component decision record](PROTOTYPE-COMPONENTS.md) adds concrete
consumers for several previously deferred concepts. These specifications do not
change the implementation status of the gallery slices above.

- **Shared pane chrome:** implement title/optional actions and optional
  leading/trailing footer content from [Pane chrome](PANE-CHROME.md). Integrate
  actions alongside existing dock tabs without repeating the active title in a
  second header. Add production-backed gallery states as each helper lands.
- **Performance:** follow the [feature plan](../feature/performance-panel.md)
  for coherent completed-frame data, right-dock content and then generic
  [floating placement](DOCKABLE-PANEL.md). Keep existing instrumentation and
  omit the live/pause/reload row and decorative header icon. No rail is needed.
- **Editor polish:** follow the separate [visual-polish plan](../feature/editor-visual-polish.md)
  for native comparison fixtures, tab/status spacing, measured UI-font roles,
  and separately tested code line pitch. Preserve user-selected fonts and themes.
- **Breadcrumbs:** implement the [editor-group contract](BREADCRUMBS.md) as a
  distinct viewport/navigation integration. Shared styling does not permit a
  feature-local editor inset or a pointer-only navigation control.
- **Activity rails:** [left/right rail semantics](ACTIVITY-RAIL.md) are now
  specified but remain deferred. They are not a prerequisite for docking,
  floating, breadcrumbs, or typography polish.

Performance work can proceed independently of the global typography rollout.
Prototype readouts and plots should first be feature-local compositions of
existing labels, rows and pure plotting helpers; extract more universal
primitives only when another concrete consumer needs the same contract.

## Conditional additions, not prerequisites

- **Combo box:** add when a concrete field accepts both suggestions and custom
  text. Do not rename the existing non-editable select to obtain this feature.
- **Radio group:** useful for a small set needing explanatory labels; do not
  replace every compact segmented control with it.
- **Split button:** defer until an action has a genuine primary/default action
  plus alternatives. Existing plain buttons should stay plain.
- **Notification/banner/progress primitives:** introduce with a real operation
  lifecycle, cancellation/retry and context ownership, not decorative loading.
- **Icon registry and badge roles:** useful once a concrete set of production
  icons can migrate together; explicit fallback/accessible labels must accompany
  icon polish.
- **Onboarding and floating action bars:** later product decisions; the IntelliJ
  catalog alone is not a reason to make Token noisier. Breadcrumbs and dockable
  panels now have the separate user-requested proposals above.

## What not to consolidate

Do not force all tab families through one style/state type, all notifications
through a modal, or all values through buttons. Do not replace Settings with a
palette. Do not move file/network/PTY effects into components. Do not recreate
OS dialogs or webview content as fake native gallery controls. Shared visual
language requires shared contracts where semantics overlap, not one enormous
enum that owns the whole application.
