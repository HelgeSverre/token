# Desktop IDE fidelity review

## Current assessment

The catalog expanded before a convincing component baseline was established.
The 15 revisions below improved density, but they did not resolve the user's
fidelity concerns and are not approved implementation targets. The 41 pages
remain a useful inventory of drafts, with differing levels of visual maturity.

The central problem is the subject of each image. Badge spends most of its space
on a Problems panel; Checkbox presents a whole preferences page; Keycap places
the marks in a large command popup. Those hosts explain location, but can
overwhelm the component's proportions, edges, typography, and states. Filling
such scenes also encourages plausible IDE workflows beyond Token's contract.
For example, the established Badge contract is a completion-kind mark; count
pills are a proposed extension. A large Problems scene makes that distinction
harder to see even though the chapter documents it. Its next revision should
give the existing completion mark the primary position and keep the count
proposal clearly separate.

Each page now has normal and emphasised views from the same source. In the
emphasised view only the component and its state variants retain their normal
appearance; everything else is composited at 25% opacity by default. The viewer
provides an opacity slider and optional inspection guides, offset from the
subject and hidden by default. Both are configurable through CSS. Chapter images lead
with that view. This isolates the review subject without redesigning the
examples or concealing their normal composition.

The next fidelity pass should refine Button, Text Field, Checkbox, Tabs, and
Scroll Area as a small baseline of established primitives. Use the smallest
context that explains each one, exact Default Dark roles, consistent text
metrics and spacing, and complete documented states. Improvement should come
from execution of the existing contract. Proposed concepts stay explicitly
separate; convincing-looking scene content does not create requirements.

The [style guide](STYLE-GUIDE.md) now reflects that narrower scope. Emphasis is
an inspection aid, not a substitute for better drawings. Technical render
checks establish repeatability and coverage, not design acceptance.

## Focus-outline audit

The Section Navigation review exposed a shared CSS collision: a global
`.focused` rule added an outer blue outline while the row drew its own blue
box-shadow. Form and Text Field also combined that outline with a focus-colored
border and shadow. These were component-state artifacts, visible even with the
viewer's inspection guides disabled.

Focus styling is now scoped to the owning component. Form and Text Field use a
single 1px focus border; the proposed Section Navigation state uses one inset
1px outline. Existing deliberate focus samples retain their own styling,
including Group Header, which now explicitly declares its ring.

The main Section Navigation and Splitter examples also showed keyboard focus
that their documented native components do not implement. Those main-scene
states have been removed. Section Navigation keeps a separately labelled
proposed focus comparison, included in the emphasised subject mapping.

The inspection guides remain a separate optional viewer layer, offset by 8px
and hidden by default. A browser comment marker is another external layer and
does not appear in the generated PNGs.

## Earlier density pass

The weakest specimens were the small controls and accessories, rather than the
editor, tree, table, and docking examples. Their individual controls were mostly
reasonable; their surroundings made them read as a web component library:
large setting cards, repeated bordered state tiles, sparse panels, and explanatory
text mixed into the simulated application.

That pass revised **15 of the 41 specimens**. All retain the 1200 × 760 capture
frame, bundled Inter / JetBrains Mono fonts, and generated Default Dark colors.
The changes concern composition, density, hierarchy, and credible owners.

| Weak specimens | What made them weak | Revision |
| --- | --- | --- |
| [Button](BUTTON.html), [Checkbox](CHECKBOX.html), [Radio Group](RADIO-GROUP.html) | Isolated settings cards, redundant description/status rows, oversized radio selection, browser-default state samples. | Flat preferences pages with category navigation, compact controls, aligned dependent options, restrained commit actions, and matching themed states. |
| [Select](SELECT.html), [ComboBox](COMBOBOX.html) | Large form cards and open lists that expanded the form. | Modest field widths, common label baselines, compact options with reserved checkmark gutters, and attached popup layers over the following rows. |
| [Group Header](GROUP-HEADER.html), [Label](LABEL.html) | Sparse form panels and weak separation between captions, help, validation, and metadata. | Language-server and typography settings with aligned controls, inset groups, quiet help, explicit validation, and document metadata in its own list. |
| [Toggle Switch](TOGGLE-SWITCH.html), [Segmented Control](SEGMENTED-CONTROL.html), [Split Button](SPLIT-BUTTON.html) | Generic workspace controls or an oversized empty action card. | A proposed live language-services surface, Token's component-gallery preview-width control with actual fixtures, and a proposed two-part Run action beside source. |
| [Toolbar](TOOLBAR.html) | Skeleton bars instead of source, a detached action row, and little relationship to the editor. | An Outline dock beside populated Rust source, grouped commands, a selected Follow toggle, attached overflow comparison, and the existing terminal action-cell arrangement. |
| [Badge](BADGE.html), [Icon](ICON.html), [Keycap](KEYCAP.html), [Progress](PROGRESS.html) | Generic boards, icon tiles, disconnected counters, and a progress dashboard. | Populated Problems/Explorer contexts, aligned shortcuts inside a command popup, and progress owned by Find references or the status area. |

## Rules carried forward

The [style guide](STYLE-GUIDE.md#desktop-ide-fidelity) now makes desktop density
explicit: 28px fields/buttons, 26px toolbar targets, 16px marks/icons, 25–28px rows,
same-line labels where space permits, and grouping through insets and hairlines.
State comparisons use a divided strip; implementation annotations sit outside
the simulated UI. `ide.css` and `ide.js` provide these shared documentation
contexts without changing the remaining specimens' styling.

JetBrains' [layout](https://plugins.jetbrains.com/docs/intellij/layout.html),
[toolbar](https://plugins.jetbrains.com/docs/intellij/toolbar.html), and
[tool-window](https://plugins.jetbrains.com/docs/intellij/tool-window.html)
guidelines informed the composition. Token supplies the palette, fonts, source
content, category navigation, status bar, and documented component contracts.
This is a visual direction, not a pixel-identical JetBrains clone or a screenshot
of a new native implementation.

Proposed controls remain marked as proposed. Illustrative Radio Group and Run
workflows do not imply those features exist today. Unknown-length work remains
textual; the separate determinate progress example has an explicit known total.

## Verification

Every revised page was rendered and visually inspected. The review also checked
source/status agreement, diagnostic counts and scope, popup overlap, consistent
control styling, and the separation of product copy from documentation notes.
Command examples match `keymap.yaml`. Function and method marks use the same
20% accent blend over the native overlay panel color; that panel role is now
included in the generated theme tokens rather than approximated with a literal.
The dual-view renderer now manages both PNGs and the chapter image links.

Reproduce the full gallery and validation with:

```sh
npm --prefix docs/ui/mockups run render
npm --prefix docs/ui/mockups run check
```

Temporary contact sheets belong under `target/verification/ui-mockups-ide/`;
they may be removed by a build-directory cleanup. The maintained documentation
images are in `renders/`.
