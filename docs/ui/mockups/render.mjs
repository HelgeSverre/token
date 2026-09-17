#!/usr/bin/env node

/**
 * Render deterministic visual references for docs/ui component chapters.
 *
 * Each source page owns a fixed 1200 × 760 #specimen. This script serves the
 * repository on a temporary loopback port, permits that origin only, and uses
 * one fixed Chromium context for every capture.
 */
import { createHash } from 'node:crypto';
import { createServer } from 'node:http';
import { readFile, stat, mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';
import { chromium } from 'playwright';

const here = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(here, '../../..');
const renderDirectory = path.join(here, 'renders');
const catalog = JSON.parse(await readFile(path.join(here, 'catalog.json'), 'utf8'));
const { cssWidth, cssHeight, deviceScaleFactor } = catalog.viewport;
const pngWidth = cssWidth * deviceScaleFactor;
const pngHeight = cssHeight * deviceScaleFactor;
const timeoutMs = 15_000;

const args = process.argv.slice(2);
const supportedArguments = new Set(['--check', '--update-docs', '--determinism', '--help']);
const usage = `Usage: npm run render -- [--only ID[,ID...]] [--check] [--update-docs] [--determinism]

Without --check, writes PNGs to docs/ui/mockups/renders/.
--check validates every selected source page, its rendered bounds and its existing PNG.
--update-docs writes the idempotent visual-reference block immediately after each H1.
--determinism captures a fixed subset twice and compares their SHA-256 checksums.
--only limits the command to comma-separated catalog IDs.`;

function fail(message) {
  throw new Error(message);
}

function escapeHtml(value) {
  return String(value).replace(/[&<>"']/g, (character) => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
  })[character]);
}

function parseArguments(values) {
  let check = false;
  let updateDocs = false;
  let determinism = false;
  let only = null;

  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
    if (value === '--help') {
      console.log(usage);
      process.exit(0);
    }
    if (value === '--check') check = true;
    else if (value === '--update-docs') updateDocs = true;
    else if (value === '--determinism') determinism = true;
    else if (value === '--only') {
      only = values[index + 1];
      index += 1;
    } else if (value.startsWith('--only=')) {
      only = value.slice('--only='.length);
    } else if (supportedArguments.has(value)) {
      fail(`Unhandled argument ${value}`);
    } else {
      fail(`Unknown argument ${value}\n\n${usage}`);
    }
  }

  if (check && updateDocs) {
    fail('--check does not modify documentation; run --update-docs separately.');
  }
  return { check, updateDocs, determinism, only };
}

const options = parseArguments(args);

function selectComponents() {
  const allIds = new Set(catalog.components.map(({ id }) => id));
  if (!options.only) return catalog.components;
  const requested = options.only.split(',').map((id) => id.trim()).filter(Boolean);
  if (!requested.length) fail('--only needs at least one catalog ID.');
  const invalid = requested.filter((id) => !allIds.has(id));
  if (invalid.length) fail(`Unknown catalog ID(s): ${invalid.join(', ')}`);
  return requested.map((id) => catalog.components.find((component) => component.id === id));
}

function validateCatalog(components) {
  if (cssWidth !== 1200 || cssHeight !== 760 || deviceScaleFactor !== 2) {
    fail('catalog.json must keep the documented 1200×760 CSS viewport at deviceScaleFactor 2.');
  }
  const identifiers = new Set();
  for (const component of components) {
    if (!/^[A-Z][A-Z0-9-]*$/.test(component.id) || component.basename !== component.id) {
      fail(`Invalid catalog identity for ${JSON.stringify(component)}.`);
    }
    if (identifiers.has(component.id)) fail(`Duplicate catalog ID ${component.id}.`);
    identifiers.add(component.id);
  }
}

