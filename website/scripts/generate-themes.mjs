#!/usr/bin/env node

// Reads ../themes/*.yaml and generates src/data/themes.ts
// Run automatically via npm prebuild hook

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const THEMES_DIR = path.resolve(__dirname, "../../themes");
const OUT_FILE = path.resolve(__dirname, "../src/data/themes.ts");

// YAML key path -> flat color key used by the website
const COLOR_MAP = {
  bg: "ui.editor.background",
  fg: "ui.editor.foreground",
  currentLine: "ui.editor.current_line_background",
  cursor: "ui.editor.cursor_color",
  selection: "ui.editor.selection_background",
  gutterBg: "ui.gutter.background",
  gutterFg: "ui.gutter.foreground",
  gutterFgActive: "ui.gutter.foreground_active",
  gutterBorder: "ui.gutter.border_color",
  sidebarBg: "ui.sidebar.background",
  statusBg: "ui.status_bar.background",
  statusFg: "ui.status_bar.foreground",
  tabBarBg: "ui.tab_bar.background",
  tabBarActiveBg: "ui.tab_bar.active_background",
  tabBarActiveFg: "ui.tab_bar.active_foreground",
  tabBarInactiveBg: "ui.tab_bar.inactive_background",
  tabBarInactiveFg: "ui.tab_bar.inactive_foreground",
  tabBarBorder: "ui.tab_bar.border",
  splitterBg: "ui.splitter.background",
  punctuation: "ui.syntax.punctuation",
  keyword: "ui.syntax.keyword",
  function: "ui.syntax.function",
  string: "ui.syntax.string",
  number: "ui.syntax.number",
  comment: "ui.syntax.comment",
  type: "ui.syntax.type",
  variable: "ui.syntax.variable",
  operator: "ui.syntax.operator",
  constant: "ui.syntax.constant",
  tag: "ui.syntax.tag",
  attribute: "ui.syntax.attribute",
};

function resolve(obj, dotPath) {
  return dotPath.split(".").reduce((o, k) => o?.[k], obj);
}

function isLight(hex) {
  const h = hex.replace("#", "").slice(0, 6);
  const r = parseInt(h.slice(0, 2), 16);
  const g = parseInt(h.slice(2, 4), 16);
  const b = parseInt(h.slice(4, 6), 16);
  return (r * 299 + g * 587 + b * 114) / 1000 > 128;
}

// Minimal YAML parser — handles only the flat key: "value" structure of theme files
// (nested via indentation). No arrays, anchors, or multi-line strings.
function parseYaml(text) {
  const root = {};
  const stack = [{ indent: -1, obj: root }];

  for (const line of text.split("\n")) {
    if (!line.trim() || line.trim().startsWith("#")) continue;

    const indent = line.search(/\S/);
    const match = line.match(/^(\s*)(\w+):\s*(.*)$/);
    if (!match) continue;

    const [, , key, rawVal] = match;
    const value = rawVal.replace(/^["']|["']$/g, "").trim();

    // Pop stack to find parent at lower indent
    while (stack.length > 1 && stack[stack.length - 1].indent >= indent) {
      stack.pop();
    }
    const parent = stack[stack.length - 1].obj;

    if (value === "") {
      // Section header — create nested object
      parent[key] = {};
      stack.push({ indent, obj: parent[key] });
    } else {
      parent[key] = value;
    }
  }

  return root;
}

// --- Main ---

if (!fs.existsSync(THEMES_DIR)) {
  // On Vercel (or CI without the parent repo), themes/ isn't available.
  // Fall back to the committed themes.ts.
  if (fs.existsSync(OUT_FILE)) {
    console.log(`Themes dir not found (${THEMES_DIR}), using existing ${OUT_FILE}`);
    process.exit(0);
  }
  console.error(`Error: themes dir not found at ${THEMES_DIR} and no existing ${OUT_FILE}`);
  process.exit(1);
}

const files = fs
  .readdirSync(THEMES_DIR)
  .filter((f) => f.endsWith(".yaml"))
  .sort();

const themes = files.map((fileName) => {
  const filePath = path.join(THEMES_DIR, fileName);
  const yaml = fs.readFileSync(filePath, "utf-8");
  const data = parseYaml(yaml);
  const id = path.basename(fileName, ".yaml");

  const colors = {};
  for (const [flatKey, yamlPath] of Object.entries(COLOR_MAP)) {
    colors[flatKey] = resolve(data, yamlPath) ?? "";
  }

  return {
    id,
    name: data.name ?? id,
    author: data.author ?? "Built-in",
    description: data.description ?? "",
    fileName,
    isLight: isLight(colors.bg || "#000000"),
    colors,
    yaml,
  };
});

// Generate TypeScript source
const colorKeys = Object.keys(COLOR_MAP);
const interfaceColors = colorKeys.map((k) => `    ${k}: string;`).join("\n");

let ts = `// AUTO-GENERATED — do not edit manually.
// Source: themes/*.yaml — regenerate with: node scripts/generate-themes.mjs

export interface ThemeData {
  id: string;
  name: string;
  author: string;
  description: string;
  fileName: string;
  isLight: boolean;
  colors: {
${interfaceColors}
  };
  yaml: string;
}

export const themes: ThemeData[] = [\n`;

for (const theme of themes) {
  const colorsStr = Object.entries(theme.colors)
    .map(([k, v]) => `      ${k}: ${JSON.stringify(v)}`)
    .join(",\n");

  ts += `  {
    id: ${JSON.stringify(theme.id)},
    name: ${JSON.stringify(theme.name)},
    author: ${JSON.stringify(theme.author)},
    description: ${JSON.stringify(theme.description)},
    fileName: ${JSON.stringify(theme.fileName)},
    isLight: ${theme.isLight},
    colors: {
${colorsStr},
    },
    yaml: ${JSON.stringify(theme.yaml)},
  },\n`;
}

ts += `];\n`;

fs.mkdirSync(path.dirname(OUT_FILE), { recursive: true });
fs.writeFileSync(OUT_FILE, ts, "utf-8");
console.log(
  `Generated ${OUT_FILE} with ${themes.length} themes from ${THEMES_DIR}`
);
