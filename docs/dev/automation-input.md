# Window-local automation input

The existing automation bridge accepts `input` events through the same window
event handler as winit. It exercises hover dismissal, pointer routing, hit
testing, drag capture, wheel conversion and focus-loss auto-save. It does not
move the system cursor, activate another app, or manufacture OS device events.

Use semantic actions for ordinary commands. Use input when the interaction path
itself matters. Each request accepts 1–64 events, validates them all first, then
dispatches them without interleaving native input. Group a move and its button or
wheel event in one request so real mouse motion cannot retarget the action.
Pin an instance for multi-event sequences; simulated focus gain
also updates Token's most-recently-focused instance bookkeeping.

## CLI and MCP

```sh
target/release/token automate instances
target/release/token automate --instance 12345 state
target/release/token automate --instance 12345 input '[{"kind":"pointer_move","x":400,"y":200},{"kind":"wheel","x":0,"y":-13.5,"unit":"pixels"}]'
target/release/token automate --instance 12345 input '[{"kind":"pointer_move","x":790,"y":120},{"kind":"pointer_button","button":"left","pressed":true}]'
# Move while pressed to drag; then send pressed:false to release.
target/release/token automate --instance 12345 input '[{"kind":"focus","focused":false}]'
```

MCP exposes one `input` tool with `instance` and `events` parameters. The socket
request is `{"type":"input","events":[...]}`. Its response contains the state
after the whole sequence. Supported events are:

- `focus`: `focused` boolean. This can trigger configured auto-save. Send `true`
  to resume the application's focus lifecycle after a simulated loss.
- `pointer_move`: finite `x`, `y` in window-local **physical pixels**, not screen
  coordinates or logical points. Coordinates outside the window are allowed for
  captured drags.
- `pointer_button`: `button` is `left`, `right` or `middle`; `pressed` is a
  boolean. Uses the current pointer position and modifier state, like native
  input. Requires a rendered window.
- `wheel`: finite `x`, `y` with `unit` `pixels` or `lines`. Positive values move
  toward the start of the document; negative values move down/right. Pixel input
  is direct; line input follows discrete-wheel animation. Events use winit's
  moved phase; this is not an OS momentum/gesture simulator.

`state.editor_geometry` describes the focused plain-text pane's content, Find
bar, text start and needed scrollbar tracks/thumbs. Rectangles are
`[x, y, width, height]`, from the same layout and scrollbar state as rendering and
hit testing. Special document tabs return `null`. `scrollbar_dragging` reports
capture; `viewport_pixel_position` and `scroll_animating` report scrolling.
Overlays can occlude the reported editor geometry; normal hit-test priority still
applies. Responses acknowledge dispatch, not completion of asynchronous saves.
Poll state and read the fixture from disk to verify a save.

## Repeatable smoke check

On macOS/Linux with Node installed, run `just smoke-input`. It builds in the
normal `target/` tree and opens an isolated window; do not interact with that
window during the test. To test an already-built package instead:

```sh
node scripts/smoke-input.mjs target/release/bundle/osx/Token.app/Contents/MacOS/token
```

The script checks fractional scrolling with Find/folding, Find-bar wheel routing,
both thumb drags and release, capture cancellation on focus loss, editor hit
testing below Find, and active/background focus-loss saves. Fixtures,
configuration, and a JSON report stay under `target/verification/input-smoke/`.
It closes only its own app process. Real trackpad delivery, OS focus switching
and perceived smoothness still require a physical/native-platform check.