function visualIndexSource() {
  const cards = catalog.components.map((component, index) => `      <article class="mockup-card">
        <a class="thumbnail" data-mockup-link data-mockup="${component.basename}" href="${component.basename}.html?view=emphasised" aria-label="Open ${escapeHtml(component.title)} mockup">
          <img src="renders/${component.basename}-emphasised.png" data-normal-src="renders/${component.basename}.png" data-emphasised-src="renders/${component.basename}-emphasised.png" width="1200" height="760" alt="${escapeHtml(component.title)} visual target under review"${index === 0 ? ' fetchpriority="high"' : ' loading="lazy"'}>
        </a>
        <div class="card-copy">
          <h2>${escapeHtml(component.title)}</h2>
          <p><a data-mockup-link data-mockup="${component.basename}" href="${component.basename}.html?view=emphasised">Open mockup</a><span aria-hidden="true"> · </span><a href="../${component.basename}.md">Read component reference</a></p>
        </div>
      </article>`).join('\n');
  return `<!doctype html>
<!-- Generated by render.mjs from catalog.json. Do not hand-edit. -->
<html lang="en" data-theme="default-dark">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Token UI mockup gallery</title>
  <link rel="stylesheet" href="shared.css">
  <script src="../../../prototypes/debug-performance-themes.js"></script>
  <script src="shared.js"></script>
  <style>
    html, body { min-inline-size: 0; }
    body { min-block-size: 100vh; overflow-y: auto; }
    .gallery { max-inline-size: 1664px; margin: 0 auto; padding: 48px 48px 72px; }
    .gallery-header { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 20px 32px; align-items: start; margin-block-end: 38px; }
    .gallery-kicker { color: var(--faint); font-size: 10px; line-height: 14px; letter-spacing: .09em; text-transform: uppercase; }
    .gallery h1 { margin-block: 5px 8px; color: var(--bright); font-size: 26px; line-height: 34px; }
    .gallery-description { max-inline-size: 740px; color: var(--muted); font-size: 13px; line-height: 20px; }
    .gallery-nav { display: flex; flex-wrap: wrap; gap: 6px 15px; flex: none; padding-block-start: 17px; color: var(--faint); font-size: 12px; line-height: 18px; }
    .gallery-nav a { color: var(--muted); }
    .gallery-nav a:hover { color: var(--bright); }
    .view-toggle { display: inline-flex; align-items: center; gap: 2px; padding: 2px; border: 1px solid var(--line); border-radius: 5px; background: var(--editor); }
    .view-toggle a { padding: 4px 8px; border-radius: 3px; color: var(--faint); font-size: 11px; line-height: 17px; }
    .view-toggle a[aria-current="true"] { background: var(--control-selected); color: var(--bright); }
    .gallery-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 22px; }
    .mockup-card { min-inline-size: 0; overflow: hidden; border: 1px solid var(--window-border); border-radius: 7px; background: var(--panel); }
    .thumbnail { display: block; background: var(--desk); border-block-end: 1px solid var(--line); }
    .thumbnail img { display: block; inline-size: 100%; block-size: auto; }
    .card-copy { min-block-size: 76px; padding: 13px 15px 14px; }
    .card-copy h2 { overflow: hidden; color: var(--bright); font-size: 13px; line-height: 20px; text-overflow: ellipsis; white-space: nowrap; }
    .card-copy p { margin-block-start: 4px; color: var(--faint); font-size: 11px; line-height: 17px; }
    .card-copy p a { color: var(--muted); }
    .card-copy p a:hover { color: var(--focus); }
    @media (max-width: 1350px) { .gallery-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
    @media (max-width: 720px) {
      .gallery { padding: 28px 20px 44px; }
      .gallery-header { gap: 16px; margin-block-end: 26px; }
      .gallery h1 { font-size: 22px; line-height: 30px; }
      .gallery-nav { inline-size: 100%; padding-block-start: 0; }
      .gallery-grid { grid-template-columns: minmax(0, 1fr); gap: 16px; }
    }
  </style>
</head>
<body>
  <main class="gallery">
    <header class="gallery-header">
      <div>
        <p class="gallery-kicker">Token · UI visual references</p>
        <h1>Component mockup gallery</h1>
        <p class="gallery-description">${catalog.components.length} component studies in Default Dark, under review. Emphasised views keep the component visible and fade its surroundings to 25% by default. Open a mockup to adjust context opacity or enable offset inspection outlines.</p>
      </div>
      <nav class="gallery-nav" aria-label="Mockup resources"><span class="view-toggle" role="group" aria-label="Mockup view"><a data-view-choice="emphasised" href="?view=emphasised">Emphasised</a><a data-view-choice="normal" href="?view=normal">Normal</a></span><a href="STYLE-GUIDE.md">Style guide</a><a href="README.md">Renderer guide</a><a href="../README.md">UI catalog</a></nav>
    </header>
    <section class="gallery-grid" aria-label="Component mockups">
${cards}
    </section>
  </main>
  <script>
    (() => {
      const query = new URLSearchParams(window.location.search);
      const view = query.get('view') === 'normal' ? 'normal' : 'emphasised';
      document.documentElement.dataset.view = view;
      for (const choice of document.querySelectorAll('[data-view-choice]')) {
        choice.setAttribute('aria-current', String(choice.dataset.viewChoice === view));
      }
      for (const image of document.querySelectorAll('img[data-normal-src]')) {
        image.src = image.dataset[view + 'Src'];
      }
      for (const link of document.querySelectorAll('[data-mockup-link]')) {
        link.href = link.dataset.mockup + '.html?view=' + view;
      }
    })();
  </script>
</body>
</html>
`;
}

