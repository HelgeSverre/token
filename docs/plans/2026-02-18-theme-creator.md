# Theme Creator Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a custom theme creator to the website's themes page — users can create themes with color pickers, edit hex values, download as YAML, and drag-drop import existing YAML theme files.

**Architecture:** Toggle between "browse mode" (existing behavior) and "edit mode" using a CSS class `.is-editing` on the IDE frame. In edit mode, the detail pane becomes interactive: name/author are text inputs, swatch dots open native color pickers, hex values are editable inputs. A single `COLOR_MAP` table handles bidirectional conversion between the website's flat color keys and the editor's nested YAML structure. Custom themes overlay a base theme's full YAML object so exports include all fields.

**Tech Stack:** Astro (single-page inline script/style), `yaml` npm package (eemeli/yaml) for parsing/serialization, native `<input type="color">` pickers, HTML5 drag-and-drop API.

---

## Task 1: Add `yaml` npm dependency

**Files:**
- Modify: `website/package.json`

**Step 1: Install the dependency**

Run:
```bash
cd website && npm install yaml
```

**Step 2: Verify it installed**

Run: `cd website && npx astro build`
Expected: Build succeeds

**Step 3: Commit**

```bash
git add website/package.json website/package-lock.json
git commit -m "chore: add yaml dependency for theme creator"
```

---

## Task 2: Add `+ New Theme` button to sidebar and custom theme entry

**Files:**
- Modify: `website/src/pages/themes.astro` (HTML sidebar section ~lines 47-60, CSS sidebar section ~lines 352-405)

**Step 1: Add the button and hidden custom entry after the theme list in the sidebar**

After the `{themes.map(...)}` block (line 59), before `</aside>`, add:

```html
<!-- Custom theme entry (hidden until edit mode) -->
<button
  class="sidebar-file sidebar-custom"
  data-theme-id="custom"
  style="display:none"
>
  <svg class="file-icon" width="14" height="14" viewBox="0 0 16 16" fill="none">
    <rect x="2" y="1" width="12" height="14" rx="1.5" stroke="currentColor" stroke-width="1.2"/>
    <path d="M5 5h6M5 8h6M5 11h3" stroke="currentColor" stroke-width="1" stroke-linecap="round"/>
  </svg>
  <span class="file-name">custom-theme.yaml</span>
</button>

<div class="sidebar-spacer"></div>

<!-- New theme button -->
<button class="sidebar-new-theme" id="btn-new-theme">
  <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
    <path d="M8 3v10M3 8h10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
  </svg>
  <span>New Theme</span>
</button>
```

**Step 2: Add CSS for the new button and spacer**

Add to the sidebar CSS section (after `.file-name` styles, around line 405):

```css
.sidebar-spacer {
  flex: 1;
}

.sidebar-new-theme {
  display: flex;
  align-items: center;
  gap: 8px;
  width: calc(100% - 16px);
  margin: 8px;
  padding: 6px 12px;
  border: 1px dashed var(--border);
  border-radius: 4px;
  background: none;
  color: var(--fg-muted);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.sidebar-new-theme:hover {
  color: var(--fg);
  border-color: var(--fg-muted);
  background: color-mix(in srgb, var(--fg) 5%, transparent);
}

/* Hide new theme button in edit mode */
.ide-frame.is-editing .sidebar-new-theme {
  display: none;
}

/* Show custom entry in edit mode */
.ide-frame.is-editing .sidebar-custom {
  display: flex;
}
```

Also update `.sidebar` to add `display: flex; flex-direction: column;` so the spacer pushes the button to the bottom.

**Step 3: Verify build**

Run: `cd website && npx astro build`
Expected: Build succeeds

**Step 4: Commit**

```bash
git add website/src/pages/themes.astro
git commit -m "feat: add New Theme button to sidebar"
```

---

## Task 3: Add COLOR_MAP and YAML conversion utilities to the script

**Files:**
- Modify: `website/src/pages/themes.astro` (script section, ~line 201)

**Step 1: Add the import and mapping table at the top of the script block**

At the top of the `<script>` block, before `document.addEventListener`, add:

