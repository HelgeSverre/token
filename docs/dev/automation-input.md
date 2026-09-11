# Window-local automation input

The existing automation bridge accepts `input` events through the same window
event handler as winit. It exercises hover dismissal, pointer routing, hit
testing, drag capture, wheel conversion and focus-loss auto-save. It does not
move the system cursor, activate another app, or manufacture OS device events.

## JavaScript helper for isolated native checks

Maintained Node scripts should use
[`scripts/lib/token-automation.mjs`](../../scripts/lib/token-automation.mjs)
instead of copying socket framing, temporary configuration, process lifecycle,
or polling loops. It has no dependencies and intentionally requires either an
explicit socket path or an isolated app it starts itself.

`startIsolatedToken` is the normal choice for macOS/Linux smoke checks. It
creates a fixture, configuration directory, and short Unix socket beneath
`target/verification/<name>/`; therefore it neither changes the user's Token
configuration nor attaches destructive input to a developer's open window.
The lifecycle helper is Unix-only. Use Token's CLI or MCP bridge for Windows
automation until the loopback endpoint gains an equivalent helper.

```js
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  createPointer,
  inputEvent,
  startIsolatedToken,
  until,
} from "./lib/token-automation.mjs";

const repositoryRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);

const run = await startIsolatedToken({
  root: repositoryRoot,
  binary: path.join(repositoryRoot, "target/debug/token"),
  name: "my-check",
  configYaml: `
lsp:
  enabled: false
auto_save:
  mode: on_focus_loss
session:
  restore: false
  save_on_exit: false
`,
  files: { "example.rs": "fn main() {}\n" },
  openFiles: ["example.rs"],
});

try {
  const initial = await run.waitForStartup();
  const { client } = run;
  await client.setCursor(0, 3);
  await client.insertText("pub ");

  const pointer = createPointer(client);
  await pointer.move(initial.editor_geometry.text_start_x + 8, 120);
  await pointer.wheel(0, -13.5); // physical pixel delta
  await client.input([inputEvent.focus(false)]);

  await until(async () => !(await client.state()).modified, {
    label: "focus-loss save",
  });
} finally {
  await run.stop(); // only stops the child that this script started
}
```

The public building blocks are deliberately small:

- `createTokenClient({ socketPath, timeoutMs })` sends one JSON request to one
  explicit Unix socket. `request` returns the full protocol reply;
  `state()`, `document()`, and `actions()` return their respective payloads.
  Mutation conveniences include `insertText`, `setCursor`, `setSelection`,
  `action`, `scroll`, `setOverlayInput`, and `openPaths`.
- `inputEvent` constructs validated focus, pointer, button, wheel, and close
  events. `createPointer(client)` retains a window-local pointer position and
  batches move/button or move/wheel events atomically. `rectCenter` accepts the
  `[x, y, width, height]` rectangles returned in editor geometry.
- `until(check, { label, timeoutMs, intervalMs })` bounds polling and a hung
  check. A check should return `false` while it is still waiting; unexpected
  errors fail immediately rather than being retried invisibly. It cannot cancel
  an already-running check, so keep checks free of delayed destructive work.

All client calls have a finite timeout (five seconds by default), including
`openPaths(paths, { wait: true })`; pick an explicit, finite `timeoutMs` when a
script intentionally waits longer. Automation responses acknowledge dispatch rather
than completion of asynchronous work, so poll state and inspect the isolated
fixture when asserting saves or server work. The helper does not simulate OS
keyboard input, modifiers, application activation, real trackpad momentum, or
system cursor movement.

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
- `close`: requests window closing through the native close-event handler. Dirty
  documents still require Save, Discard or Cancel; it does not force termination.
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
testing below Find, active/background focus-loss saves, and cancelled window
closing followed by an ordinary save. Fixtures,
configuration, and a JSON report stay under `target/verification/input-smoke/`.
It closes only its own app process. Real trackpad delivery, OS focus switching
and perceived smoothness still require a physical/native-platform check.