async function writeVisualIndex() {
  const filePath = path.join(here, 'index.html');
  const expected = visualIndexSource();
  let existing = null;
  try { existing = await readFile(filePath, 'utf8'); } catch { /* create below */ }
  if (existing !== expected) await writeFile(filePath, expected);
}

async function verifyVisualIndex() {
  const filePath = path.join(here, 'index.html');
  let existing;
  try { existing = await readFile(filePath, 'utf8'); } catch { fail('mockup index is missing. Run npm run render.'); }
  if (existing !== visualIndexSource()) {
    fail('mockup index is stale relative to catalog.json. Run npm run render.');
  }
}

function mimeType(filePath) {
  const extension = path.extname(filePath).toLowerCase();
  return new Map([
    ['.css', 'text/css; charset=utf-8'], ['.html', 'text/html; charset=utf-8'],
    ['.js', 'text/javascript; charset=utf-8'], ['.json', 'application/json; charset=utf-8'],
    ['.svg', 'image/svg+xml'], ['.png', 'image/png'], ['.webp', 'image/webp'],
    ['.ttf', 'font/ttf'], ['.woff', 'font/woff'], ['.woff2', 'font/woff2'],
    ['.yaml', 'text/yaml; charset=utf-8'], ['.yml', 'text/yaml; charset=utf-8'],
  ]).get(extension) ?? 'application/octet-stream';
}

function startRepositoryServer() {
  const server = createServer(async (request, response) => {
    try {
      if (!['GET', 'HEAD'].includes(request.method ?? '')) {
        response.writeHead(405, { Allow: 'GET, HEAD' }).end();
        return;
      }
      const url = new URL(request.url ?? '/', 'http://127.0.0.1');
      const requestedPath = decodeURIComponent(url.pathname);
      const resolved = path.resolve(repositoryRoot, `.${requestedPath}`);
      if (resolved !== repositoryRoot && !resolved.startsWith(`${repositoryRoot}${path.sep}`)) {
        response.writeHead(403).end('Forbidden');
        return;
      }
      const details = await stat(resolved);
      if (!details.isFile()) {
        response.writeHead(404).end('Not found');
        return;
      }
      response.writeHead(200, {
        'Cache-Control': 'no-store',
        'Content-Type': mimeType(resolved),
        'X-Content-Type-Options': 'nosniff',
      });
      if (request.method === 'HEAD') response.end();
      else response.end(await readFile(resolved));
    } catch {
      response.writeHead(404).end('Not found');
    }
  });
  return new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', () => {
      const address = server.address();
      if (!address || typeof address === 'string') return reject(new Error('Could not reserve loopback port.'));
      resolve({
        origin: `http://127.0.0.1:${address.port}`,
        close: () => new Promise((done, failClose) => server.close((error) => error ? failClose(error) : done())),
      });
    });
  });
}