```ts
import YAML from 'yaml';

// Bidirectional mapping: flat ThemeData color key → YAML nested path
const COLOR_MAP: Record<string, string> = {
  bg: 'ui.editor.background',
  fg: 'ui.editor.foreground',
  currentLine: 'ui.editor.current_line_background',
  cursor: 'ui.editor.cursor_color',
  selection: 'ui.editor.selection_background',
  gutterBg: 'ui.gutter.background',
  gutterFg: 'ui.gutter.foreground',
  gutterFgActive: 'ui.gutter.foreground_active',
  gutterBorder: 'ui.gutter.border_color',
  sidebarBg: 'ui.sidebar.background',
  statusBg: 'ui.status_bar.background',
  statusFg: 'ui.status_bar.foreground',
  tabBarBg: 'ui.tab_bar.background',
  tabBarActiveBg: 'ui.tab_bar.active_background',
  tabBarActiveFg: 'ui.tab_bar.active_foreground',
  tabBarInactiveBg: 'ui.tab_bar.inactive_background',
  tabBarInactiveFg: 'ui.tab_bar.inactive_foreground',
  tabBarBorder: 'ui.tab_bar.border',
  splitterBg: 'ui.sidebar.border',
  keyword: 'ui.syntax.keyword',
  function: 'ui.syntax.function',
  string: 'ui.syntax.string',
  number: 'ui.syntax.number',
  comment: 'ui.syntax.comment',
  type: 'ui.syntax.type',
  variable: 'ui.syntax.variable',
  operator: 'ui.syntax.operator',
  punctuation: 'ui.syntax.punctuation',
  constant: 'ui.syntax.constant',
  tag: 'ui.syntax.tag',
  attribute: 'ui.syntax.attribute',
};

function getAtPath(obj: any, path: string): string | undefined {
  return path.split('.').reduce((o, k) => o?.[k], obj);
}

function setAtPath(obj: any, path: string, value: string): void {
  const keys = path.split('.');
  let current = obj;
  for (let i = 0; i < keys.length - 1; i++) {
    if (!current[keys[i]]) current[keys[i]] = {};
    current = current[keys[i]];
  }
  current[keys[keys.length - 1]] = value;
}

/** Convert a parsed YAML theme object → flat ThemeData.colors */
function flatColorsFromYaml(yamlObj: any): Record<string, string> {
  const colors: Record<string, string> = {};
  for (const [flatKey, yamlPath] of Object.entries(COLOR_MAP)) {
    const val = getAtPath(yamlObj, yamlPath);
    if (val) colors[flatKey] = val;
  }
  return colors;
}

/** Build a full YAML object by merging flat colors into a base YAML structure */
function buildYamlObject(
  baseYaml: any,
  flatColors: Record<string, string>,
  meta: { name: string; author: string; description: string }
): any {
  // Deep clone base
  const obj = JSON.parse(JSON.stringify(baseYaml));
  obj.name = meta.name;
  obj.author = meta.author;
  obj.description = meta.description;
  for (const [flatKey, yamlPath] of Object.entries(COLOR_MAP)) {
    if (flatColors[flatKey]) {
      setAtPath(obj, yamlPath, flatColors[flatKey]);
    }
  }
  return obj;
}
```

**Step 2: Verify build**

Run: `cd website && npx astro build`
Expected: Build succeeds

**Step 3: Commit**

```bash
git add website/src/pages/themes.astro
git commit -m "feat: add COLOR_MAP and YAML conversion utilities"
```

---

## Task 4: Store full base YAML objects for each built-in theme

**Files:**
- Modify: `website/src/data/themes.ts` — replace the partial `yaml` string snippets with full YAML matching the canonical format

**Step 1: Update each theme's `yaml` field**

For each theme in `themes.ts`, replace the `yaml` field with a full YAML string that matches the real editor's format. Use `fleet-dark.yaml` as the template structure. For fields not exposed in the UI (like `secondary_cursor_color`, `overlay.*`, `csv.*`, sidebar extras), derive sensible defaults from the theme's existing colors.

Example for fleet-dark (it already has a real YAML file at `themes/fleet-dark.yaml` — use that content). For other themes, generate the full structure using the same nesting, filling in the fields we have and deriving the rest.

