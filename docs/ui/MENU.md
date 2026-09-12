# Menu

## Purpose and naming

A menu is a transient ordered set of commands/choices, anchored to a trigger
and dismissed after the declared completion. It differs from Toolbar (persistent
actions), List (data selection), and Select (one value choice that may render a
menu-like popup). Token's strongest verified menu is the editor/tab/file-tree
context menu; completion/reference/code-action overlays share row machinery but
not its data or acceptance semantics.

## Verified context-menu model and ownership

`ContextMenuTarget` captures editor selection/clipboard context, a specific
tab/group/path, or a file-tree path/directory; it resolves a three-variant
`ContextMenuRegion` (`src/context_menu/types.rs:11-47`). `MenuItem` owns label,
enabled flag, optional shortcut hint, command/targeted-message/no action, and
separator (`:50-111`). `selectable_items` filters separators and
`first_enabled_index` creates the initial selectable index (`:118-146`).

| State/data                 | Owner                | Invariant                                  |
| -------------------------- | -------------------- | ------------------------------------------ |
| item vector/order          | `ContextMenuState`   | the sole ordering authority                |
| anchor `(x,y,h)`           | `ContextMenuState`   | captured at open; raw click can have `h=0` |
| selected index             | `CursorOverlayState` | indexes non-separator display order        |
| region                     | `ContextMenuState`   | builder/automation identity                |
| command or target messages | `MenuItem`           | Update resolves effect, not painter        |

Open update rejects active modals, builds region items, clears sibling cursor
overlays, selects first enabled, stores anchor/region, and redraws
(`src/update/context_menu.rs:15-74`). Shift+F10 constructs an editor target at
the live caret rect after runtime has read clipboard state (`:62-86`).
Activation ignores disabled/out-of-range selection without dismissal; enabled
activation clears menu/overlay then routes `CommandId` or targeted messages
through update (`:89-122`).

The view creates one row per non-separator item, maps hints to keycap chips, and
converts separator runs to untitled overlay sections while retaining the same
flat order (`src/view/modal.rs:2510-2600`). Cursor/menu `OverlayLayout` is the
shared geometry authority (`src/view/overlay_surface.rs:850-880`, `:1018-1040`).

## Transition, keyboard, pointer, accessibility

Transition: runtime resolves concrete target/anchor → `ContextMenuMsg::Open` →
builder produces immutable-on-open item order → layout once → painter/hit/key
use selectable order → activation validates enabled → clear overlay → route
effect. Do not index raw `items` from a pointer/key handler: separators would
shift command identity.

**Verified:** cursor overlays have limited key priority before normal editor
input (`src/runtime/input.rs:220-250`). Tests verify disabled entries remain
open, enabled command dismisses, separators cannot activate, sibling completion
state is cleared, and modals block opening (`src/update/context_menu.rs:142-280`).
No verified submenu, app-switch dismissal, semantic menu roles, or visible
keyboard focus indicator exists.

**Proposed:** Escape/click-away/app switch dismiss; Up/Down skips disabled,
Home/End find enabled endpoints, Enter activates; Left/Right remain unused
unless a single-level submenu is implemented. Right-click selection behavior is
explicit per target. Expose menu/menuitem, accessible name, shortcut, disabled,
separator, focus and expanded state; focus must not depend only on color.

## Layout, themes, gallery, acceptance

Use solved overlay geometry for paint/hit, flip/clamp cursor panels at window
edges, clip long labels and preserve shortcut readability. Context menu uses
overlay theme roles and normally UI font, not sidebar/document-tab colors. The
gallery has static `MenuRows` and select-option previews but no live context-menu
specimen (`src/model/gallery.rs:45-57`).

Priority 1: gallery/test matrix for three regions, disabled, separator, long
shortcut, edge flip/clamp, keyboard focus, click-away. Priority 2: selector/
menu-button and one-level submenu only with a concrete consumer. Acceptance:
one ordering authority; separator never activates; disabled blocks keyboard and
pointer; all dismissal paths clear state; accessible state is testable.

## Sources

Primary: [IntelliJ UI components](https://plugins.jetbrains.com/docs/intellij/user-interface-components.html)
and [toolbar drop-down/menu behavior](https://plugins.jetbrains.com/docs/intellij/toolbar-drop-down.html).
The dropdown source informs proposed behavior only. Secondary local context:
`temporary-docs/intellij-platform-sdk/references/ui-settings-and-toolwindows.md`.
