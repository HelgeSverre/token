# Context Menu (Right-Click Menu)

A context-sensitive popup menu triggered by right-click, rendered as an `OverlaySurface` context anchored at the click position — the same component that renders the command palette, pickers, and the completion/hover popups.

> **Status:** 📋 Planned
> **Priority:** P2 (Important)
> **Effort:** L (1-2 weeks)
> **Created:** 2026-01-07
> **Updated:** 2026-08-13 (revised against the shipped `OverlaySurface`/cursor-overlay system — bespoke rendering plan deleted, V1 scope and key/mouse routing finalized)
> **Milestone:** 3 - Workspace Features

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Data Structures](#data-structures)
4. [Overlay Context: rendering via OverlaySurface](#overlay-context-rendering-via-overlaysurface)
5. [Key & Mouse Routing](#key--mouse-routing)
6. [Shortcut Hint Integration](#shortcut-hint-integration)
7. [V1 Scope: Regions & Menus](#v1-scope-regions--menus)
8. [Keyboard Trigger](#keyboard-trigger)
9. [Cross-References](#cross-references)
10. [Implementation Plan](#implementation-plan)
11. [Testing Strategy](#testing-strategy)
12. [References](#references)

---

## Overview

### Current State

Right-click is wired to a stub: `runtime/mouse.rs::handle_right_click` receives the resolved `HitTarget` and event, and unconditionally returns `EventResult::Bubble` with a `// Future: show context menus based on target` comment — the dispatch point exists, hit-testing already resolves a rich `HitTarget` (`GroupTab`, `SidebarItem`, editor content, etc.), but nothing consumes it.

Separately, [overlay-surface.md](overlay-surface.md) shipped a general-purpose popup component (`OverlaySurface`, `Anchor::{Centered, Cursor}`, `Body::{List, Fields, Zones}`) that already renders the command palette, pickers, and (behind debug demos) cursor-anchored completion/hover shells via `ui.cursor_overlay: Option<CursorOverlayState>`. That doc explicitly lists the context menu as a **Future** consumer of `Anchor::Cursor` and does not implement it. This plan is that implementation.

### Goals

- **Goal 1:** Provide context-sensitive actions via right-click in the editor text area, tab bar, and file tree (V1 — see [Scope](#v1-scope-regions--menus)).
- **Goal 2:** Display keyboard shortcut hints (keycap chips) for menu items that have bound commands, reusing `overlay_surface::binding_chips`.
- **Goal 3:** Support full keyboard navigation within the menu (arrows, Enter, Escape) via the existing cursor-overlay key-routing branch.
- **Goal 4:** Reuse `OverlaySurface`/`Body::List` entirely for rendering — no new painter code, no new theme keys.
- **Goal 5:** Extensible per-region builder-function pattern so new regions (status bar, panels, outline, terminal) are cheap to add later.

### Non-Goals

- **Nested submenus:** V1 uses flat menus only, matching `Body::List`'s flat `FlatIndex` model (no submenu concept exists in `OverlaySurface`).
- **Searchable/filterable menus:** No type-to-filter (command palette / Search Everywhere serves this purpose).
- **Bespoke rendering:** No new `view/context_menu.rs` rendering module, no new painter primitives, no new `overlay.*` theme keys. Everything comes from the existing `OverlaySurface` chrome, row anatomy, and theme keys.
- **Hover-to-expand:** No submenu expansion (since no submenus in V1).
- **Code actions:** The LSP quick-fix menu ([lsp-integration.md](lsp-integration.md) Future) is a sibling `Anchor::Cursor` `Body::List` context, not this one — see [Cross-References](#cross-references).

---

## Architecture

### Integration Points

```
┌──────────────────┐     ┌───────────────────────┐
│  Right-Click     │────►│ runtime/mouse.rs       │
│  (MouseButton)   │     │ handle_right_click()   │
└──────────────────┘     └──────────┬─────────────┘
                                     │  HitTarget already resolved
                                     ▼
┌──────────────────┐     ┌───────────────────────┐
│ ContextMenuMsg   │◄────│ Build ContextMenuTarget│
│ ::Open(request)  │     │ from HitTarget, then   │
└────────┬─────────┘     │ per-region builder     │
         │                └───────────────────────┘
         ▼
┌──────────────────┐     ┌───────────────────────┐
│   update()       │────►│ ui.cursor_overlay =    │
│                  │     │ Some(ContextMenu(..))  │
└────────┬─────────┘     └───────────────────────┘
         │
         ▼
┌──────────────────┐     ┌───────────────────────┐
│ OverlaySurface    │────►│ Anchor::Cursor at the │
│ render (existing) │     │ click pixel; Body::List│
└──────────────────┘     └───────────────────────┘
```

The menu is **not** a new rendering system — it is a new *spec builder* (`view/modal.rs`-style function) that produces an `OverlaySpec` with `Body::List`, consumed by the overlay-surface render/layout/hit-test functions that already exist. The only genuinely new code is: (1) the `HitTarget → ContextMenuTarget` mapping and per-region item builders, (2) the state to hold the built items, and (3) wiring `handle_right_click` to open it and the key/mouse routing rules below.

### Module Structure

```
src/
├── context_menu/               # New module (create)
│   ├── mod.rs                  # Public exports
│   ├── types.rs                # ContextMenuTarget, ContextMenuRegion, MenuItem, MenuAction
│   └── builders.rs              # Per-region menu builders → Vec<MenuItem>
├── model/
│   └── ui.rs                   # CursorOverlayKind::ContextMenu variant (see below);
│                                #   ui.context_menu_items: Option<Vec<MenuItem>> alongside it
├── messages.rs                 # ContextMenuMsg (ActivateItem only — nav reuses cursor-overlay keys)
├── update/
│   └── context_menu.rs         # execute_menu_action(); dispatch from ModalMsg-style handler
├── runtime/
│   ├── mouse.rs                 # handle_right_click(): hit-test target → open the menu
│   └── input.rs                 # handle_cursor_overlay_key(): add a ContextMenu arm
└── view/
    └── modal.rs                 # render_context_menu(): OverlaySpec builder (Anchor::Cursor,
                                  #   Body::List), following render_palette's pattern
```

No `view/geometry.rs` additions are needed: `HitTarget::GroupTab { group_id, tab_index, tab_id }` and `HitTarget::SidebarItem { path, is_dir, .. }` already carry everything a menu builder needs — the old doc's `hit_test_tab`/`TabHitResult` gap is already closed by the shared `hit_test_ui` path all mouse handling goes through.

---

## Data Structures

These are unchanged in shape from the original spec — the model was always sound, only the render/routing plan around it was obsolete.

### ContextMenuRegion

```rust
/// Which UI region spawned the menu — V1 covers three; the rest are Future.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextMenuRegion {
    Editor,
    EditorTabBar,
    FileTree,
    // Future: StatusBar, OutlinePanel, Terminal, Dock
}
```

### ContextMenuTarget

```rust
use std::path::PathBuf;
use crate::model::editor_area::{GroupId, TabId};

/// Detailed context about what was right-clicked, built from the `HitTarget`
/// hit-testing already resolved for the click.
#[derive(Debug, Clone)]
pub enum ContextMenuTarget {
    /// Right-click in editor text area.
    Editor {
        group_id: GroupId,
        has_selection: bool,
        clipboard_has_content: bool,
    },
    /// Right-click on a tab.
    Tab {
        group_id: GroupId,
        tab_id: TabId,
        file_path: Option<PathBuf>,
    },
    /// Right-click on a file/folder in the file tree.
    FileTreeItem { path: PathBuf, is_dir: bool },
}
```

### MenuAction / MenuItem

Unchanged from the original spec:

```rust
#[derive(Debug, Clone)]
pub enum MenuAction {
    Messages(Vec<crate::messages::Msg>),
    None, // disabled items / separators
}

#[derive(Debug, Clone)]
pub struct MenuItem {
    pub label: String,
    pub enabled: bool,
    pub shortcut_hint: Option<String>, // populated by ShortcutHintProvider
    pub action: MenuAction,
    pub is_separator: bool,
}
```

`MenuItem::from_command`, `::separator`, `::custom` constructors carry over as-is (see the pre-revision doc's snippets in git history if the exact bodies are needed — nothing about them changed).

### From MenuItem to OverlaySurface Rows

The one genuinely new piece of plumbing: a `Vec<MenuItem>` (update-layer, region-agnostic) maps to `overlay_surface::Section`/`Row` (view-layer, `OverlaySurface`-shaped) at spec-build time — the same "ordering authority produces plain data, view maps it to the spec" split every other `OverlaySurface` context uses (overlay-surface.md "Ordering authority").

```rust
// view/modal.rs, alongside render_palette / render_recent_files etc.
fn context_menu_sections<'a>(items: &'a [MenuItem]) -> Vec<Section<'a>> {
    // Separators become section boundaries (see "Separators" below): each
    // run of non-separator items between separators is its own `Section`
    // with `title: None`. `FlatIndex` then addresses only real rows,
    // exactly like every other list context — disabled items stay in the
    // list (so keyboard nav order matches the visual order) but are
    // unselectable, per the existing `enabled` skip rule.
}

fn row_for_item(item: &MenuItem) -> Row<'_> {
    Row {
        icon: RowIcon::None, // or RowIcon::Glyph for the handful of items that want one
        label: &item.label,
        match_indices: &[], // context menus have no fuzzy query
        detail: None,
        accessory: match &item.shortcut_hint {
            Some(_binding_str) => Accessory::Keycaps(/* binding_chips() output, cached */),
            None => Accessory::None, // no DimText filler — an unbound action just has no accessory
        },
    }
}
```

---

## Overlay Context: rendering via OverlaySurface

### Where it plugs into `CursorOverlayKind`

`ui.cursor_overlay: Option<CursorOverlayState>` (`src/model/ui.rs`) already exists for exactly this purpose — a non-modal, cursor-anchored popup with its own key-routing branch. `CursorOverlayState { kind: CursorOverlayKind, selected: usize, scroll: usize }` is region-agnostic; it needs nothing new to host a menu **except** somewhere to hold the built `Vec<MenuItem>`, because unlike `Completion`/`Hover` (which read live model state — `completion_menu`, `hover_card`), a context menu's item list is a one-shot snapshot built at open time from the click's `HitTarget`.

**Decision:** add `CursorOverlayKind::ContextMenu` and a sibling `ui.context_menu_items: Option<Vec<MenuItem>>`, populated alongside `ui.cursor_overlay = Some(CursorOverlayState::new(ContextMenu))` when the menu opens and cleared when it closes — mirroring the `Completion`/`completion_menu` and `Hover`/`hover_card` pairing exactly. This was evaluated against reusing `completion_menu`'s shape (rejected: that struct is `CompletionItem`-typed and LSP-flavored, wrong domain) and against a bespoke non-`cursor_overlay` state (rejected: it would need its own copy of the flip/clamp/dismiss/hit-test machinery `cursor_overlay` already has). `selected`/`scroll` on `CursorOverlayState` are reused as-is for keyboard navigation and the (V1-unlikely, but not excluded) case of a menu taller than the visible cap.

### Anchor

`Anchor::Cursor { x, y, h, prefer_below, width }` takes a **pixel rect**, not a text-grid position, despite the field names reading like caret geometry — it's `(x, y)` top-left + line-height `h`, in physical px, used today by `view::caret::active_text_input_rect` for the text caret and by the debug completion/hover demos for an arbitrary point. A right-click needs the exact same shape: `x, y` = the click point, `h` = 0 (there's no "line" to flip around — the menu always flips relative to the click point itself, using the existing below-space check with a zero-height anchor line, which degenerates correctly since `y + h == y`). No new `Anchor` variant, no signature change — this is a **usage note**, not a code change:

```rust
Anchor::Cursor {
    x: click_x_px,
    y: click_y_px,
    h: 0,
    prefer_below: true, // matches VS Code / JetBrains: menu drops down-right from the click
    width: WidthRule { pct: 100.0, min: 160.0, max: 320.0 }, // content-sized in practice; these bound it
}
```

Flip-above-when-no-room and edge-clamping are identical to every other `Anchor::Cursor` consumer — nothing region-specific.

### Body::List and row anatomy

`Body::List { sections, selected, scroll, max_visible }` with the "Completion" row-height class from overlay-surface.md's row table (24px, 4px inset, 5px radius) — a context menu is closer to completion in density than to the 30px centered-list rows. `max_visible`: uncapped in practice (V1 menus top out at ~8 items including separators), but the existing `SelectableListViewport` scroll machinery applies unmodified if a future region needs more.

- **Icon:** `RowIcon::None` for most items; `RowIcon::Glyph` is available but not required — V1 menus lean on label text alone (matches the "text-first" feel of the reference platforms' native menus more than completion's badge-heavy rows).
- **Accessory:** `Accessory::Keycaps(binding_chips(hint))` for any item with a bound command; `Accessory::None` (not `DimText`) when unbound — a blank trailing column reads better than a manufactured dim placeholder for a plain context menu.
- **Separators:** **Section boundary**, not a dedicated thin-rule row. `Section` already exists as a rendering unit (title + hairline `rule` + rows) and separators in a flat action menu are structurally "no more items in this group" — reusing `Section { title: None, rows }` boundaries means zero new paint code and `FlatIndex` naturally treats a separator as non-addressable (no `MenuItem::separator()` sentinel row to special-case in `move_up`/`move_down`, which the original doc had to hand-write). The tradeoff: `Section`'s rule line always renders under a titled section; an untitled section renders no rule, so V1 separators are **whitespace-only breaks** (the section boundary's vertical gap, no visible hairline) rather than a drawn line. This is visually close enough to a thin rule at 24px row height that it is not worth a dedicated row variant; revisit if a designer flags it.

### Theming

Entirely `overlay.*` — no new keys. Panel/hairline/shadow from the `Anchor::Cursor` chrome row in the Visual Language table (8px radius, `panel_background`/`panel_secondary`, ring shadow); selection wash `overlay.selection_wash` + `text_bright` lift; keycaps `overlay.keycap_{bg,border,fg}` via the shared `draw_keycap` primitive.

---

## Key & Mouse Routing

### Keyboard: consume-five, dismiss-and-passthrough on anything else

The context menu's key handling **differs from Completion's** and matches the doc's own **Hover** precedent more than its Completion precedent, with one adjustment. Compare against the three existing `handle_cursor_overlay_key` branches (`runtime/input.rs`):

| Kind | Up/Down/Enter/Esc/Tab | Any other key |
| --- | --- | --- |
| `Completion` | navigate / accept / dismiss | **passes through** to the document (typing narrows the filter — the whole point of a completion popup) |
| `Hover` | (irrelevant — dismissed before matching) | **dismisses**, then the key still reaches the editor |
| **`ContextMenu` (new)** | navigate (Up/Down) / activate (Enter) / dismiss (Esc) | **dismisses the menu**, and the key is then **consumed**, not passed through |

The menu needs its own branch, not a reuse of either existing one: unlike Completion, a context menu has no query to narrow (typing "c" for Copy isn't a filter), so passthrough would just leak keystrokes into the document while the menu silently sits open over it — confusing. Unlike Hover, a context menu is not dismiss-only: it has real Up/Down/Enter navigation to preserve. Tab is **not** claimed by the menu (unlike Completion, where it doubles as Enter) — Tab has no natural meaning for a flat action menu and is better left unclaimed so it dismisses like any other key rather than silently doing nothing.

```rust
// runtime/input.rs::handle_cursor_overlay_key, new arm alongside
// DebugCompletion/DebugHover/Completion:
if kind == CursorOverlayKind::ContextMenu {
    return match key {
        Key::Named(NamedKey::ArrowUp) => { /* move selection, skip disabled/section gaps */ }
        Key::Named(NamedKey::ArrowDown) => { /* ditto */ }
        Key::Named(NamedKey::Enter) => {
            // activate selected item, dismiss, execute its MenuAction
        }
        Key::Named(NamedKey::Escape) => {
            model.ui.cursor_overlay = None;
            model.ui.context_menu_items = None;
            Some(Some(Cmd::Redraw))
        }
        _ => {
            // ANY other key: dismiss and consume (do not fall through).
            model.ui.cursor_overlay = None;
            model.ui.context_menu_items = None;
            Some(Some(Cmd::Redraw))
        }
    };
}
```

The `_ => { dismiss; Some(...) }` arm (returning `Some`, not `None`) is the mechanism that makes "any other key" *consumed* rather than *passed through* — returning `None` is what Completion does to signal "not mine, let it fall through"; a context menu never does that.

### Mouse: click-away consumes (JetBrains behavior), not click-through (VS Code)

**Decision:** consumed. A click outside the menu (anywhere — editor, sidebar, another tab) dismisses the menu and does **not** additionally act on whatever it landed on. This matches the existing `HitTarget::CursorOverlay`/`HitTarget::Modal` precedent already in the codebase (`hit_test.rs` doc comment: "outside dismisses"; modal outside-clicks are consumed today, not click-through) — introducing click-through for context menus specifically would be the one inconsistent surface in the app. It's also the safer default: a mis-click that both dismisses a menu *and* fires a destructive editor/file-tree action (e.g. accidentally selecting a different file in the tree while dismissing) is worse than one extra click.

**Alternative noted, not chosen:** VS Code's single-click-through (the dismissing click also lands where it fell) is the more "modern" feel and is a plausible future toggle if this feels heavy in practice — not worth the small validation surface for a V1.

Implementation follows the existing `HitTarget::CursorOverlay { flat_index }` pattern exactly (`hit_test_ui` already gives cursor overlays highest priority ahead of the modal check, claims only points inside the popup panel, and `handle_mouse_press`'s preamble dismisses on any press outside): a row click at `flat_index` activates that item (mirrors `ModalRow`); a click inside the panel but not on a row (e.g. the whitespace-only separator gap) is a no-op, not a dismiss; nothing new is needed in `hit_test.rs` beyond confirming `CursorOverlayKind::ContextMenu` routes through the same arm as `Completion`/`Hover` today.

Scroll wheel: no scroll behavior needed for V1 (menus fit without scrolling); the existing `HoverRegion::CursorOverlay` wheel routing is inert here since `max_scroll` is 0, same as the current Hover card.

---

## Shortcut Hint Integration

Unchanged in shape from the original spec: a `ShortcutHintProvider<'a> { keymap: &'a Keymap }` with `hint_for(command: Command) -> Option<String>`, built in the same place menus are built (see [Menu Building Location](#menu-building-location-unchanged) below), feeding `MenuItem.shortcut_hint`. The only change from the original doc: the hint string is no longer painted as raw text — it's converted to keycap chips via `overlay_surface::binding_chips(&hint)` at spec-build time (the same function the command palette already uses for its own keycap accessories), and the existing **>4-chip → `DimText` fallback** rule (Visual Language > Keycaps) applies unmodified. `format_keystroke`/`key_to_string` (or equivalent) is whatever the palette's existing hint-formatting path already uses — this doc does not re-specify it, only points at reuse.

### Menu Building Location (unchanged)

Menus are still built in `App` (or wherever `handle_right_click` runs — `runtime/mouse.rs`'s `handle_mouse_press` dispatch, which already receives `&mut AppModel` and would need read access to the keymap the way `handle_modal_key`'s callers do today), for the same reason as the original doc: `Keymap` isn't threaded into `update()`.

---

## V1 Scope: Regions & Menus

Per-region builder functions (`context_menu/builders.rs`), each `fn(&AppModel, &ContextMenuTarget, &ShortcutHintProvider) -> Vec<MenuItem>` — the pattern from the original doc, carried over unchanged.

### Editor text area

| Item | Command | Enabled when |
| --- | --- | --- |
| Cut | `Command::Cut` | has selection |
| Copy | `Command::Copy` | has selection |
| Paste | `Command::Paste` | clipboard has content |
| *(separator)* | | |
| Go to Definition | `Command::GotoDefinition` (⌘B) | LSP available at cursor |
| Show Usages | *new* `Command::ShowUsages` (⌥⌘F7) — landing alongside this feature; see note below | LSP available at cursor |
| Show Hover | `Command::ShowHover` (⇧⌘D) | always |
| *(separator)* | | |
| Reveal in File Explorer | dispatches the file-tree reveal (reuses the sidebar's existing "reveal active file" behavior) | file has a path (not an unsaved buffer) |

`Show Usages` (⌥⌘F7) is being implemented concurrently with this feature (find-references, LSP Future territory) — the menu item is speced against its `CommandId`/keybinding now so the builder doesn't need a follow-up edit once the command lands; if it hasn't landed when this phase ships, the row is simply omitted rather than shown disabled (an item for a command that doesn't exist yet has no `Command` variant to point `MenuAction::from_command` at).

### Tab (editor tab bar)

`ContextMenuTarget::Tab` needs `group_id` + `tab_id` threaded through so actions apply to the *clicked* tab, not the focused one — `HitTarget::GroupTab { group_id, tab_index, tab_id }` already carries exactly this from hit-testing.

| Item | Action | Notes |
| --- | --- | --- |
| Close | `LayoutMsg::CloseTab(tab_id)` | already exists |
| Close Others | *new* `LayoutMsg::CloseOtherTabs { group_id, keep: tab_id }` | new variant |
| Close All | *new* `LayoutMsg::CloseAllTabs { group_id }` | new variant |
| *(separator)* | | |
| Reveal in File Explorer | | file has a path |
| Reveal in Finder | OS file manager reveal (existing `RevealInFinder`-style command, generalized off "current file" to the clicked tab's `file_path`) | file has a path |
| Copy Absolute Path | `CommandId::CopyAbsolutePath` pattern, targeted at the clicked tab | file has a path |
| Copy Relative Path | `CommandId::CopyRelativePath` pattern, targeted at the clicked tab | file has a path, workspace open |

`CloseOtherTabs`/`CloseAllTabs` don't exist on `LayoutMsg` today (only `CloseTab(TabId)` and `CloseFocusedTab`) — new variants, scoped to a `group_id` so multi-split layouts only close tabs in the clicked group's bar, matching every editor's tab-context-menu convention.

### File tree

| Item | Action | Notes |
| --- | --- | --- |
| Open | opens the file (dirs: expand/collapse, matching left-click) | |
| *(separator)* | | |
| Reveal in Finder | | |
| Copy Absolute Path | | |
| Copy Relative Path | | workspace open |
| *(separator)* | | |
| Refresh Tree | `WorkspaceMsg::Refresh` (existing — `CommandId::FileTreeRefresh`) | |

File-tree Rename/Delete/New File/New Folder stay **deferred**, matching the original doc's Phase 5 deferral list — no new information changes that call.

### Future (not V1)

Status bar, docked panels (Outline, Terminal, other dock panels) — same `ContextMenuRegion`/builder pattern extends to them later; listed for completeness, not scoped here.

---

## Keyboard Trigger

**Shift+F10** opens the editor's context menu at the **text caret** position (not the mouse position — there is no mouse position to anchor to on a keyboard trigger). This is VS Code's binding; JetBrains has no universal equivalent (context-dependent per tool window), so there's nothing to reconcile against. Only the editor region gets a keyboard trigger in V1 — tab and file-tree menus are click-only, matching how those regions have no other keyboard-invoked popups today either.

Implementation: `Shift+F10` resolves to `Command::ShowContextMenu` (new), which builds the editor menu (`ContextMenuTarget::Editor` from the current cursor position/selection rather than a click) and opens it anchored at the caret rect from `view::caret::active_text_input_rect` — the exact same rect the text caret and completion popup already use, so no new geometry code, only a new `Anchor::Cursor` call site with `prefer_below: true` and `h` = the real line height instead of 0.

---

## Cross-References

- **[overlay-surface.md](overlay-surface.md)** owns `OverlaySurface`, `Anchor::Cursor`, `Body::List`, row/keycap/theme primitives, and the `cursor_overlay` key-routing branch this doc extends with a `ContextMenu` arm. That doc's Future list ("Context menu as a cursor-anchored context") is resolved by this doc.
- **[lsp-integration.md](lsp-integration.md)** (LSP Future: code actions) will consume the same `Anchor::Cursor` + `Body::List` shell for its quick-fix menu — an `ActionMenu`-style sibling context, not a variant of `ContextMenu` (different trigger: a gutter/diagnostic-driven lightbulb, not right-click; different dismiss rule likely). The row-anatomy and key-routing precedent set here (consume-and-dismiss-on-other-key, rather than Completion's passthrough) is the one code actions should default to as well, since it's also a flat action list with no query.
- **`Command::GotoDefinition`, `Command::ShowHover`** (lsp-integration.md Phase 3/4) and the concurrently-landing **Show Usages** command are consumed, not defined, by this doc.

---

## Implementation Plan

Phases reordered against the real seams: rendering/theming is a solved problem (reuse), so the work is hit-test wiring, state, builders, and routing.

### Phase 1: State & Message Plumbing

**Effort:** S (1-2 days)

- [ ] `CursorOverlayKind::ContextMenu` variant; `ui.context_menu_items: Option<Vec<MenuItem>>` alongside `ui.cursor_overlay`, cleared together everywhere `cursor_overlay` is cleared (grep the existing `cursor_overlay = None` sites — `Escape`, dismiss-on-other-key, focus loss — and add the paired clear, the same way `hover_card`/`completion_menu` are paired today).
- [ ] `context_menu/types.rs`: `ContextMenuRegion`, `ContextMenuTarget`, `MenuAction`, `MenuItem` (constructors carried over from the pre-revision spec).
- [ ] `ContextMenuMsg::ActivateItem { index }` (or fold activation into the existing cursor-overlay Enter-key arm directly — no separate message type needed if `handle_cursor_overlay_key`'s `ContextMenu` arm executes the action inline, matching how `Completion`'s arm calls `update()` directly rather than round-tripping through a dedicated message).

### Phase 2: Hit-Test Wiring & Open/Close

**Effort:** M (2-3 days)

- [ ] `runtime/mouse.rs::handle_right_click`: map `HitTarget` → `ContextMenuTarget` for the three V1 regions (editor content, `GroupTab`, `SidebarItem`); dispatch to the matching builder; open `ui.cursor_overlay` + `ui.context_menu_items` at the click's pixel position (`Anchor::Cursor` with `h: 0`, per [Overlay Context](#overlay-context-rendering-via-overlaysurface)). Regions with no V1 menu (status bar, dock, etc.) keep bubbling.
- [ ] Guard: don't open while a modal is active (`ui.has_modal()`) or another cursor overlay is already open (close the old one first, matching `Completion`'s re-open behavior).
- [ ] `handle_cursor_overlay_key`: new `ContextMenu` arm — Up/Down/Enter/Esc claimed, dismiss-and-consume on everything else (see [Key & Mouse Routing](#key--mouse-routing)).
- [ ] `hit_test_ui` / `handle_mouse_press`: confirm `HitTarget::CursorOverlay` already covers row-click activation and outside-click dismissal for `ContextMenu` the same as `Completion`/`Hover` (it should — the branch is kind-agnostic); add a unit test asserting it.
- [ ] `Shift+F10` → `Command::ShowContextMenu`, anchored at the caret rect (editor region only).

### Phase 3: Rendering (spec builder only — no new paint code)

**Effort:** S (1-2 days)

- [ ] `view/modal.rs::render_context_menu`: `MenuItem` list → `OverlaySpec { anchor: Anchor::Cursor { .. }, body: Body::List { .. }, header: None, footer: None, tabs: None }`, following `render_palette`'s existing pattern for how a builder scope constructs and immediately consumes a spec.
- [ ] `context_menu_sections`: separator → `Section` boundary mapping (see [Data Structures](#data-structures)).
- [ ] Wire into whatever dispatches `OverlaySurface::render` for cursor overlays today (alongside the `Completion`/`Hover`/debug-demo arms).

### Phase 4: Per-Region Builders

**Effort:** M (2-3 days)

- [ ] `ShortcutHintProvider` (or confirm the palette's existing equivalent is reusable as-is — likely, since it only needs `&Keymap`).
- [ ] `build_editor_menu`, `build_tab_menu`, `build_file_tree_menu` per the [V1 Scope](#v1-scope-regions--menus) tables.
- [ ] New `LayoutMsg::CloseOtherTabs`/`CloseAllTabs` variants + their `update` arms.
- [ ] Unit tests per builder: enablement rules (Paste needs clipboard, tab items need the right `tab_id` threaded through, LSP items need a server attached).

### Phase 5: Polish & Automation

**Effort:** S (1-2 days)

- [ ] Automation snapshot arm: `overlay: { context: "context_menu", region, rows: [...], selected }` — reuses the existing `overlay` snapshot shape from overlay-surface.md Behaviour (`SetOverlayInput` doesn't apply here — no query input — but open/navigate/confirm via command-by-name does).
- [ ] Manual checklist (below) at 1x/1.25x/2x, a couple of bundled themes.
- [ ] Update CHANGELOG.md.

### Future (deferred, unchanged from the original doc's judgment)

- [ ] File-tree: New File/Folder, Rename, Delete, Copy/Cut/Paste.
- [ ] Tab: Close Saved, Close to the Right, Move to new split.
- [ ] Editor: Format Selection, Toggle Comment.
- [ ] Status bar / Outline / Terminal / Dock regions.
- [ ] Code actions (LSP Future) as a sibling `Anchor::Cursor` context.
- [ ] Click-through (VS Code-style) as a togglable alternative to click-away-consumes, if requested.

---

## Testing Strategy

### Unit Tests

- `ContextMenuTarget` construction from each V1 `HitTarget` variant.
- Builder enablement rules (Cut/Copy require selection, Paste requires clipboard content, path items require a saved file, tab items resolve against the *clicked* `tab_id` not the focused one).
- `handle_cursor_overlay_key` for `ContextMenu`: Up/Down wrap and skip section gaps; Enter activates and dismisses; Escape dismisses; any other key dismisses **and returns `Some`** (consumed, not passed through) — the one behavioral divergence from `Completion` worth a dedicated regression test given how easy it'd be to copy-paste the wrong arm.
- `context_menu_sections`: separator-to-section-boundary mapping, including the "disabled items stay addressable-but-unselectable" rule.
- `binding_chips` >4-chip fallback still applies (already covered by existing `overlay_surface` tests — assert a context-menu row hits the same path, not a fresh regression).

### Integration / Automation Tests

1. Right-click in editor → `overlay` snapshot shows the Editor menu with correct enablement.
2. Right-click a tab → menu items resolve against that tab, not the focused one (drag focus elsewhere first, then right-click a non-focused tab).
3. Keyboard nav: open → Down → Down → Enter → correct action executes, menu closes.
4. Click outside menu → menu closes, no action fires on the click target (click-away-consumes).
5. `Shift+F10` in editor → menu opens at the caret, not at a stale mouse position.

### Manual Testing Checklist

- [ ] Right-click in editor, on a tab, on a file/folder in the tree — correct menu each time
- [ ] Cut/Copy disabled with no selection; Paste disabled with empty clipboard
- [ ] Arrow keys navigate, skipping the separator gaps; Enter activates; Escape closes
- [ ] Any non-navigation key (e.g. typing a letter) closes the menu without inserting into the document
- [ ] Clicking outside the menu closes it without triggering the click target
- [ ] Menu flips/clamps correctly near window edges (right edge, bottom edge)
- [ ] Keycap chips render correctly for bound items (⌘B, ⇧⌘D, etc.); unbound items show no accessory
- [ ] Shift+F10 opens the editor menu at the caret
- [ ] Menu closes when a modal opens or focus is lost
- [ ] Legible under at least one light and one dark bundled theme (no new keys, but confirm nothing clips)

---

## References

### Internal Docs

- [overlay-surface.md](overlay-surface.md) — the component this doc builds on: `OverlaySurface`, `Anchor::Cursor`, `Body::List`, `binding_chips`, `cursor_overlay` routing, hit-testing shared-layout rule, `overlay.*` theme keys.
- [lsp-integration.md](lsp-integration.md) — `GotoDefinition`, `ShowHover`, future code-actions sibling context.
- [Panel UI Abstraction](../archived/panel-ui-abstraction.md) — historical reference only.

### External Resources

- [VS Code Context Menu](https://code.visualstudio.com/docs/getstarted/userinterface#_context-menus) — menu organization, Shift+F10 trigger.
- [macOS HIG: Context Menus](https://developer.apple.com/design/human-interface-guidelines/context-menus) — platform conventions.

---

## Design Decisions

| Decision | Options Considered | Chosen | Rationale |
| --- | --- | --- | --- |
| Rendering | new bespoke `view/context_menu.rs` vs. `OverlaySurface` context | `OverlaySurface`, `CursorOverlayKind::ContextMenu` | The component already exists and does exactly this shape of work; a second rendering system would duplicate chrome/shadow/theming for no benefit |
| Menu state | reuse `completion_menu` shape vs. new `context_menu_items` | new, paired with `cursor_overlay` like `hover_card` | `completion_menu` is `CompletionItem`-typed (wrong domain); the pairing pattern is already proven twice |
| Anchor | new `Anchor` variant for a raw point vs. reuse `Anchor::Cursor` with `h: 0` | reuse, `h: 0` | `Anchor::Cursor` already degenerates correctly to a point; a new variant would duplicate flip/clamp logic for no behavioral gain |
| Separators | dedicated thin-rule row vs. `Section` boundary | `Section` boundary (whitespace-only break) | Zero new paint code; `FlatIndex` already treats section boundaries as non-addressable, avoiding a hand-written separator-skip in nav |
| Key routing on non-nav keys | passthrough (Completion-style) vs. dismiss-and-consume | dismiss-and-consume | A context menu has no query to narrow; passthrough would leak keystrokes into the document under an still-open, now-stale menu |
| Click-away | consume (JetBrains) vs. click-through (VS Code) | consume | Matches every existing overlay's outside-click behavior in this codebase; avoids compounding a dismiss with an accidental destructive click |
| Keyboard trigger | none vs. Shift+F10 | Shift+F10 (editor only) | VS Code precedent; JetBrains has no universal equivalent to reconcile against; tab/file-tree stay click-only like their other interactions |

## Open Questions

1. ~~Bespoke rendering or OverlaySurface?~~ → OverlaySurface; this revision.
2. ~~Where does the item list live in the model?~~ → `ui.context_menu_items`, paired with `ui.cursor_overlay`.
3. ~~Separator rendering?~~ → `Section` boundary, whitespace-only (no drawn rule for an untitled section).
4. Click-through vs. consume may be worth revisiting after real usage — flagged, not blocking V1.
5. Whether code actions (LSP Future) should share `ContextMenuRegion`/builder machinery or stay fully separate — leaning separate (different trigger, likely different dismiss rule), final call deferred to that doc.

---

## Changelog

| Date | Change |
| --- | --- |
| 2026-01-07 | Initial draft |
| 2026-01-07 | Added integration gaps: focus/input routing, coordinates, menu building location, tab hit-testing, keymap API |
| 2026-08-13 | Revised against the shipped `OverlaySurface`/cursor-overlay system: deleted the bespoke rendering plan, retargeted at `CursorOverlayKind::ContextMenu` + `Anchor::Cursor`, finalized V1 scope (editor/tab/file-tree), key routing (dismiss-and-consume on non-nav keys), mouse routing (click-away consumes), and Shift+F10 keyboard trigger |