The `yaml` field should contain everything needed to be a valid `version: 1` theme file.

**Step 2: Verify build**

Run: `cd website && npx astro build`
Expected: Build succeeds

**Step 3: Commit**

```bash
git add website/src/data/themes.ts
git commit -m "feat: store full YAML for all built-in themes"
```

---

## Task 5: Make detail pane interactive in edit mode

**Files:**
- Modify: `website/src/pages/themes.astro` (HTML detail pane ~lines 98-178, CSS detail pane section ~lines 570-670)

**Step 1: Add editable controls to the detail pane header**

Replace the detail header and metadata area (lines 100-105) so both read-only and edit-mode controls coexist:

```html
<div class="detail-header">
  <h2 class="detail-name detail-readonly" id="detail-name">{themes[0].name}</h2>
  <input class="detail-name-input detail-editable" id="detail-name-input"
    type="text" value={themes[0].name} placeholder="Theme name" />
  <span class="detail-badge" id="detail-badge">{themes[0].isLight ? 'light' : 'dark'}</span>
</div>
<input class="detail-author-input detail-editable" id="detail-author-input"
  type="text" value="You" placeholder="Author name" />
<p class="detail-author detail-readonly" id="detail-author">by {themes[0].author}</p>
<p class="detail-desc detail-readonly" id="detail-desc">{themes[0].description}</p>
```

**Step 2: Add base theme dropdown and download button**

After the description paragraph, before the first swatch section, add:

```html
<div class="detail-base-theme detail-editable">
  <label class="detail-label" for="base-theme-select">Based on</label>
  <select id="base-theme-select" class="detail-select">
    {themes.map((t) => (
      <option value={t.id}>{t.name}</option>
    ))}
  </select>
</div>
```

After the last swatch section (closing `</div>` of swatches-syntax), before `</aside>`, add:

```html
<div class="detail-actions detail-editable">
  <button class="btn-download" id="btn-download-yaml">
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
      <path d="M8 2v9M4 8l4 4 4-4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
      <path d="M2 13h12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
    </svg>
    Download YAML
  </button>
  <button class="btn-exit-edit" id="btn-exit-edit">
    ✕ Exit
  </button>
</div>
```

**Step 3: Make each swatch row include a hidden color input and editable hex input**

In each `.swatch-row` template (there are 3 of them in the HTML — Editor, Chrome, Syntax), update the row contents. Replace the pattern:

```html
<span class="swatch-dot" style={`background:${val}`}></span>
<span class="swatch-label">{key}</span>
<span class="swatch-hex">{val}</span>
```

With:

```html
<label class="swatch-dot-label">
  <span class="swatch-dot" style={`background:${val}`}></span>
  <input type="color" class="swatch-color-input" value={val.substring(0, 7)} tabindex="-1" />
</label>
<span class="swatch-label">{key}</span>
<span class="swatch-hex detail-readonly">{val}</span>
<input type="text" class="swatch-hex-input detail-editable" value={val}
  pattern="^#[0-9a-fA-F]{6,8}$" spellcheck="false" />
```

**Step 4: Add CSS for edit mode toggling and new controls**