async function verifyThemeSource() {
  const generatedPath = path.join(repositoryRoot, 'prototypes/debug-performance-themes.js');
  const source = await readFile(generatedPath, 'utf8');
  const sandbox = { window: {} };
  vm.runInNewContext(source, sandbox, { filename: generatedPath });
  const defaultTheme = sandbox.window.TOKEN_PERFORMANCE_THEMES?.themes?.find(({ id }) => id === 'default-dark');
  if (!defaultTheme?.source || !defaultTheme?.sha256) {
    fail('debug-performance-themes.js lacks default-dark source metadata. Regenerate it with `python3 prototypes/generate-performance-themes.py`.');
  }
  const actual = createHash('sha256').update(await readFile(path.join(repositoryRoot, defaultTheme.source))).digest('hex');
  if (actual !== defaultTheme.sha256) {
    fail(`Default Dark source changed (${defaultTheme.source}); regenerate theme data with \`python3 prototypes/generate-performance-themes.py\` before rendering.`);
  }
}

function pngDimensions(buffer) {
  const signature = '89504e470d0a1a0a';
  if (buffer.subarray(0, 8).toString('hex') !== signature || buffer.subarray(12, 16).toString('ascii') !== 'IHDR') {
    fail('Expected a PNG with an IHDR header.');
  }
  return { width: buffer.readUInt32BE(16), height: buffer.readUInt32BE(20) };
}

function renderPath(component, variant) {
  const suffix = variant === 'emphasised' ? '-emphasised' : '';
  return path.join(renderDirectory, `${component.basename}${suffix}.png`);
}

async function readExistingPng(component, variant) {
  const filePath = renderPath(component, variant);
  let buffer;
  try { buffer = await readFile(filePath); } catch { fail(`${component.id}: missing ${path.relative(repositoryRoot, filePath)} (${variant}). Run npm run render.`); }
  const dimensions = pngDimensions(buffer);
  if (dimensions.width !== pngWidth || dimensions.height !== pngHeight) {
    fail(`${component.id}: expected ${pngWidth}×${pngHeight} PNG, found ${dimensions.width}×${dimensions.height}.`);
  }
  return buffer;
}

function sourceUrl(origin, component, view = 'normal') {
  const query = new URLSearchParams({ view, capture: '1' });
  return `${origin}/docs/ui/mockups/${encodeURIComponent(component.basename)}.html?${query}`;
}

