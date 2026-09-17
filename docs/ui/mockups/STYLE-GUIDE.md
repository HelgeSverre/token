# Component mockup style guide

These specimens are intended to become implementation references: precise visual
targets for a component's shape, typography, spacing, states, and composition.
The current catalog is a set of drafts under review, not an approved fidelity
baseline. A deterministic screenshot establishes reproducibility, not design
quality. Read the component's Markdown chapter before choosing contents and states.

The component is the subject. Surrounding UI only explains its placement, scale,
or relationship to another element. Use the Performance prototype's typography
and Token's Default Dark palette to improve existing contracts; do not invent
features to make a scene feel complete. Better alignment, text hierarchy,
spacing, separators, and state treatment can improve on today's editor without
changing what the component does.

Established surfaces, proposed components, and deferred concepts must retain
their separate status. A polished proposal is not evidence that Token implements
it, and a familiar-looking IDE workflow is not permission to add that workflow.

## One capture frame

- Each component has an uppercase HTML basename matching its chapter, for example
  BUTTON.html and BUTTON.md. All files live in docs/ui/mockups; renders are in
  its renders subdirectory. Administrative chapters do not need component images.
- The capture root is #specimen, exactly 1200 by 760 CSS pixels at the page origin.
  Chromium captures at device scale 2, producing a 2400 by 1520 PNG for every page.
  Never stretch a component or magnify its font to fill that frame.
- Use 32px outer padding, a compact name/context header, a 24px gap, the specimen
  body, and a small provenance footer. The surrounding frame is documentation
  chrome; the actual component sits inside the body.
- All important content must fit. Deliberately scrollable/clipped examples use
  data-clip-demo on their viewport; the full screenshot itself must not scroll.
  Do not use overflow:hidden on the entire frame to disguise accidental clipping.

## Two views of one source

Every component has one HTML source and two generated PNGs:

| View | HTML query | Render |
| --- | --- | --- |
| Emphasised (default) | `?view=emphasised` | `renders/COMPONENT-emphasised.png` |
| Normal | `?view=normal` | `renders/COMPONENT.png` |

The page's Normal / Emphasised toggle changes only the documentation view.
`?capture=1` hides this viewer control during rendering. The chapter leads with
the emphasised image and links to both views; the gallery can show either set.
Both use the same capture frame, content, fonts, and component geometry.

In the emphasised view the component keeps its normal appearance and everything
else is composited at **25% opacity over the specimen background by default**. This includes
surrounding UI, annotations, and documentation chrome. Existing disabled states
remain disabled; emphasis must not brighten them into enabled states. Use the
normal view to assess the actual contrast and hierarchy of a complete scene.
The dimmed context is a documentation aid, never an application state or a theme
token proposal.

The **Context** slider adjusts remaining context opacity from 0–100%; lower
values dim the surroundings more. **Outline** toggles an optional inspection
guide. Both controls are disabled in Normal view and excluded from captures.
Their settings can be shared through the URL, for example
`?view=emphasised&context=20&outlines=1`.

The inspection guide is a neutral dashed stroke, **8px clear of the subject's
visible edge**, including any existing focus outline. It lives in the viewer
layer, never in the component's border, outline, or layout. A second mask
protects every subject and its clearance, so guides cannot cross neighboring
subjects. Guides are off by default for both browsing and generated images.
The component's own focus styling remains part of its documented state; browser
comment markers are separate from the HTML reference.

Configure a page or all captures through CSS (the defaults live in shared.css):

```css
#specimen {
  --mockup-context-opacity: .25; /* remaining opacity, from 0 to 1 */
  --mockup-outline-display: none; /* block enables inspection guides */
  --mockup-outline-offset: 8px; /* clear gap; minimum 4px */
  --mockup-outline-width: 1px;
  --mockup-outline-color: var(--faint);
}
```

Viewer controls and URL parameters override these defaults for that browser
view. To suppress guides even when a URL or viewer control enables them:

```css
.mockup-emphasis-outlines { display: none !important; }
```

Regeneration uses the page's CSS settings, independent of a previously adjusted
browser tab. No outline is included unless the source CSS opts into it. Changing
these settings must preserve normal captures and the subject's own pixels.