```css
/* ── Edit mode toggling ──────────────────────────────── */
.detail-editable {
  display: none;
}

.ide-frame.is-editing .detail-editable {
  display: flex;
}

.ide-frame.is-editing .detail-readonly {
  display: none;
}

/* ── Editable inputs ─────────────────────────────────── */
.detail-name-input {
  font-family: var(--font-sans);
  font-size: var(--text-lg);
  font-weight: 700;
  color: var(--fg-bright);
  letter-spacing: -0.02em;
  background: var(--bg-active);
  border: 1px solid var(--border);
  border-radius: var(--r-sm);
  padding: 4px 8px;
  flex: 1;
  min-width: 0;
}

.detail-author-input {
  font-size: var(--text-xs);
  color: var(--fg-muted);
  background: var(--bg-active);
  border: 1px solid var(--border);
  border-radius: var(--r-sm);
  padding: 2px 8px;
  margin-bottom: 16px;
  width: 100%;
  font-family: var(--font-mono);
}

/* ── Base theme dropdown ─────────────────────────────── */
.detail-base-theme {
  display: none;  /* overridden by .is-editing .detail-editable */
  flex-direction: column;
  gap: 4px;
  margin-bottom: 16px;
}

.detail-label {
  font-family: var(--font-mono);
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.1em;
  color: var(--fg-dim);
}

.detail-select {
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  color: var(--fg);
  background: var(--bg-active);
  border: 1px solid var(--border);
  border-radius: var(--r-sm);
  padding: 4px 8px;
  cursor: pointer;
}

/* ── Swatch color picker ─────────────────────────────── */
.swatch-dot-label {
  position: relative;
  cursor: pointer;
  flex-shrink: 0;
}

.swatch-color-input {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  opacity: 0;
  cursor: pointer;
  border: none;
  padding: 0;
}

/* In browse mode, disable clicking through to picker */
.swatch-color-input {
  pointer-events: none;
}
.ide-frame.is-editing .swatch-color-input {
  pointer-events: auto;
}

/* ── Swatch hex input ────────────────────────────────── */
.swatch-hex-input {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--fg-dim);
  text-transform: uppercase;
  background: var(--bg-active);
  border: 1px solid transparent;
  border-radius: var(--r-sm);
  padding: 1px 4px;
  width: 80px;
  transition: border-color var(--duration-fast) var(--ease-out);
}

.swatch-hex-input:focus {
  border-color: var(--border);
  outline: none;
}

/* ── Action buttons ──────────────────────────────────── */
.detail-actions {
  display: none;
  flex-direction: column;
  gap: 8px;
  margin-top: 24px;
  padding-top: 16px;
  border-top: 1px solid var(--border);
}

.btn-download {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 8px 16px;
  background: var(--syn-keyword);
  color: var(--bg);
  border: none;
  border-radius: var(--r-sm);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  font-weight: 600;
  cursor: pointer;
  transition: opacity var(--duration-fast) var(--ease-out);
}

.btn-download:hover {
  opacity: 0.85;
}

.btn-exit-edit {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 6px 16px;
  background: none;
  color: var(--fg-muted);
  border: 1px solid var(--border);
  border-radius: var(--r-sm);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.btn-exit-edit:hover {
  color: var(--fg);
  border-color: var(--fg-muted);
}
```

**Step 5: Verify build**

Run: `cd website && npx astro build`
Expected: Build succeeds

**Step 6: Commit**

```bash
git add website/src/pages/themes.astro
git commit -m "feat: add interactive detail pane controls for edit mode"
```

---

## Task 6: Wire up edit mode state machine and live preview

**Files:**
- Modify: `website/src/pages/themes.astro` (script section)

**Step 1: Add state management and event wiring inside the DOMContentLoaded handler**

After the existing `buttons.forEach(...)` click handler block (~line 298), add:

