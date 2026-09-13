# Menu — context-menu implementation reference

## Scope

A menu is a transient, ordered command set anchored to a trigger and dismissed after its declared route. Token's real menu is the editor/tab/file-tree context menu. Completion, references, code actions, and Settings Select use some overlay machinery but have distinct data ownership and are not arbitrary `MenuItem` consumers.

The central invariant is one **addressing space**: keyboard selection, pointer hit testing, render rows, automation, and activation all address “nonseparator items in display order.” A raw `Vec<MenuItem>` index becomes wrong as soon as separators exist.

## Current representation, ownership, and lifetime

**Current excerpt** — [src/context_menu/types.rs](../../src/context_menu/types.rs) and [src/model/ui.rs](../../src/model/ui.rs).

```rust
pub enum ContextMenuTarget {
    Editor { group_id: GroupId, has_selection: bool, clipboard_has_content: bool },
    Tab { group_id: GroupId, tab_id: TabId, file_path: Option<PathBuf> },
    FileTreeItem { path: PathBuf, is_dir: bool },
}
pub enum MenuAction { Command(CommandId), Messages(Vec<Msg>), None }
pub struct MenuItem {
    pub label: String, pub enabled: bool, pub shortcut_hint: Option<String>,
    pub action: MenuAction, pub is_separator: bool,
}
pub struct ContextMenuState {
    pub items: Vec<MenuItem>,
    pub anchor: (usize, usize, usize), // physical px x,y,h, captured at open
    pub region: ContextMenuRegion,
}
```

`ContextMenuTarget` is transient open input: it snapshots clicked group/tab/path and editor selection/clipboard facts needed by the pure builder. It prevents a right click in an unfocused split from deriving availability from the focused group. `items` is durable only for the popup lifetime and is the sole ordering/action authority. `anchor` cannot be re-derived after a raw click; `region` is automation/testing identity. `Messages(Vec<Msg>)` owns target-specific `TabId`/path so activation never retargets the current focused tab.

`CursorOverlayState` separately owns `kind=ContextMenu`, `selected`, `scroll`, and `hover_row`; each is selectable-space indexed. `selected=0` is the degenerate empty/all-disabled value, but means no enabled row. `selectable_items(items)` filters separators and `first_enabled_index` finds the first enabled item or 0.

```text
context_menu.is_some() ⇔ cursor_overlay.kind == ContextMenu
0 ≤ selected < selectable_count, unless selectable_count == 0 where selected = 0
separator ⇒ !enabled && action == None && never selectable
activation(index) = selectable_items(items).nth(index), never items[index]
```

`close_menu` clears both correlated state fields. Replacing items without closing/reconciling is invalid. Menus build once at open, so action and shortcut order remains stable during an interaction.

## Builder, reducer, effects, and ordering

**Current algorithm sketch** — faithful to [src/update/context_menu.rs](../../src/update/context_menu.rs).

```rust
fn open_menu(model: &mut AppModel, target: ContextMenuTarget, anchor: (usize, usize, usize)) -> Option<Cmd> {
    if model.ui.has_modal() { return None; }
    let items = context_menu::build_menu(model, &target);
    let selected = context_menu::first_enabled_index(&items);
    // Clear all sibling cursor-overlay backing state before replacing overlay.
    model.ui.completion_menu = None;
    model.ui.hover_card = None;
    model.ui.reference_list = None;
    model.ui.code_action_list = None;
    let mut overlay = CursorOverlayState::new(CursorOverlayKind::ContextMenu);
    overlay.selected = selected;
    model.ui.cursor_overlay = Some(overlay);
    model.ui.context_menu = Some(ContextMenuState { items, anchor, region: target.region() });
    Some(Cmd::Redraw)
}
fn activate(model: &mut AppModel, index: usize) -> Option<Cmd> {
    let action = model.ui.context_menu.as_ref()
        .and_then(|m| context_menu::selectable_items(&m.items).nth(index))
        .filter(|item| item.enabled)
        .map(|item| item.action.clone());
    let Some(action) = action else { return Some(Cmd::Redraw); };
    close_menu(model);
    match action { MenuAction::Command(id) => execute_command(model, id),
        MenuAction::Messages(messages) => dispatch_in_order(model, messages),
        MenuAction::None => Some(Cmd::Redraw) }
}
```

The region builder establishes order. Editor is Cut/Copy/Paste, separator, LSP navigation/hover, separator, Reveal; selection, clipboard, document path, and LSP registration set enabled state. Tab actions capture clicked `TabId`; file-tree Open/path actions capture tree path. Builders then derive shortcut hints from live keymap/context. This is O(n) construction plus hint strings, once per open, with no I/O. Targeted reveal/copy effects still route through Update.

