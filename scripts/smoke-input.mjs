// Run from any directory: node scripts/smoke-input.mjs [path/to/token]
// Uses an isolated native window and fixture/config files under target/.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import net from "node:net";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { setTimeout as delay } from "node:timers/promises";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const binary = path.resolve(
  process.argv[2] ?? path.join(root, "target/release/token"),
);
const base = path.join(root, "target/verification/input-smoke");
await mkdir(base, { recursive: true });
const fixture = await mkdtemp(path.join(base, "run-"));
// Keep the Unix socket path short enough for sockaddr_un on macOS.
const socket = path.join(fixture, "s");
const config = path.join(fixture, "config");
await mkdir(path.join(config, "token-editor"), { recursive: true });
await writeFile(
  path.join(config, "token-editor/config.yaml"),
  `
lsp:
  enabled: false
hover_on_mouse: false
auto_save:
  mode: on_focus_loss
session:
  restore: false
  save_on_exit: false
`,
);
const source = Array.from(
  { length: 80 },
  (_, i) =>
    `fn sample_${i}() { // ${"wide ".repeat(60)}\n    let message = "hello";\n    println!("{message}");\n}\n`,
).join("\n");
await writeFile(path.join(fixture, "scroll.rs"), source);
await writeFile(path.join(fixture, "first.txt"), "first\n");
await writeFile(path.join(fixture, "second.txt"), "second\n");
const child = spawn(
  binary,
  ["--foreground", "--new-window", fixture, path.join(fixture, "scroll.rs")],
  {
    cwd: root,
    env: {
      ...process.env,
      XDG_CONFIG_HOME: config,
      TOKEN_AUTOMATION_SOCKET: socket,
    },
    stdio: ["ignore", "ignore", "pipe"],
  },
);
let stderr = "";
child.stderr.on("data", (chunk) => {
  stderr = (stderr + chunk).slice(-16000);
});
const exited = new Promise((resolve) => {
  child.on("error", (error) => resolve({ error: error.message }));
  child.on("exit", (code, signal) => resolve({ code, signal }));
});

function request(body) {
  return new Promise((resolve, reject) => {
    const client = net.createConnection(socket);
    let data = "";
    client.setTimeout(5000, () =>
      client.destroy(new Error("automation timed out")),
    );
    client.on("error", reject);
    client.on("connect", () => client.write(JSON.stringify(body) + "\n"));
    client.on("data", (chunk) => {
      data += chunk;
      if (!data.includes("\n")) return;
      client.end();
      try {
        const response = JSON.parse(data);
        assert(response.ok, response.message);
        resolve(response.state);
      } catch (error) {
        reject(error);
      }
    });
    client.on("end", () => {
      if (!data.includes("\n"))
        reject(new Error("automation closed without a reply"));
    });
  });
}
const state = () => request({ type: "state" });
const input = (...events) => request({ type: "input", events });
const action = (name) => request({ type: "execute_action", name });
let pointer;
const move = (x, y) => {
  pointer = { kind: "pointer_move", x, y };
  return input(pointer);
};
// Target and action share one dispatch, independent of native pointer motion.
const button = (pressed) =>
  input(pointer, { kind: "pointer_button", button: "left", pressed });
const wheel = (x, y) => input(pointer, { kind: "wheel", x, y, unit: "pixels" });
const focus = (focused) => input({ kind: "focus", focused });
const center = ([x, y, width, height]) => [x + width / 2, y + height / 2];
const checks = [];
async function until(check, label, timeout = 10000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) {
    if (await check()) return;
    await delay(100);
  }
  throw new Error(`Timed out: ${label}`);
}

try {
  await until(
    async () => {
      if (child.exitCode !== null) throw new Error(`Token exited: ${stderr}`);
      try {
        return (await state()).editor_geometry;
      } catch (error) {
        if (error.code === "ENOENT" || error.code === "ECONNREFUSED")
          return false;
        throw error;
      }
    },
    "native window startup",
    60000,
  );
  const initial = await state();
  // Syntax parsing is asynchronous; wait for actual collapse, not a fixed delay.
  await until(
    async () =>
      (await action("CollapseAllFolds")).visual_row_count <
      initial.visual_row_count,
    "fold candidates",
  );
  await action("ToggleFindReplace");
  await request({ type: "set_overlay_input", text: "message" });
  await until(async () => {
    const status = (await state()).overlay?.status;
    return status === "160 matches" || status?.endsWith(" of 160");
  }, "Find results");
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
  await move(...center(geometry.find_bar));
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
    const start = center(scrollbar.thumb);
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
  await move(...center(current.editor_geometry.vertical_scrollbar.thumb));
  const recapture = await button(true);
  assert(
    recapture.scrollbar_dragging,
    JSON.stringify({ before: current, after: recapture }),
  );
  assert(!(await focus(false)).scrollbar_dragging);
  await focus(true);
  checks.push("focus loss releases scrollbar capture");

  current = await request({ type: "set_cursor", line: 0, column: 0 });
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
    await request({
      type: "open_paths",
      paths: [{ path: path.join(fixture, name) }],
      wait: false,
    });
    await request({ type: "insert_text", text: "saved " });
    assert.equal(
      await readFile(path.join(fixture, name), "utf8"),
      name.replace(".txt", "\n"),
    );
  }
  await focus(false);
  await until(async () => !(await state()).modified, "focus-loss save");
  assert.equal(
    await readFile(path.join(fixture, "first.txt"), "utf8"),
    "saved first\n",
  );
  assert.equal(
    await readFile(path.join(fixture, "second.txt"), "utf8"),
    "saved second\n",
  );
  checks.push("focus-loss auto-save writes active and background documents");
  const report = {
    binary,
    fixture,
    checks,
    limitation:
      "Synthetic native-handler input, not OS app switching or physical trackpad delivery.",
  };
  await writeFile(
    path.join(fixture, "report.json"),
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  try {
    await action("Quit");
  } catch {
    /* startup failure or already exited */
  }
  const result = await Promise.race([exited, delay(5000).then(() => null)]);
  if (!result) {
    child.kill();
    process.exitCode = 1;
    console.error("Token did not quit within five seconds");
  } else if (result.error || result.code !== 0) {
    process.exitCode = 1;
    console.error("Token exited abnormally:", result);
  }
  if (stderr) await writeFile(path.join(fixture, "stderr.log"), stderr);
}
