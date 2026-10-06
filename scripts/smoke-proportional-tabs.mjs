import {
  startIsolatedToken,
  createPointer,
  until,
} from "./lib/token-automation.mjs";
import assert from "node:assert/strict";
import { writeFile } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import { setTimeout as delay } from "node:timers/promises";
// Linux/X11 only: point this at a dedicated, empty Xvfb display. Captures include
// its root window; never use a personal desktop. Requires ImageMagick `import`.
const display = process.env.TOKEN_TEST_DISPLAY;
assert(display, "Set TOKEN_TEST_DISPLAY to a dedicated Xvfb display");
const root = process.cwd();
const run = await startIsolatedToken({
  root,
  binary: "target/release/token",
  name: "native-tabs",
  env: { DISPLAY: display, WINIT_UNIX_BACKEND: "x11" },
  configYaml:
    "proportional_tabs: true\nlsp:\n  enabled: false\nsession:\n  restore: false\n  save_on_exit: false\n",
  files: {
    "iiiiiiii.rs": 'fn main() {\n    println!("Native tab trial");\n}\n',
    "WWWWWWWW.rs": "// Wide title\n",
    "café-猫.rs": "// Unicode title\n",
  },
  openFiles: ["iiiiiiii.rs", "WWWWWWWW.rs", "café-猫.rs"],
});
try {
  const state = await run.waitForStartup();
  const pointer = createPointer(run.client);
  const click = async (x, y) => {
    await pointer.move(x, y);
    await pointer.button(true);
    await pointer.button(false);
  };
  const capture = async (name) => {
    await pointer.move(700, 500);
    await delay(300);
    execFileSync(
      "import",
      ["-window", "root", `target/verification/native-tabs-${name}.png`],
      { env: { ...process.env, DISPLAY: display } },
    );
  };
  const reload = async (enabled, font = "Inter") => {
    await writeFile(
      `${run.config}/token-editor/config.yaml`,
      `proportional_tabs: ${enabled}\nui_font: ${font}\nlsp:\n  enabled: false\nsession:\n  restore: false\n  save_on_exit: false\n`,
    );
    await run.client.action("ToggleCommandPalette");
    const palette = await run.client.setOverlayInput("Reload Configuration");
    assert.equal(palette.overlay.rows[0].label, "Reload Configuration");
    await click(400, 179);
    await until(async () => !(await run.client.state()).overlay, {
      label: "reload palette dismissed",
    });
    await delay(300);
    assert.deepEqual(
      (await run.client.state()).editor_geometry,
      state.editor_geometry,
    );
  };
  // At x=335 the trial hits the wide second tab; code-font geometry hits the first.
  await click(335, 16);
  assert.equal((await run.client.state()).document_name, "WWWWWWWW.rs");
  await click(275, 16);
  await capture("after");
  await reload(false);
  await click(335, 16);
  assert.equal((await run.client.state()).document_name, "iiiiiiii.rs");
  await capture("before");
  await reload(true, "JetBrains Mono");
  await click(335, 16);
  assert.equal((await run.client.state()).document_name, "iiiiiiii.rs");
  await reload(true);
  await click(335, 16);
  assert.equal((await run.client.state()).document_name, "WWWWWWWW.rs");
  await click(490, 16);
  assert.equal((await run.client.state()).document_name, "café-猫.rs");
  // Reorder the narrow first tab after the wide second one using native input.
  await pointer.move(275, 16);
  await pointer.dragTo(445, 16);
  await click(275, 16);
  assert.equal((await run.client.state()).document_name, "WWWWWWWW.rs");
  await capture("reordered");
  console.log(
    "PASS: native tab clicks, Unicode title, config toggle, configured font reload, unchanged editor geometry, drag reorder",
  );
} finally {
  await run.stop({ timeoutMs: 60000 });
}