Runtime right-click resolves `HitTarget` and stores raw `(x,y,0)`; Shift+F10 uses caret height. A modal blocks opening; opening clears completion, hover-card, references, and code-action backing state before it replaces the cursor overlay. Scroll/edit paths dismiss menus and the global update reconciliation cancels scroll animation while one is open. That prevents open-time context acting against moving content.

## Layout, mapping, clipping, and hit test

`view::modal::with_cursor_overlay_spec` projects state to `Anchor::Menu { x,y,h, prefer_below: true, width min=200,max=520 }`, no header/footer, and unlimited V1 visible rows. Shared overlay placement measures content, prefers below, flips above when needed, and clamps to physical window bounds. Hit testing calls `layout_measured` with the render painter's text measure, so hit rectangles are painted rectangles.

Separators transform representation rather than consuming an index:

```text
items:        [Cut, Copy, separator, Goto, separator, separator, Reveal]
selectables:  [Cut, Copy,            Goto,                      Reveal]
flat indices: [ 0,   1,              2,                         3    ]
sections:     [[Cut, Copy], [Goto], [Reveal]]
display rows: Cut, Copy, hairline, Goto, hairline, Reveal
```

`context_menu_rows` pairs selectable items with shortcut chips; more than four chips becomes dim text. `context_menu_sections` drops leading/trailing/empty runs, while overlay layout inserts visible separator boundaries. Hits return `OverlayHit::Row(FlatIndex)` only for action rows and `Inside` for separator/background. Thus pointer flat index 2 activates Goto, never the second raw separator.

**Worked trace.** Anchor `(399,299,0)` in a `400×300` window yields a measured menu between its 200/520-pixel limits (or window-constrained), then clamps to `panel.x + panel.w ≤ 400` and `panel.y + panel.h ≤ 300`. Existing modal tests use `[First, separator, Second]`: display row 0 hits flat 0, separator hits Inside, display row 2 hits flat 1. A long `⌃⌥⇧⌘R` binding exceeds the 200-pixel floor but not 520/window and its expanded painted row remains hit-testable.

## Keyboard, pointer, cancellation, and focus

When ContextMenu overlay is open, runtime input priority handles Up/Down with wrap and disabled skipping, Enter as `ActivateItem { index: selected }`, and Escape dismissal. Any other character or modified key dismisses **and consumes**, so it cannot type into the editor beneath a stale menu. An all-disabled menu has no enabled movement; Enter is a no-op and leaves it open under current reducer rules.

Pointer hover writes `hover_row` in the same flat space and is independent from keyboard selection. Disabled row activation is a no-op that leaves the popup open. A left click away dismisses and swallows its click; a right click away dismisses without swallowing so the next target can open its own menu. This is dismissal policy, not stored OS pointer capture. Token currently has no semantic menu/menuitem bridge, focus tree, submenu, or confirmed app-deactivation dismissal; do not document these as implemented.

## Invalidation, async safety, integration, and tests

Layout is derived each render/hit test from item labels, hints/chips, enabled/selected/hover state, UI font/scale, anchor, and window size. It has no independent layout cache; glyph caches live beneath the painter. Item build is linear and V1 is small. A future unbounded menu needs explicit scroll/virtualization rather than assuming `usize::MAX` visible rows remains cheap.

Builders are synchronous snapshots. A future async menu stores `(menu_instance_id, owner_id, generation)`; only a matching still-open menu accepts a reply, then revalidates stable action identity/enabled state just before dispatch. Closing/opening another menu invalidates old generations. Never apply by stale row index.

Concrete path: runtime hit-test → `ContextMenuMsg::Open { target, anchor }` → update builder/state → overlay spec/layout/render/hit → `ContextMenuMsg::ActivateItem` → normal command/message Update. Automation snapshots `selectable_items`, preserving the same row identity.

| Initial state                                     | event                   | expected result                                        |
| ------------------------------------------------- | ----------------------- | ------------------------------------------------------ |
| Cut disabled, Copy enabled, Paste disabled        | Down from Copy          | Copy remains selection after disabled skip             |
| [First, separator, Second], selected 1            | Enter                   | activates Second; separator is unaddressable           |
| Close(tab 9), focused tab changes to 2 after open | activate                | closes captured tab 9 only                             |
| modal active                                      | right-click file tree   | no menu; click consumed                                |
| menu open                                         | left press outside      | both menu fields clear; underlying action does not run |
| menu open                                         | right press another tab | old state clears, second target opens                  |
| future reply for closed instance                  | apply reply             | discard with no command/redraw                         |

Existing tests cover mapping, clamping, long hints, disabled activation, modal block, completion dismissal, keyboard wrap, Escape, click-away policy, and runtime dispatch. Gallery rows are static presentation; they do not replace these identity/dismissal/revalidation tests.