`subjects.js` lists the subject selectors for every catalog entry. Selectors are
scoped to the specimen body and must match visible elements. Choose the smallest
elements that express the component and include its state comparisons:

- Badge, keycap, checkbox, radio, or icon: the mark or control itself. Its label,
  row, field, menu, and tool window are context unless part of that contract.
- Scroll area: the scrollbar track and thumb; its source rows establish extent.
- Breadcrumbs or tabs: the complete bar or strip, including its owned items.
- Dialog, popup, panel, or editor surface: the component's complete surface;
  a host, anchor, adjacent pane, and external notes remain context.
- Foundations: the typography, color, spacing, and state specimens themselves.

Do not select a whole Settings page to emphasise its checkbox, or an annotation
column to emphasise the component it describes. Adding or replacing a state
requires reviewing its subject mapping as well as the normal composition.

`emphasis.js` applies a non-interactive SVG veil with holes measured from
the subject elements. It does not clone content, reparent elements, or apply
opacity to nested containers. This preserves layout and avoids dimming a child
twice. Existing focus outlines are included in the undimmed area; optional
inspection guides are rendered separately outside it.
The renderer rejects missing or empty mappings. Shared loading works from both
local files and the renderer's loopback server.

## Palette and typography

- Load ../../../prototypes/debug-performance-themes.js before shared.js. The
  shared helper selects Default Dark and applies its generated semantic colors.
  That palette comes from themes/dark.yaml and records its source SHA-256.
- Use shared.css tokens for editor, tab, sidebar, overlay, selection, border,
  text, focus, status and syntax colors. Default Dark's blue global status bar
  remains blue; a pane footer inherits its local surface instead.
- Use the repository's Inter-Regular.ttf for UI labels and JetBrainsMono.ttf
  for code, paths and numeric values. Fonts are loaded locally, never from a CDN.
  Both supplied faces are regular; hierarchy uses size, spacing and color.
- The illustration's UI text is 13px with 19px line spacing, supporting labels
  11–12px, code 12.5px with 22px line spacing, and section titles 14px. Keep these
  common sizes unless the component itself defines a different role.
- This deliberately follows the prototype's proportional UI typography and
  roomier chrome. Current native Code-font roles and metrics remain documented
  in the prose. Do not call this styling pixel-identical to current Token.
- Use 1px quiet separators, 3–4px control corners, and restrained surface depth.
  No invented brand gradients, giant rounded cards, colorful emoji, or marketing
  illustrations. Icons use the shared 16px stroke-SVG vocabulary.

## Put the component in a credible context

| Component kind | Minimum useful context |
| --- | --- |
| Button, checkbox, select, field | A label or small form group showing what the control changes, plus labelled state variants. A full preferences page is usually unnecessary. |
| Scrollbar | A cropped viewport with enough rows to establish the clipped extent; show track, thumb, and at least one documented end/hover variant. |
| Breadcrumbs | A short source/gutter excerpt below the path/file/symbol bar. Include tabs only when their relationship matters; show narrow overflow separately. |
| Tabs | The content family they select. Show document, dock, terminal and overlay families separately; preserve their distinct semantics. |
| Splitter | Two populated neighboring panes and their shared boundary; never a detached vertical line. |
| Menu, tooltip, popup, documentation card | A visible trigger or source anchor and enough surrounding content to explain placement. Distinguish a static open-state drawing from working popup behavior. |
| Dialog | A documented decision with a restrained fragment of its host. Keep Settings as a preferences page, not a command palette. |
| Panel / pane chrome | Header, body and optional footer. Titles appear once, icons/actions are optional, footers remain local rather than global status bars. |
| Dockable panel | A real editor behind a floating inspection panel, plus a docked comparison. Same title/body identity; no fake OS titlebar. |
| Activity rail | Both edge placements with adjacent editor/dock content, labelled as deferred; selected/focused are distinct. |
| Status bar | A full-width editor bottom edge and realistic segments. Its height is independent of the text size. |
| Empty/error/progress feedback | The operation or collection that owns it. Use fixed illustrative values and actionable copy only where the documented contract supports actions. |
| Foundations | A compact typography, semantic-color and spacing board; no invented interactive control family. |

