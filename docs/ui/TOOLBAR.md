# Toolbar

## Purpose and current boundary

A toolbar is a persistent, compact group of frequent actions/settings for one
scope. It is not a Menu (transient command choice), section navigation, or a
tab strip. Token has buttons and domain action rows but no shared `Toolbar`
model/painter, focus policy, overflow component, or customization system.

## Verified consumers and anatomy

The closest current surface is terminal tab/action chrome. It reserves fixed
Previous, Next, New, Close action cells around a clipped sessions viewport
(`src/panels/terminal.rs:17-76`). It uses the general button renderer for
hover/normal state and renders `<`, `>`, `+`/pending `...`, `x`
(`src/panels/terminal.rs:99-143`). The same row also selects terminal sessions,
so its state is terminal chrome, **not** a reusable toolbar.

| Existing piece          | Owner                 | Why it is not shared toolbar evidence         |
| ----------------------- | --------------------- | --------------------------------------------- |
| terminal action squares | terminal model/update | coupled to session tabs/spawn/close lifecycle |
| form/panel buttons      | owning modal/panel    | feature-local geometry/events                 |
| document tab strip      | editor group          | navigation only; no close action button       |

`TerminalMsg::Tab` owns terminal actions and produces lifecycle commands
(`src/update/terminal.rs:16-82`). The gallery's terminal normal/overflow/exited
specimens prove this current chrome (`src/model/gallery.rs:83-130`). No Token
main toolbar equivalent is represented by the gallery or `src/view/` renderer.

## Proposed data and event contract

Use a toolbar only when its owner can identify a stable scope and a common
workflow. The owner supplies ordered `ActionId`s, visible/enabled/selected/
pending state, accessible label and tooltip, icon/text presentation, group
separators, overflow policy, and activation message. Toolbar resolves geometry,
hit targets and visual state; it does not perform I/O or execute a command.

| Item kind       | Required owner data         | Activation                            |
| --------------- | --------------------------- | ------------------------------------- |
| action          | ID, label, enabled/pending  | message routes through Update/Command |
| toggle          | action data + selected      | same action changes declared state    |
| menu button     | label + menu model/expanded | opens Menu contract                   |
| selector/search | value/query + focus         | feature-owned update                  |
| separator/label | presentation only           | never activates                       |
| overflow        | hidden action IDs           | opens Menu with same actions/order    |

Transition: owner computes actions → one layout solves visible/overflow rects →
paint/hit use it → pointer/key emits action ID → update checks enabled and
returns command/redraw → state refreshes. Keyboard, menu and toolbar routes
must reuse the same action command rather than duplicate side effects.

## Focus, accessibility, layout and theme

**Verified:** terminal stores hover as `hovered_tab`; no shared toolbar focus,
roving keyboard, tooltips, or accessibility roles are implemented. **Proposed:**
Tab reaches a keyboard-operable toolbar; arrows rove compact icon groups; Enter/
Space activate; Tab exits; menu buttons follow Menu dismissal/selection. Expose
role/name/pressed/disabled/expanded state and a focus indicator independent of
hover/color. Icon-only actions require a tooltip/name.

Use one scaled horizontal or vertical row. Reserve a visible overflow affordance
instead of clipping actionable controls. The terminal container uses sidebar
colors while its buttons use button painting; a new toolbar must choose explicit
theme roles, not borrow document-tab colors. UI controls normally use
`FontRole::Ui`; terminal's Code choice is deliberate. Code/UI metrics/caches
are separate (`src/view/frame.rs:832-930`).

Primary IntelliJ guidance allows action/toggle/dropdown/split buttons, search,
labels, separators and overflow; recommends only frequent actions and a
chevron instead of a second toolbar when space runs out. [Toolbar guideline](https://plugins.jetbrains.com/docs/intellij/toolbar.html)

## Gallery, gaps, acceptance

Priority 1: do not extract until another concrete toolbar shares both geometry
and interaction. If extracted, gallery must prove normal/hover/pressed/focused/
disabled/toggle/menu/overflow, narrow and HiDPI states, and keyboard invocation.
Priority 2: customization only for a genuine main toolbar. Acceptance: one
geometry authority; hidden items remain reachable; disabled blocks pointer and
keyboard; command result matches menu invocation; semantic name/state is
available. Secondary local context: `temporary-docs/intellij-platform-sdk/references/ui-settings-and-toolwindows.md`.
