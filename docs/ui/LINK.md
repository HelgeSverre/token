# Link

## Purpose and naming

A **Link** navigates to a destination; it is not a small primary-action button, a terminal string that looks like a URL, or row activation. Distinguish **internal link** (Token route/page), **external link** (validated web URL), and **drop-down link** (secondary action menu). Only the first two have plausible near-term Token use.

IntelliJ uses regular links for in-window navigation, external-link icons for web resources, and Buttons for primary actions; focused links activate with Space. [Link](https://plugins.jetbrains.com/docs/intellij/link.html) is high-confidence primary guidance.

## Current Token contract — high confidence

Token has **no generic Link painter/type**. Terminal hyperlinks are the concrete link-like behavior, but remain terminal-specific.

| Concern    | Current terminal behavior                                                                                                                                                                     |
| ---------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Data       | `TerminalLink { range, uri }` represents OSC-8/scanned `http`/`https` URI and terminal-grid range. It rejects hidden cells, unsafe schemes, malformed URLs, and work over bounded cell count. |
| Hover      | Runtime resolves links only with `⌘` macOS or `Ctrl` elsewhere, in active Terminal dock, with no selection drag; then it sets pointer cursor and `hovered_link`.                              |
| Activation | Primary click resolves again at pointer, never trusts cached hover, then issues `Cmd::OpenWebUrl(uri)`.                                                                                       |
| Paint      | Terminal owns text paint. This is grid-range behavior, not a Label and not reusable Link chrome.                                                                                              |

Palette/list rows and buttons own all other navigation. Token lacks internal route type, link hit rectangle, underline/hover/focus paint, external icon, keyboard activation and accessibility representation.

## Proposed Token contract — proposed, not implemented

Defer until Settings, empty state, or inline help has a real navigation consumer. Terminal URLs do not justify it: they require cell ranges, modifier policy, URL safety and selection precedence.

```text
Link { label, destination: Internal(Route) | External(WebUrl), enabled, visited: optional, icon: None | External | Help }
event: Activate(destination)
```

Owner validates routes/URLs, maps activation to `Msg`/`Cmd`, and owns confirmation/persistence. Painter measures/clips/draws/reports hit geometry only. `WebUrl` above is a proposed validated type, not an existing Token type: the current `Cmd::OpenWebUrl(String)` path checks `util::is_web_url` at the runtime boundary. Preserve that validation when introducing typed destinations.

### Anatomy, geometry, theme, font

A Link is meaningful text phrase, optional destination icon, hover/focus indicator and pointer/focus hit rectangle. Measure with `TextPainter`; clip/ellipsis inside caller line and never activate invisible clipped suffix. Inline help links only the shortest phrase that identifies outcome. Use [Label](LABEL.md) UI typography; add an explicit link foreground plus hover underline/focus outline only when a consumer proves need. Do not silently reuse `overlay.accent` without contrast validation. External gets external-arrow; empty-state help can use help glyph. Non-color-only focus is mandatory.

### Input, focus, accessibility

- Pointer activates only when enabled; press-drag-out cancels.
- Tab order reaches the link. Space activates as IntelliJ specifies; decide and test whether Enter also activates consistently across Token.
- Internal changes Token state only. External dispatches after validated URL and failures use transient/status messaging.
- Future drop-down link is Link plus shared menu/dismissal contract, not [Split Button](SPLIT-BUTTON.md).
- Proposed role/name/state are `link`, visible phrase, destination kind/disabled state. Current accessibility tree is absent.

## Consumers, gallery, acceptance

No generic consumer exists. First use must be a constrained Settings route or context-help external page; Save/destructive/primary work stays [Button](BUTTON.md). Add one gallery family, not files per IntelliJ variant: internal, external-icon, inline-help phrase, hover/focus, disabled, clipped, and drop-down only after a real consumer. Terminal URI behavior stays terminal testing/screenshots.

Acceptance: destination types cannot mix; external icon/cursor are correct; measured text equals hit geometry; pointer/keyboard/dismissal paths tested; unsafe URLs never reach open command; and gallery uses production painter.

## Evidence

- Token: [terminal link detection](../../src/terminal/links.rs), [URL validation](../../src/util/mod.rs), [terminal state](../../src/terminal/mod.rs), [mouse modifier/activation](../../src/runtime/mouse.rs), [runtime cursor update](../../src/runtime/app.rs), [validated browser launch](../../src/runtime/mod.rs).
- Primary: [Link](https://plugins.jetbrains.com/docs/intellij/link.html), [Description Text](https://plugins.jetbrains.com/docs/intellij/description-text.html), [Inline Help Text](https://plugins.jetbrains.com/docs/intellij/inline-help-text.html), [Components](https://plugins.jetbrains.com/docs/intellij/components.html).
