# Link — terminal reality and future generic contract

## Boundary

Token has no generic Link model, painter, focus target, or route type. A link navigates; it is not a compact Button, list-row activation, or terminal string that merely looks URL-shaped. The only implemented link-like behavior is the terminal's grid-aware web link. Its grid coordinates and modifier policy make it non-reusable as generic UI chrome.

Use a future generic link only for internal help/settings navigation or a validated external destination. A primary save/run/destructive operation stays a [Button](BUTTON.md); a link that reveals choices follows [Menu](MENU.md).

## Current implementation: terminal hyperlinks

**Current excerpt** — [src/terminal/links.rs](../../src/terminal/links.rs).

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLink {
    pub range: RangeInclusive<Point>, // inclusive Alacritty grid cells
    pub uri: String,                  // owned http(s) target
}
const MAX_LINK_CELLS: usize = 4096;
pub(super) fn link_at<T: EventListener>(term: &Term<T>, point: Point) -> Option<TerminalLink>;
```

`range` is not a pixel rectangle: it is an inclusive terminal-grid range that can wrap history rows. The result is an owned snapshot, stored transiently as `TerminalState::hovered_link: Option<(session_id, TerminalLink)>`. The terminal grid stays authoritative; hover is never authority to launch. The owned URI avoids borrowing OSC-8 grid metadata across terminal access.

The detector rejects out-of-column/grid points, hidden cells, unsafe OSC-8 schemes, and excessive scans. OSC-8 expands backward/forward up to 4,096 cells while link metadata matches. Plain text bounds a candidate span, uses a cached thread-local `https?` regex, rejects hidden cells, trims terminal punctuation/unmatched delimiters, then validates the URI. `util::is_web_url` rejects whitespace/control characters and accepts only parseable `http`/`https` URLs with a host. This prevents rendered text becoming arbitrary process launch.

### Input, cancellation, and revalidation

```text
pointer move + ⌘ (macOS) / Ctrl (other) + active Terminal dock
  → TerminalViewport::point_inside physical pixels → terminal grid Point
  → session.link_at(point) → terminal.hovered_link → pointer cursor
left click under same modifier → resolve link_at again at click point
  → Cmd::OpenWebUrl(uri) → runtime validates again → background browser launch