async function inspectPage(page, component) {
  const state = await page.evaluate(({ cssWidth, cssHeight }) => {
    const specimen = document.querySelector('#specimen');
    const fontFamilies = ['Inter', 'JetBrains Mono'];
    return {
      ready: window.__mockupReady === true,
      specimenCount: document.querySelectorAll('#specimen').length,
      specimen: specimen ? (() => {
        const rect = specimen.getBoundingClientRect();
        return { x: rect.x, y: rect.y, width: rect.width, height: rect.height, scrollWidth: specimen.scrollWidth, scrollHeight: specimen.scrollHeight };
      })() : null,
      documentScrollWidth: document.documentElement.scrollWidth,
      documentScrollHeight: document.documentElement.scrollHeight,
      fontsStatus: document.fonts.status,
      fonts: fontFamilies.map((family) => ({
        family,
        loaded: document.fonts.check(`12px "${family}"`),
        faceLoaded: [...document.fonts].some((face) => face.family.replaceAll('"', '') === family && face.status === 'loaded'),
      })),
      theme: {
        selected: document.documentElement.dataset.theme,
        defaultDark: window.TOKEN_PERFORMANCE_THEMES?.themes?.find(({ id }) => id === 'default-dark'),
        fields: window.TOKEN_PERFORMANCE_THEMES?.fields,
      },
      themeTokens: (() => {
        const colors = window.TOKEN_PERFORMANCE_THEMES?.themes?.find(({ id }) => id === 'default-dark')?.colors;
        if (!colors) return [];
        const probe = document.createElement('i');
        probe.setAttribute('aria-hidden', 'true');
        Object.assign(probe.style, { position: 'fixed', visibility: 'hidden', pointerEvents: 'none', inset: '0 auto auto 0' });
        document.body.append(probe);
        const normalize = (color) => {
          probe.style.color = '';
          probe.style.color = color;
          return getComputedStyle(probe).color;
        };
        const rootStyle = getComputedStyle(document.documentElement);
        const tokens = Object.entries(colors).map(([token, expected]) => {
          const applied = rootStyle.getPropertyValue(token).trim();
          return { token, expected, applied, matches: normalize(expected) === normalize(applied) };
        });
        probe.remove();
        return tokens;
      })(),
      interiorOverflow: [...(specimen?.querySelectorAll('*') ?? [])].flatMap((element) => {
        if (element.closest('[data-clip-demo]') || element.hasAttribute('data-allow-overflow')) return [];
        const style = getComputedStyle(element);
        if (style.display === 'inline' || style.display === 'contents') return [];
        const clipped = [style.overflowX, style.overflowY].some((value) => ['auto', 'scroll', 'hidden', 'clip'].includes(value));
        if (clipped) return [];
        const horizontal = element.scrollWidth > element.clientWidth + 1;
        const vertical = element.scrollHeight > element.clientHeight + 1;
        return horizontal || vertical ? [{
          tag: element.tagName.toLowerCase(),
          className: typeof element.className === 'string' ? element.className : (element.getAttribute('class') ?? ''),
          horizontal,
          vertical,
          scrollWidth: element.scrollWidth,
          scrollHeight: element.scrollHeight,
          clientWidth: element.clientWidth,
          clientHeight: element.clientHeight,
        }] : [];
      }),
      cssWidth,
      cssHeight,
    };
  }, { cssWidth, cssHeight });
  if (!state.ready) fail(`${component.id}: page did not set window.__mockupReady = true.`);
  if (state.specimenCount !== 1 || !state.specimen) fail(`${component.id}: expected exactly one #specimen.`);
  const { specimen } = state;
  if (specimen.x !== 0 || specimen.y !== 0 || specimen.width !== cssWidth || specimen.height !== cssHeight) {
    fail(`${component.id}: #specimen must fill the fixed ${cssWidth}×${cssHeight} viewport at (0, 0); found ${JSON.stringify(specimen)}.`);
  }
  if (specimen.scrollWidth > cssWidth || specimen.scrollHeight > cssHeight || state.documentScrollWidth > cssWidth || state.documentScrollHeight > cssHeight) {
    fail(`${component.id}: visual frame overflows its ${cssWidth}×${cssHeight} bounds: ${JSON.stringify(state)}.`);
  }
  if (state.fontsStatus !== 'loaded' || state.fonts.some(({ loaded, faceLoaded }) => !loaded || !faceLoaded)) {
    fail(`${component.id}: bundled Inter and JetBrains Mono fonts did not load: ${JSON.stringify(state.fonts)}.`);
  }
  if (state.theme.selected !== 'default-dark' || !state.theme.defaultDark?.colors || !state.theme.fields) {
    fail(`${component.id}: page must apply generated Default Dark data (set data-theme="default-dark" through shared.js).`);
  }
  const mismatchedTokens = state.themeTokens.filter(({ matches }) => !matches);
  if (mismatchedTokens.length) {
    fail(`${component.id}: applied Default Dark CSS tokens differ from debug-performance-themes.js: ${JSON.stringify(mismatchedTokens)}.`);
  }
  if (state.interiorOverflow.length) {
    fail(`${component.id}: unexpected interior overflow. Add a layout fix, a clipping container, or data-clip-demo for an intentional clipping demonstration: ${JSON.stringify(state.interiorOverflow)}.`);
  }
}

function assertEmphasisInspection(component, inspection, mode) {
  if (!inspection || inspection.mode !== mode || !Number.isFinite(inspection.contextOpacity) || inspection.contextOpacity < 0 || inspection.contextOpacity > 1 || inspection.componentId !== component.id) {
    fail(`${component.id}: invalid emphasis inspection for ${mode}: ${JSON.stringify(inspection)}.`);
  }
  if (!inspection.outline || typeof inspection.outline.enabled !== 'boolean' || !Number.isFinite(inspection.outline.offset) || inspection.outline.offset < 4 || !Number.isFinite(inspection.outline.width) || inspection.outline.width <= 0) {
    fail(`${component.id}: invalid inspection outline settings: ${JSON.stringify(inspection.outline)}.`);
  }
  if (!Array.isArray(inspection.targets) || inspection.targets.length === 0 || !Array.isArray(inspection.missingSelectors) || inspection.missingSelectors.length !== 0) {
    fail(`${component.id}: emphasis subjects are incomplete for ${mode}: ${JSON.stringify(inspection)}.`);
  }
  for (const target of inspection.targets) {
    if (typeof target.selector !== 'string' || !target.selector || !Number.isFinite(target.x) || !Number.isFinite(target.y)
      || !Number.isFinite(target.width) || target.width <= 0 || !Number.isFinite(target.height) || target.height <= 0
      || !Number.isFinite(target.radius) || target.radius < 0) {
      fail(`${component.id}: invalid emphasis target for ${mode}: ${JSON.stringify(target)}.`);
    }
  }
}