Use a dominant component example and a compact state comparison. Crop the
surroundings before adding more UI. State labels belong outside the component.
Favor actual Token labels, paths, source, settings, and documented tool content.
A full application screenshot should be exceptional for a small primitive.
Do not repeat identical cards across chapters with only the title changed.

## Desktop IDE fidelity

The first gallery pass overused large bordered cards, descriptions under every
setting, isolated state tiles, and empty example panels. A credible IDE context
needs the relationships between controls to be visible. Use the following
baseline for new specimens and revisions:

- **Preferences are one flat page.** Keep Token's category navigation. Use
  left-aligned labels beside their fields when space allows; align neighboring
  inputs and indent dependent controls to their parent label. Use 20px between
  groups and 6–8px within a group. A group may have a quiet heading and hairline,
  but it should not become a nested card.
- **Controls stay compact.** Default to 28px fields/buttons, 26px toolbar action
  targets, 16px checkbox/radio/icon drawings, and 25–28px list rows. Text stays
  12–13px; reduce excess surrounding space before shrinking labels. Field width
  follows plausible values, usually 180–260px, rather than filling the page.
- **The window explains the action.** Show an actual category, source file,
  search query, diagnostic, or terminal session when the subject needs it.
  Populate only enough rows to reveal the relevant alignment and hierarchy.
  Do not substitute skeleton bars for source or data.
  Empty space is appropriate when the component's state is actually empty.
- **Tool windows remain part of the editor.** Use a small title/tab row,
  compact grouped actions, a populated body, and optional local footer. Keep
  selection, focus, inactive tabs, and unavailable actions distinct. Existing
  terminal controls and dock hosts remain Token-specific examples.
- **Accessories remain subordinate.** Put counts beside the tool or collection
  they describe, shortcut keycaps in a reserved trailing column, and icons beside
  their actual targets. Use severity color for severity and selection color for
  selection; avoid turning ordinary values into colored pills.
- **Separate UI from explanation.** Keep implementation notes and state labels
  outside the simulated app. Prefer an unboxed note column and a divided state
  strip to repeated cards. Product copy should describe the user's operation.

