#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import { basename, join, resolve } from "node:path";
import process from "node:process";

const PRIMARY = ["default-dark", "github-light", "jake"];
const SCALES = [1, 1.5, 2];

function usage(message) {
  if (message) console.error(message);
  console.error("Usage: node scripts/ui-polish-baseline.mjs --binary PATH [--out-dir PATH] [--all-themes] [--ui-tabs]");
  process.exit(message ? 2 : 0);
}

const argv = process.argv.slice(2);
let binary;
let outDir = "target/verification/ui-polish-baseline";
let allThemes = false;
let uiTabs = false;
for (let i = 0; i < argv.length; i++) {
  if (argv[i] === "--binary") binary = argv[++i];
  else if (argv[i] === "--out-dir") outDir = argv[++i];
  else if (argv[i] === "--all-themes") allThemes = true;
  else if (argv[i] === "--ui-tabs") uiTabs = true;
  else if (argv[i] === "--help" || argv[i] === "-h") usage();
  else usage(`Unknown argument: ${argv[i]}`);
}
if (!binary) usage("--binary is required (the harness never guesses which build to use)");

binary = resolve(binary);
outDir = resolve(outDir);
const fixtureDir = resolve("screenshots/polish");
const fixtures = (await readdir(fixtureDir))
  .filter((name) => name.endsWith(".yaml"))
  .sort();
const themes = allThemes
  ? JSON.parse(execFileSync(binary, ["--list-builtin-themes"], { encoding: "utf8" }))
  : PRIMARY;
const configDir = join(outDir, ".isolated-config");
await mkdir(configDir, { recursive: true });

const sha256 = async (path) => createHash("sha256").update(await readFile(path)).digest("hex");
const binarySha256 = await sha256(binary);
const captures = [];

for (const theme of themes) {
  for (const scale of SCALES) {
    const destination = join(outDir, theme, `${scale}x`);
    await mkdir(destination, { recursive: true });
    for (const fixture of fixtures) {
      const fixturePath = join(fixtureDir, fixture);
      console.error(`[${theme} ${scale}x] ${fixture}`);
      execFileSync(binary, [
        "--scenario", fixturePath,
        "--theme", theme,
        "--scale", String(scale),
        "--out-dir", destination,
        "--metadata",
        ...(uiTabs ? ["--ui-tabs"] : []),
      ], {
        stdio: "inherit",
        env: { ...process.env, XDG_CONFIG_HOME: configDir },
      });
      const stem = basename(fixture, ".yaml");
      // Scenario names intentionally carry a polish- prefix.
      const png = join(destination, `screenshot-polish-${stem}.png`);
      const metadata = join(destination, `screenshot-polish-${stem}.json`);
      const { source_files: sourceFiles } = JSON.parse(await readFile(metadata, "utf8"));
      captures.push({
        fixture: fixturePath,
        fixture_sha256: await sha256(fixturePath),
        source_dependencies_sha256: Object.fromEntries(await Promise.all(
          sourceFiles.map(async (path) => [path, await sha256(resolve(path))]),
        )),
        theme, scale, png, metadata,
        png_sha256: await sha256(png),
      });
    }
  }
}

let gitCommit = null;
try { gitCommit = execFileSync("git", ["rev-parse", "HEAD"], { encoding: "utf8" }).trim(); } catch {}
const manifest = {
  schema_version: 1,
  generated_at: new Date().toISOString(),
  command: process.argv,
  binary,
  binary_sha256: binarySha256,
  git_commit: gitCommit,
  platform: { os: process.platform, architecture: process.arch, node: process.version },
  isolated_config_dir: configDir,
  themes,
  scales: SCALES,
  captures,
};
await writeFile(join(outDir, "manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`);
console.error(`Rendered ${captures.length} captures to ${outDir}`);