```ts
// ── Edit mode state ──────────────────────────────────
let editMode = false;
let customColors: Record<string, string> = {};
let baseThemeId = themes[0].id;

const btnNewTheme = document.getElementById('btn-new-theme')!;
const btnDownload = document.getElementById('btn-download-yaml')!;
const btnExitEdit = document.getElementById('btn-exit-edit')!;
const baseThemeSelect = document.getElementById('base-theme-select') as HTMLSelectElement;
const nameInput = document.getElementById('detail-name-input') as HTMLInputElement;
const authorInput = document.getElementById('detail-author-input') as HTMLInputElement;
const customSidebarBtn = document.querySelector('.sidebar-custom') as HTMLButtonElement;

function enterEditMode(fromThemeId?: string) {
  const base = themes.find((t: any) => t.id === (fromThemeId || baseThemeId)) || themes[0];
  baseThemeId = base.id;
  customColors = { ...base.colors };
  baseThemeSelect.value = base.id;
  nameInput.value = 'My Custom Theme';
  authorInput.value = 'You';

  editMode = true;
  ideFrame.classList.add('is-editing');

  // Activate custom sidebar entry
  buttons.forEach(b => b.classList.remove('active'));
  customSidebarBtn.classList.add('active');

  applyCustomTheme();
}

function exitEditMode() {
  editMode = false;
  ideFrame.classList.remove('is-editing');
  customSidebarBtn.classList.remove('active');

  // Re-apply the first theme or whichever was last active
  const fallback = themes.find((t: any) => t.id === baseThemeId) || themes[0];
  applyTheme(fallback);
}

function applyCustomTheme() {
  const customTheme = {
    id: 'custom',
    name: nameInput.value,
    author: authorInput.value,
    description: '',
    fileName: 'custom-theme.yaml',
    isLight: isLightTheme(customColors.bg),
    colors: customColors,
  };
  applyTheme(customTheme);
}

function isLightTheme(bg: string): boolean {
  const hex = bg.replace('#', '');
  const r = parseInt(hex.substring(0, 2), 16);
  const g = parseInt(hex.substring(2, 4), 16);
  const b = parseInt(hex.substring(4, 6), 16);
  return (r * 299 + g * 587 + b * 114) / 1000 > 128;
}

// New Theme button
btnNewTheme.addEventListener('click', () => {
  // Use currently active theme as base
  const activeBtn = document.querySelector('.sidebar-file.active') as HTMLElement;
  const activeId = activeBtn?.dataset.themeId || themes[0].id;
  enterEditMode(activeId);
});

// Exit edit mode
btnExitEdit.addEventListener('click', exitEditMode);

// Base theme dropdown — reset colors from chosen base
baseThemeSelect.addEventListener('change', () => {
  const newBase = themes.find((t: any) => t.id === baseThemeSelect.value);
  if (newBase) {
    baseThemeId = newBase.id;
    customColors = { ...newBase.colors };
    applyCustomTheme();
    // Update swatch displays
    updateAllSwatchInputs();
  }
});

// Clicking a built-in theme exits edit mode
// Modify existing button handler (wrap existing logic):
```

Also **modify the existing button click handler** (around line 293) to exit edit mode:

Replace:
```ts
buttons.forEach(btn => {
  btn.addEventListener('click', () => {
    const theme = themes.find((t: any) => t.id === btn.dataset.themeId);
    if (theme) applyTheme(theme);
  });
});
```

With:
```ts
buttons.forEach(btn => {
  btn.addEventListener('click', () => {
    if (btn.dataset.themeId === 'custom') return; // handled separately
    const theme = themes.find((t: any) => t.id === btn.dataset.themeId);
    if (theme) {
      if (editMode) exitEditMode();
      applyTheme(theme);
    }
  });
});
```

**Step 2: Wire up color picker and hex input events**

Add event delegation on the detail pane for color changes:

```ts
const detailPane = document.getElementById('detail-pane')!;

// Color picker changes
detailPane.addEventListener('input', (e) => {
  if (!editMode) return;
  const target = e.target as HTMLInputElement;

  if (target.classList.contains('swatch-color-input')) {
    const row = target.closest('.swatch-row') as HTMLElement;
    const key = row?.dataset.colorKey;
    if (!key) return;

    // Preserve alpha if the existing value had it
    const existing = customColors[key] || '';
    const alpha = existing.length === 9 ? existing.substring(7) : '';
    customColors[key] = target.value + alpha;

    // Update the sibling displays
    const dot = row.querySelector('.swatch-dot') as HTMLElement;
    const hexInput = row.querySelector('.swatch-hex-input') as HTMLInputElement;
    const hexText = row.querySelector('.swatch-hex') as HTMLElement;
    dot.style.background = customColors[key];
    if (hexInput) hexInput.value = customColors[key];
    if (hexText) hexText.textContent = customColors[key];

    applyCustomTheme();
  }

  if (target.classList.contains('swatch-hex-input')) {
    const row = target.closest('.swatch-row') as HTMLElement;
    const key = row?.dataset.colorKey;
    if (!key) return;

    const val = target.value.trim();
    if (/^#[0-9a-fA-F]{6,8}$/.test(val)) {
      customColors[key] = val;
      const dot = row.querySelector('.swatch-dot') as HTMLElement;
      const colorInput = row.querySelector('.swatch-color-input') as HTMLInputElement;
      dot.style.background = val;
      if (colorInput) colorInput.value = val.substring(0, 7);
      applyCustomTheme();
    }
  }
});

// Name/author changes trigger live update
nameInput.addEventListener('input', () => {
  if (editMode) applyCustomTheme();
});

function updateAllSwatchInputs() {
  [swatchesEditor, swatchesChrome, swatchesSyntax].forEach(container => {
    container.querySelectorAll('.swatch-row').forEach(row => {
      const key = (row as HTMLElement).dataset.colorKey;
      if (!key || !(key in customColors)) return;
      const val = customColors[key];
      const dot = row.querySelector('.swatch-dot') as HTMLElement;
      const hex = row.querySelector('.swatch-hex') as HTMLElement;
      const hexInput = row.querySelector('.swatch-hex-input') as HTMLInputElement;
      const colorInput = row.querySelector('.swatch-color-input') as HTMLInputElement;
      dot.style.background = val;
      if (hex) hex.textContent = val;
      if (hexInput) hexInput.value = val;
      if (colorInput) colorInput.value = val.substring(0, 7);
    });
  });
}
```