async function setEmphasisMode(page, component, mode) {
  const inspection = await page.evaluate((requestedMode) => {
    window.MockupEmphasis.setMode(requestedMode, { updateUrl: false });
    window.MockupEmphasis.refresh();
    return window.MockupEmphasis.inspect();
  }, mode);
  assertEmphasisInspection(component, inspection, mode);
  return inspection;
}

async function capture(browser, origin, component) {
  const diagnostics = [];
  const context = await browser.newContext({
    viewport: { width: cssWidth, height: cssHeight },
    deviceScaleFactor,
    colorScheme: 'dark',
    reducedMotion: 'reduce',
    locale: 'en-US',
    timezoneId: 'UTC',
    serviceWorkers: 'block',
  });
  await context.route('**/*', async (route) => {
    const url = new URL(route.request().url());
    if (url.origin === origin) await route.continue();
    else await route.abort('blockedbyclient');
  });
  const page = await context.newPage();
  page.on('console', (message) => {
    if (message.type() === 'error') diagnostics.push(`console error: ${message.text()}`);
  });
  page.on('pageerror', (error) => diagnostics.push(`page error: ${error.message}`));
  page.on('requestfailed', (request) => {
    if (!request.failure()?.errorText?.includes('ERR_BLOCKED_BY_CLIENT')) diagnostics.push(`failed request: ${request.url()} (${request.failure()?.errorText ?? 'unknown'})`);
  });
  page.on('response', (response) => {
    if (response.status() >= 400) diagnostics.push(`HTTP ${response.status()}: ${response.url()}`);
  });
  try {
    await page.goto(sourceUrl(origin, component), { waitUntil: 'networkidle', timeout: timeoutMs });
    await page.evaluate(() => document.fonts.ready);
    await page.waitForFunction(() => window.__mockupReady === true && window.MockupEmphasis, undefined, { timeout: timeoutMs });
    await page.addStyleTag({ content: '* { animation: none !important; transition: none !important; caret-color: transparent !important; }' });
    await inspectPage(page, component);
    if (diagnostics.length) fail(`${component.id}: ${diagnostics.join('\n')}`);
    const normalInspection = await setEmphasisMode(page, component, 'normal');
    const normal = await page.locator('#specimen').screenshot({ animations: 'disabled' });
    const emphasisedInspection = await setEmphasisMode(page, component, 'emphasised');
    const emphasised = await page.locator('#specimen').screenshot({ animations: 'disabled' });
    for (const [variant, png] of Object.entries({ normal, emphasised })) {
      const dimensions = pngDimensions(png);
      if (dimensions.width !== pngWidth || dimensions.height !== pngHeight) {
        fail(`${component.id}: ${variant} screenshot expected ${pngWidth}×${pngHeight}, found ${dimensions.width}×${dimensions.height}.`);
      }
    }
    return { normal, emphasised, normalInspection, emphasisedInspection };
  } finally {
    await context.close();
  }
}

function managedBlock(component) {
  return `<!-- token-ui-mockup:begin ${component.id} -->\n[![Visual target under review: ${component.title}](mockups/renders/${component.basename}-emphasised.png)](mockups/${component.basename}.html?view=emphasised)\n\n*Visual target under review. [Normal PNG](mockups/renders/${component.basename}.png) · [Open normal mockup](mockups/${component.basename}.html?view=normal) · [Open emphasised mockup](mockups/${component.basename}.html?view=emphasised).*\n<!-- token-ui-mockup:end ${component.id} -->`;
}

function managedBlockPattern(component) {
  return new RegExp(`(?:^|\\n)<!-- token-ui-mockup:begin ${component.id} -->\\n[\\s\\S]*?\\n<!-- token-ui-mockup:end ${component.id} -->(?=\\n|$)`, 'g');
}