This takes alignment and grouping cues from JetBrains' official
[Layout guide](https://plugins.jetbrains.com/docs/intellij/layout.html),
toolbar organization from its
[Toolbar guide](https://plugins.jetbrains.com/docs/intellij/toolbar.html), and
editor context from its
[Tool Window guide](https://plugins.jetbrains.com/docs/intellij/tool-window.html).
It does not import a JetBrains palette, application titlebar, rail, or features
that Token's component contract does not describe. Proposed examples retain
their explicit implementation status. See [the fidelity review](FIDELITY-REVIEW.md)
for the first set of revised specimens and the reasons they were selected.

## Page contract and shared helpers

Each page links shared.css, then the generated theme script and shared.js. Call
Mockup.mount with id, title, description, status and content. Status is one of
"Established surface · visual study", "Proposed component", or "Deferred concept";
use the chapter's actual status. Content is the page's own HTML composition.
Keep documentation context helpers separate from the proposed native component
API. Their existence does not add a primitive to Token's implementation plan.

Shared helpers return escaped HTML fragments:

    Mockup.icon(name, size = 16)
    Mockup.code({ lines, start = 114, active = 132 })
    Mockup.editor({ title = "chrome.rs", tabs, breadcrumb = false,
                    sidebar = false, status = false, footer = "", lines })
    Mockup.tree({ selected = "chrome.rs" })
    Mockup.sparkline({ values, color, width = 240, height = 70 })
    Mockup.mount({ id, title, description, status, content })

Shared layout classes: cols, cols.equal, stack, row, grow, surface, surface-header,
surface-body, surface-footer, state-grid, state-card, state-label, window,
editor-tabs, editor-tab, breadcrumb, code-view, tree-row, status-bar, btn,
btn.primary, btn.quiet, btn.icon-btn, field, field-label, help, keycap, badge,
menu, menu-row, separator, section-title, muted, dim, mono, selected.
See shared.css for exact styles. Page-specific layout belongs in a scoped style
block, not changes to shared tokens or copied theme literals.

Static focus styling is component-scoped (`.btn.focused`, `.field.focused`, or a
page's own component selector). A bare `.focused` class must not paint a global
ring: it can silently stack with a component's border or box-shadow. Use one
deliberate focus treatment, usually a 1px border or outline, and preserve the
browser's `:focus-visible` feedback on interactive viewer controls.

For compact desktop contexts, load `ide.css` after `shared.css` and `ide.js`
after `shared.js`. These are additive; unchanged specimens keep their own layout.

    Ide.preferences({ section = "Editor", title = section, content, footer,
                      categories })

`content` and `footer` are trusted local HTML fragments, like `Mockup.mount`'s
content; user-provided values must be escaped. The helper preserves one Settings
page with category navigation. Use `ide-study` for a main scene and 100px state
strip; `ide-scene` for a main context and 216px note column; `ide-notes` /
`ide-note` for explanations; and `ide-states` / `ide-state` for unboxed variants.
`ide-context` scopes compact controls. `ide-group`, `ide-group-title`,
`ide-form-row`, `ide-check-row`, and `ide-dependent` provide the preferences
alignment baseline. These are documentation scaffolds, not new Token primitives.

Code helpers take plain source strings and escape/highlight them centrally.
An explicit breadcrumb can supply an object with path segments and an optional
symbol, for example `{ path: ["src", "view", "modal.rs"], symbol: "render_modal" }`.
The editor helper selects the tab matching its title unless a tab supplies its
own active state. Keep labels, active rows, counts, and plotted values consistent
with the one fixed fixture used by each example.

## Deterministic state and accessibility

- These are static documented states. No random numbers, clocks, network calls,
  moving charts, caret blink, or hover-dependent screenshots. Plot fixed data.
- Use data-state or explicit classes to represent hover/focus/selected/disabled;
  label the state. Do not rely on the capture tool leaving a mouse over a target.
- Established scenes show states supported by the documented implementation.
  Keep additional focus or keyboard proposals in a separately labelled sample.
  Selection, component focus, optional inspection guides, and browser comment
  markers are different layers; disabling guides must not remove real focus.
- Use semantic HTML and accessible names for controls; inputs shown as samples
  are readonly. Static representations must not claim a complete input machine.
  Decorative icons are hidden from assistive technology; charts have descriptive
  labels and adjacent textual values. No meaningful state is color-only.
- Mockup.mount waits for both fonts and sets window.__mockupReady. The renderer
  also disables animation and caret, fixes locale/timezone, and blocks external
  requests. Keep all resources local to the repository.

## Establish fidelity before expanding the catalog

The next design pass should calibrate a small set of established primitives:
Button, Text Field, Checkbox, Tabs, and Scroll Area. First verify each chapter's
current contract, then refine the component and a minimal host at normal scale.
Review text metrics, spacing, edges, focus, hover, selection, and disabled states
in the normal view, then confirm isolation in the emphasised view. Record the
resulting shared measurements before propagating that treatment to other pages.

Do not interpret all 41 existing drafts as equally mature or copy their entire
surrounding scenes into implementation. New concepts such as breadcrumbs and
activity rails remain separate proposals. Emphasis makes the subject easier to
discuss; it does not repair weak composition or establish visual acceptance.

## Regeneration and documentation

See README.md for the pinned toolchain and render commands. The renderer's
managed Markdown block is inserted directly below the chapter's H1 and contains
a relative emphasised image link, links to the normal PNG and both HTML views,
and a short caption identifying the visual target as under review.
Regeneration must replace that block, never duplicate it or rewrite the chapter.
PNG files are checked-in documentation assets at the user's requested path;
temporary browser/QA files belong under target/verification.

Inspect every render at full size or via a contact sheet, then inspect the
context-heavy and densest pages individually. Verify names, intended states,
contrast, clipping, consistent scale, and true source palette. An all-green
script result alone does not establish visual fidelity.