**Step 3: Verify build**

Run: `cd website && npx astro build`
Expected: Build succeeds

**Step 4: Commit**

```bash
git add website/src/pages/themes.astro
git commit -m "feat: wire up edit mode state machine and live color editing"
```

---

## Task 7: YAML download

**Files:**
- Modify: `website/src/pages/themes.astro` (script section)

**Step 1: Add download handler after the edit mode wiring**

```ts
btnDownload.addEventListener('click', () => {
  const base = themes.find((t: any) => t.id === baseThemeId) || themes[0];
  const baseYamlObj = YAML.parse(base.yaml);

  const yamlObj = buildYamlObject(baseYamlObj, customColors, {
    name: nameInput.value || 'Custom Theme',
    author: authorInput.value || 'Unknown',
    description: '',
  });

  const yamlStr = YAML.stringify(yamlObj, { indent: 2, lineWidth: 0 });
  const blob = new Blob([yamlStr], { type: 'text/yaml' });
  const url = URL.createObjectURL(blob);

  const a = document.createElement('a');
  a.href = url;
  a.download = slugify(nameInput.value || 'custom-theme') + '.yaml';
  a.click();
  URL.revokeObjectURL(url);
});

function slugify(s: string): string {
  return s.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');
}
```

**Step 2: Verify build**

Run: `cd website && npx astro build`
Expected: Build succeeds

**Step 3: Commit**

```bash
git add website/src/pages/themes.astro
git commit -m "feat: add YAML download for custom themes"
```

---

## Task 8: YAML drag-and-drop import

**Files:**
- Modify: `website/src/pages/themes.astro` (script section, CSS)

**Step 1: Add a drop overlay element in the HTML**

After the `<div id="theme-data" ...>` element, add:

```html
<div class="drop-overlay" id="drop-overlay">
  <div class="drop-overlay-content">
    <svg width="32" height="32" viewBox="0 0 16 16" fill="none">
      <path d="M8 10V2M4 6l4-4 4 4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
      <path d="M2 13h12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
    </svg>
    <span>Drop theme YAML to import</span>
  </div>
</div>
```

**Step 2: Add CSS for the drop overlay**

```css
.drop-overlay {
  display: none;
  position: fixed;
  inset: 0;
  z-index: 1000;
  background: rgba(0, 0, 0, 0.6);
  backdrop-filter: blur(4px);
  align-items: center;
  justify-content: center;
}

.drop-overlay.visible {
  display: flex;
}

.drop-overlay-content {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  color: var(--fg-bright);
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  padding: 40px 60px;
  border: 2px dashed var(--fg-muted);
  border-radius: var(--r-lg);
}
```

**Step 3: Add drag-and-drop handlers in the script**