```

`runtime/mouse.rs::terminal_link_at_pointer` rejects missing/wrong modifiers, non-terminal targets, selection drag, absent viewport/session, and invalid grid points. Click resolves again, rather than trusting hover: changed session bytes, selection, pointer, or active session cannot launch a stale URI. Runtime resets hover as state changes, and [runtime `open_web_url`](../../src/runtime/mod.rs) repeats URL validation at the effect boundary.

**Worked traces.** In a 16-column terminal, `(https://example.com/a(b)).` wraps; clicking a URL cell returns `https://example.com/a(b)`, not `).`. An OSC-8 `file:///tmp/no` never hovers; `http://localhost:3000/path` does. During a selection drag, the same pointer returns `None`: selection wins. Existing terminal tests cover wrapped ranges, hidden cells, punctuation, and schemes.

Cost is O(k) grid walking plus bounded regex work for k ≤ 4,096; the regex is cached thread-locally. Link results intentionally are not durable cache entries: session bytes, viewport/scrollback, modifier, active panel/session, and drag state invalidate them on every hover/click.

## Proposed generic Link (not implemented)

**Proposed API.** `Route`, `LinkId`, and `WebUrl` do not exist in Token; `WebUrl` must be constructed only after the current URL validation.

```rust
enum LinkDestination<Route> { Internal(Route), External(WebUrl) }
enum LinkIcon { None, External, Help }
struct Link<LinkId, Route> {
    id: LinkId, // stable owner identity, never row index
    label: String, // visible and accessible name
    destination: LinkDestination<Route>,
    enabled: bool,
    icon: LinkIcon,
    focused: bool, // transient projection of the owner's focus manager
    press: Option<PointerId>, // transient capture; never persisted as destination state
}
struct LinkLayout {
    text_hit: Rect, icon_hit: Option<Rect>, // disjoint visible regions; gap is not a hit
    text: Rect, icon: Option<Rect>, visible_text: String,
}
```

The owner durably owns ID/destination/availability; `focused` and `press` are transient owner UI state shown here explicitly so focus/capture have a defined lifetime. Render borrows the control/layout for one frame. `LinkLayout` is derived and invalid when label, UI font/scale, available width, icon, or ellipsis policy changes. Theme changes repaint but should not alter hit geometry. The hit area must equal the visible affordance, never an invisible logical label suffix.

**Layout algorithm sketch.** Measure label/icon in physical UI pixels. Let I be icon width plus gap and A available width. Draw `truncate(label, max(0, A-I))`; set `text_hit` to the measured truncated text and `icon_hit` to the icon rectangle. Hit testing is `text_hit.contains(p) || icon_hit.is_some_and(|r| r.contains(p))`: the visual gap is deliberately not clickable. For A=96, I=16, and a 212-pixel label, truncate/measure text to 80 pixels and place the 16-pixel icon after its gap; the two regions, not a 96-pixel union, are active. At A=12, use an explicit accessible icon-only policy or omit it—never leave an invisible operational rect.

**Reducer sketch.** For this proposal, `LinkId: Copy + Eq` is stable owner identity, `PointerId: Copy + Eq` identifies one pointer sequence, and the returned ID is wrapped in the owner message. `inside` comes from the two hit regions above.

```rust
enum LinkEvent {
    Down { pointer: PointerId, inside: bool },
    Up { pointer: PointerId, inside: bool },
    Cancel { pointer: PointerId },
    FocusLost,
    KeyActivate,
}
fn reduce_link<LinkId: Copy + Eq, Route>(link: &mut Link<LinkId, Route>, event: LinkEvent) -> Option<LinkId> {
    if !link.enabled { link.press = None; return None; }
    match event {
        LinkEvent::Down { pointer, inside: true } => { link.press = Some(pointer); None }
        LinkEvent::Up { pointer, inside } if link.press == Some(pointer) => {
            link.press = None; inside.then_some(link.id)
        }
        LinkEvent::Cancel { pointer } if link.press == Some(pointer) => { link.press = None; None }
        LinkEvent::FocusLost => { link.press = None; None }
        LinkEvent::KeyActivate if link.focused => Some(link.id),
        _ => None,
    }
}
```

Release outside, matching cancel, focus loss, owner removal, and disabled transition clear capture; wrong-pointer release/cancel leaves a live capture intact. Tab/Shift+Tab are the owner focus traversal: they set `focused` only for enabled links. Space/Enter on focus activate once; Escape does not. A future a11y bridge exposes link role/name/disabled/destination and a non-color focus mark.

External Update either constructs validated `WebUrl` or uses the existing runtime boundary; internal Update rechecks that its route still exists. Async destination data carries `(owner_id, generation)` and is discarded unless both still match; resolve `LinkId` at activation rather than stale vector index.

## Integration and verification

A first Settings help consumer calculates `LinkLayout` once for Settings hit test and render, emits `SettingsMsg::OpenHelp(link_id)`, then Update chooses an internal state transition or `Cmd::OpenWebUrl`. It must not feed `TerminalLink` grid ranges into UI layout: their units, identities, and lifetimes differ.

| Initial state                                  | action                   | expected output                         |
| ---------------------------------------------- | ------------------------ | --------------------------------------- |
| OSC-8 `file:///tmp/a`, modifier held           | hover/click              | no hover and no open command            |
| valid terminal link, then session bytes change | click old hover position | newly resolved link only                |
| proposed width 96, icon 16                     | hit text, gap, then icon | text/icon each activate; gap never does |
| proposed capture                               | down inside, up outside  | no route; capture cleared               |
| async generation 4, reply 3                    | apply reply              | discard, no changed destination         |
| disabled focused candidate                     | Tab, Enter               | traversal skips; zero activation        |

Current tests prove terminal URL safety, not generic link keyboard/accessibility behavior. A future gallery is visual coverage only; automation must exercise the proposed transition vectors.
