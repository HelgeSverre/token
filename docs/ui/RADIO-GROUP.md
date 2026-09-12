# Radio group

## Purpose and current status

A radio group exposes a small set of mutually exclusive choices with all
options visible. Token has **no implemented radio-circle painter or semantic
radio-group type**. Existing Settings choice rows are visually selected buttons;
the reusable `SegmentedControl` is the closest single-choice primitive
([segmented_control.rs](../../src/view/segmented_control.rs#L26)). The wrapped
choice geometry in [controls.rs](../../src/view/controls.rs#L119) is shared by
Settings and its `choice-group.selected` gallery fixture, but is neither a
radio-group model nor proof of radio keyboard behaviour.

### Current overlap versus missing component

| Candidate piece                 | Present behaviour                                                                   | Why it is not a radio group                                                          |
| ------------------------------- | ----------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| `SegmentedControl`              | Paints labelled, selected adjoining button surfaces.                                | No group label, option IDs, roving focus, radio indicator, events or disabled state. |
| Settings `FormChoice`           | Stores static labels and active index; update changes active and marks draft dirty. | It may render buttons or a select based on context, not radio controls.              |
| `choice_group_rects`            | Returns shared wrapping rectangles for Settings/gallery button choices.             | It has no selection model or pointer/keyboard dispatch.                              |
| Gallery `choice-group.selected` | Static visual fixture.                                                              | It does not instantiate a radio group or verify events/accessibility.                |

There are no current data fields, theme roles, focus states or gallery acceptance
that can be labelled “implemented radio”. Keep this contract proposed until a
consumer needs visible, non-segmented alternatives.

## Proposed contract

Use a RadioGroup when 2–4 choices are mutually exclusive, labels can be visible
and the alternatives deserve equal prominence. Model it as `group_id`, required
group label, options `{ id, label, help?, enabled }`, one selected id and an
owner `Select(id)` message. It does not own persistence or effects. A control
with no selected value must say so explicitly rather than silently choosing an
index; most Settings choices should have a safe/default selection.

Pointer click selects an enabled option. Tab enters/leaves once; arrow keys move
within choices and commit; Space chooses the focused option; Home/End choose
ends. Role/name is `radiogroup` from its group label, with each option role
`radio`, checked and disabled state. Include label and description in the
accessible name/description. Do not represent an on/off setting as two radios;
use a checkbox. This is **proposed**, guided by IntelliJ's [Radio button](https://plugins.jetbrains.com/docs/intellij/radio-button.html).

| Proposed data                                    | Ownership / invariant                                                                                      |
| ------------------------------------------------ | ---------------------------------------------------------------------------------------------------------- |
| `group_id`, visible group label/help             | Group owns semantic name and description; label is required, not placeholder text.                         |
| Options `{ id, label, help?, enabled }`          | Stable IDs; labels can wrap to two lines but should remain concise.                                        |
| `selected: Option<Id>` and `focused: Option<Id>` | Owner owns committed selection and roving focus; no component persistence/effects.                         |
| `invalid`, `disabled`, `read_only`               | Explicit field/group state. Disabled blocks and is skipped; read-only exposes selection but cannot change. |
| `Select(id)` message                             | Update layer commits typed value and may return commands.                                                  |

| Proposed transition            | Contract                                                                                                                                   |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------ |
| Initial/replaced options       | Reconcile selection/focus by ID. If selected disappears, choose declared fallback or explicit invalid/no-selection state; never raw index. |
| Pointer / Space                | Enabled option becomes selected and receives focus.                                                                                        |
| Arrow / Home / End             | Move roving focus among enabled options and commit according to declared policy.                                                           |
| Tab / Shift+Tab                | Enter/leave group once, never once per option.                                                                                             |
| Disabled/read-only/empty group | No activation; empty group is unavailable with explanatory text rather than an unlabeled blank area.                                       |

## Layout, visual and gallery requirements

Put the group label before its choices; place label beside each control, use
sentence case, avoid terminal punctuation and negation. Short 2–3 choice groups
may share a line; otherwise arrange vertically. The general alignment guidance
is primary-source IntelliJ [Layout](https://plugins.jetbrains.com/docs/intellij/layout.html).

No radio theme roles exist. Proposed roles should cover normal/hover/focused,
selected indicator, disabled text, error border/text and group label/help; do
not overload `button.background_selected` merely because settings currently
uses buttons. Use UI font and parent scale/clip authority.

Acceptance: gallery states for 2/4 choices, selected/focused/disabled/error,
long two-line label, narrow wrapping, each keyboard transition, and radio versus
segmented selection. Prefer the already-shared `choice_group_rects` only if its
layout rules are adopted intentionally; otherwise create one geometry authority.
Also assert label/option counts, focus reconciliation after replacement and an
accessible group name; current Token has none of these tests.