```ts
const dropOverlay = document.getElementById('drop-overlay')!;
let dragCounter = 0;

document.addEventListener('dragenter', (e) => {
  e.preventDefault();
  dragCounter++;
  if (dragCounter === 1) dropOverlay.classList.add('visible');
});

document.addEventListener('dragleave', (e) => {
  e.preventDefault();
  dragCounter--;
  if (dragCounter === 0) dropOverlay.classList.remove('visible');
});

document.addEventListener('dragover', (e) => {
  e.preventDefault();
});

document.addEventListener('drop', async (e) => {
  e.preventDefault();
  dragCounter = 0;
  dropOverlay.classList.remove('visible');

  const file = e.dataTransfer?.files[0];
  if (!file || !/\.ya?ml$/i.test(file.name)) return;

  try {
    const text = await file.text();
    const yamlObj = YAML.parse(text);

    if (!yamlObj || !yamlObj.ui) {
      console.warn('Invalid theme YAML: missing ui section');
      return;
    }

    // Convert YAML → flat colors, falling back to current base theme
    const base = themes.find((t: any) => t.id === baseThemeId) || themes[0];
    const importedColors = flatColorsFromYaml(yamlObj);
    customColors = { ...base.colors, ...importedColors };

    // Enter edit mode with imported data
    editMode = true;
    ideFrame.classList.add('is-editing');
    buttons.forEach(b => b.classList.remove('active'));
    customSidebarBtn.classList.add('active');

    nameInput.value = yamlObj.name || file.name.replace(/\.ya?ml$/i, '');
    authorInput.value = yamlObj.author || 'Imported';

    updateAllSwatchInputs();
    applyCustomTheme();
  } catch (err) {
    console.error('Failed to parse YAML:', err);
  }
});
```

**Step 4: Verify build**

Run: `cd website && npx astro build`
Expected: Build succeeds

**Step 5: Commit**

```bash
git add website/src/pages/themes.astro
git commit -m "feat: add YAML drag-and-drop import for themes"
```

---

## Task 9: Update themes.ts with full canonical YAML strings

**Files:**
- Modify: `website/src/data/themes.ts`

**Step 1: Update each theme's `yaml` field to contain the full version-1 YAML**

For each theme, generate a complete YAML string that matches the canonical format from `themes/fleet-dark.yaml`. This includes all sections: `version`, `name`, `author`, `description`, `ui.editor`, `ui.gutter`, `ui.status_bar`, `ui.sidebar`, `ui.tab_bar`, `ui.overlay`, `ui.csv`, `ui.syntax`.

For fields not exposed in the UI (like `overlay`, `csv`, `sidebar.selection_*`, `sidebar.hover_*`, etc.), derive reasonable defaults:
- `sidebar.foreground` = `fg`
- `sidebar.selection_background` = `selection` with alpha
- `sidebar.hover_background` = semi-transparent white/black depending on light/dark
- `sidebar.folder_icon` = `function` color
- `sidebar.file_icon` = `variable` color
- `sidebar.border` = `gutterBorder`
- `overlay.border` = `gutterBorder`
- `overlay.background` = `bg` + `E0` alpha
- `overlay.foreground` = `fg`
- `overlay.input_background` = darken/lighten `bg`
- `overlay.selection_background` = `selection`
- `csv.*` = derived from gutter/syntax colors
- `tab_bar.modified_indicator` = `fg`
- `editor.secondary_cursor_color` = `cursor` + `80` alpha
- `syntax.function_builtin` = `function`
- `syntax.variable_builtin` = derived from `variable`
- `syntax.property` = derived from `variable`/`attribute`
- `syntax.escape` = `keyword`
- `syntax.label` = `string`
- `syntax.text*` = `fg`
- `syntax.text_title` = `function`
- `syntax.text_uri` = `string`

**Step 2: Verify build**

Run: `cd website && npx astro build`
Expected: Build succeeds

**Step 3: Commit**

```bash
git add website/src/data/themes.ts
git commit -m "feat: full canonical YAML for all built-in themes"
```

---

## Task 10: Responsive handling and final polish

**Files:**
- Modify: `website/src/pages/themes.astro` (CSS responsive section)

**Step 1: Hide the New Theme button on small screens where detail pane is hidden**

Add to the `@media (max-width: 1024px)` block:

```css
@media (max-width: 1024px) {
  .detail-pane {
    display: none;
  }

  .sidebar-new-theme {
    display: none;
  }
}
```

**Step 2: Verify build**

Run: `cd website && npx astro build`
Expected: Build succeeds

**Step 3: Commit**

```bash
git add website/src/pages/themes.astro
git commit -m "feat: responsive handling for theme creator"
```
