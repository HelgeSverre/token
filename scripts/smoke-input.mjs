// Run from any directory: node scripts/smoke-input.mjs [path/to/token]
// Uses an isolated native window and fixture/config files under target/.
import assert from "node:assert/strict";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  createPointer,
  inputEvent,
  rectCenter,
  startIsolatedToken,
  until,
} from "./lib/token-automation.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const binary = path.resolve(
  process.argv[2] ?? path.join(root, "target/release/token"),
);
const configYaml = `
lsp:
  enabled: false
hover_on_mouse: false
auto_save:
  mode: on_focus_loss
session:
  restore: false
  save_on_exit: false
`;
const source = Array.from(
  { length: 80 },
  (_, i) =>
    `fn sample_${i}() { // ${"wide ".repeat(60)}\n    let message = "hello";\n    println!("{message}");\n}\n`,
).join("\n");
const run = await startIsolatedToken({
  root,
  binary,
  name: "input-smoke",
  configYaml,
  files: {
    "scroll.rs": source,
    "first.txt": "first\n",
    "second.txt": "second\n",
  },
  openFiles: ["scroll.rs"],
});
const { child, client, fixture } = run;
const pointer = createPointer(client);
const state = () => client.state();
const action = (name) => client.action(name);
const move = (x, y) => pointer.move(x, y);
const button = (pressed) => pointer.button(pressed);
const wheel = (x, y) => pointer.wheel(x, y);
const focus = (focused) => client.input([inputEvent.focus(focused)]);
const checks = [];

try {
  await run.waitForStartup({
    timeoutMs: 60000,
    ready: (snapshot) => snapshot.editor_geometry,
  });
  const initial = await state();
  // Syntax parsing is asynchronous; wait for actual collapse, not a fixed delay.
  await until(
    async () =>
      (await action("CollapseAllFolds")).visual_row_count <
      initial.visual_row_count,
    { label: "fold candidates" },
  );
  await action("ToggleFindReplace");
  await client.setOverlayInput("message");
  await until(
    async () => {
      const status = (await state()).overlay?.status;
      return status === "160 matches" || status?.endsWith(" of 160");
    },
    { label: "Find results" },
  );
  let current = await state();
  assert(current.editor_geometry.find_bar[3] > 0);
  assert(current.visual_row_count < initial.visual_row_count);
  const geometry = current.editor_geometry;
  await move(geometry.text_start_x + 40, geometry.content[1] + 40);
  for (const pixels of [13.5, 0.25, 1.75, 2.5]) {
    const previous = (await state()).viewport_pixel_position[1];
    current = await wheel(0, -pixels);
    assert(
      Math.abs(current.viewport_pixel_position[1] - previous - pixels) < 0.001,
    );
    assert(!current.scroll_animating);
  }
  checks.push("fractional pixel wheel with Find and folding");
  await move(...rectCenter(geometry.find_bar));
  const beforeFindWheel = current.viewport_pixel_position;
  assert.deepEqual(
    (await wheel(0, -19)).viewport_pixel_position,
    beforeFindWheel,
  );
  checks.push("Find-bar wheel does not move editor");

  for (const [name, axis] of [
    ["vertical_scrollbar", 1],
    ["horizontal_scrollbar", 0],
  ]) {
    current = await state();
    const scrollbar = current.editor_geometry[name];
    assert(scrollbar, `${name} is present`);
    const start = rectCenter(scrollbar.thumb);
    await move(...start);
    assert((await button(true)).scrollbar_dragging, `${name} captured`);
    const end = [...start];
    end[axis] += 37.25;
    const dragged = await move(...end);
    assert(
      dragged.viewport_pixel_position[axis] >
        current.viewport_pixel_position[axis],
    );
    assert(!(await button(false)).scrollbar_dragging);
    end[axis] += 12;
    assert.deepEqual(
      (await move(...end)).viewport_pixel_position,
      dragged.viewport_pixel_position,
    );
    checks.push(`${name} thumb drag, release and post-release motion`);
  }
  current = await state();
  await move(...rectCenter(current.editor_geometry.vertical_scrollbar.thumb));
  const recapture = await button(true);
  assert(
    recapture.scrollbar_dragging,
    JSON.stringify({ before: current, after: recapture }),
  );
  assert(!(await focus(false)).scrollbar_dragging);
  await focus(true);
  checks.push("focus loss releases scrollbar capture");

  current = await client.setCursor(0, 0);
  await move(
    current.editor_geometry.text_start_x + 1,
    current.editor_geometry.content[1] + 1,
  );
  await button(true);
  current = await button(false);
  assert.equal(current.cursor_line, 0);
  assert.equal(current.cursor_column, 0);
  checks.push("editor hit testing below docked Find");

  await focus(true);
  for (const name of ["first.txt", "second.txt"]) {
    await client.openPaths([{ path: path.join(fixture, name) }]);
    await client.insertText("saved ");
    assert.equal(await run.readFixture(name), name.replace(".txt", "\n"));
  }
  await focus(false);
  await until(async () => !(await state()).modified, {
    label: "focus-loss save",
  });
  assert.equal(await run.readFixture("first.txt"), "saved first\n");
  assert.equal(await run.readFixture("second.txt"), "saved second\n");
  checks.push("focus-loss auto-save writes active and background documents");
  await focus(true);
  await client.insertText("keep unsaved ");
  current = await client.input([inputEvent.close()]);
  assert.equal(current.overlay?.context, "unsaved_changes");
  assert.deepEqual(
    current.overlay.rows.map((row) => row.label),
    ["Cancel", "Save", "Discard Changes"],
  );
  assert(current.modified);
  assert.equal(await run.readFixture("second.txt"), "saved second\n");
  // Replacing the confirmation cancels the close intention. A later ordinary
  // save must not unexpectedly close this window when its async reply arrives.
  await action("OpenSettings");
  await action("OpenSettings");
  await action("SaveFile");
  await until(async () => !(await state()).modified, {
    label: "ordinary save after cancelled closing",
  });
  assert.equal(child.exitCode, null);
  assert((await run.readFixture("second.txt")).includes("keep unsaved "));
  checks.push(
    "window close protects dirty text; cancelling the intention allows a later save without exiting",
  );
  const report = {
    binary,
    fixture,
    checks,
    limitation:
      "Synthetic native-handler input, not OS app switching or physical trackpad delivery.",
  };
  await run.writeFixture("report.json", JSON.stringify(report, null, 2) + "\n");
  console.log(JSON.stringify(report, null, 2));
} finally {
  try {
    await run.stop();
  } catch (error) {
    process.exitCode = 1;
    console.error(error);
  }
  if (run.stderr) await run.writeFixture("stderr.log", run.stderr);
}