function topLevelHeading(source, component) {
  const heading = source.match(/^# [^\n]+/m);
  if (!heading || heading.index === undefined) fail(`${component.id}: ${component.doc} needs a top-level H1 before its visual reference.`);
  return { text: heading[0], index: heading.index, end: heading.index + heading[0].length };
}

function updateDocumentationSource(source, component) {
  const withoutExisting = source.replace(managedBlockPattern(component), '');
  const heading = topLevelHeading(withoutExisting, component);
  const before = withoutExisting.slice(0, heading.end);
  const body = withoutExisting.slice(heading.end).replace(/^\n+/, '').replace(/\n*$/, '');
  return `${before}\n\n${managedBlock(component)}${body ? `\n\n${body}\n` : '\n'}`;
}

async function updateDocumentation(component) {
  const docPath = path.resolve(here, component.doc);
  const source = await readFile(docPath, 'utf8');
  const updated = updateDocumentationSource(source, component);
  if (updated !== source) await writeFile(docPath, updated);
}

async function verifyDocumentationReference(component) {
  const source = await readFile(path.resolve(here, component.doc), 'utf8');
  const block = managedBlock(component);
  const begin = `<!-- token-ui-mockup:begin ${component.id} -->`;
  const end = `<!-- token-ui-mockup:end ${component.id} -->`;
  const beginCount = source.split(begin).length - 1;
  const endCount = source.split(end).length - 1;
  const heading = topLevelHeading(source, component);
  const expectedStart = heading.end + 2;
  if (beginCount !== 1 || endCount !== 1 || source.indexOf(block) !== expectedStart) {
    fail(`${component.id}: ${component.doc} needs exactly one current visual-reference block immediately after its H1. Run npm run update-docs.`);
  }
}

async function main() {
  const components = selectComponents();
  validateCatalog(catalog.components);
  await verifyThemeSource();
  for (const component of components) {
    for (const filePath of [path.join(here, `${component.basename}.html`), path.resolve(here, component.doc)]) {
      try { await stat(filePath); } catch { fail(`${component.id}: missing ${path.relative(repositoryRoot, filePath)}.`); }
    }
  }

  if (options.check) await verifyVisualIndex();
  else await writeVisualIndex();

  const server = await startRepositoryServer();
  let browser;
  try {
    browser = await chromium.launch({ headless: true });
    if (!options.check) {
      await mkdir(renderDirectory, { recursive: true });
    }
    const captured = new Map();
    for (const component of components) {
      const fresh = await capture(browser, server.origin, component);
      captured.set(component.id, fresh);
      if (options.check) {
        for (const variant of ['normal', 'emphasised']) {
          const existing = await readExistingPng(component, variant);
          if (!fresh[variant].equals(existing)) {
            fail(`${component.id}: ${path.relative(repositoryRoot, renderPath(component, variant))} is stale. Run npm run render.`);
          }
        }
        await verifyDocumentationReference(component);
        console.log(`checked ${component.id}`);
      } else {
        await writeFile(renderPath(component, 'normal'), fresh.normal);
        await writeFile(renderPath(component, 'emphasised'), fresh.emphasised);
        if (options.updateDocs) await updateDocumentation(component);
        const normalHash = createHash('sha256').update(fresh.normal).digest('hex');
        const emphasisedHash = createHash('sha256').update(fresh.emphasised).digest('hex');
        console.log(`rendered ${component.id} normal=${normalHash} emphasised=${emphasisedHash}`);
      }
    }
    if (options.determinism) {
      for (const component of components.slice(0, Math.min(3, components.length))) {
        const repeat = await capture(browser, server.origin, component);
        for (const variant of ['normal', 'emphasised']) {
          const firstHash = createHash('sha256').update(captured.get(component.id)[variant]).digest('hex');
          const repeatHash = createHash('sha256').update(repeat[variant]).digest('hex');
          if (firstHash !== repeatHash) {
            fail(`${component.id}: non-deterministic ${variant} screenshot (${firstHash} != ${repeatHash}).`);
          }
          console.log(`deterministic ${component.id} ${variant} ${repeatHash}`);
        }
      }
    }
  } finally {
    try {
      await browser?.close();
    } finally {
      await server.close();
    }
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(`mockup renderer failed: ${error.message}`);
    process.exitCode = 1;
  });
}

export { managedBlock, updateDocumentationSource, visualIndexSource, writeVisualIndex };
