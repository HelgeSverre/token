# UI mockup renderer

This directory keeps the visual reference for each component chapter in
[`docs/ui`](../README.md). Every catalogued component has a standalone HTML
page, two 2400 × 1520 PNG renders, and a short managed reference immediately
under its chapter H1. Browse all of them in the generated [visual gallery](index.html),
and use the [style guide](STYLE-GUIDE.md) when composing or reviewing a page.
The [desktop fidelity review](FIDELITY-REVIEW.md) records the initial revisions
and the narrower component-first direction. These are visual targets under
review, not an approved design baseline or proof of native implementation.

Pages and the gallery default to **Emphasised**: the subject keeps its appearance
while everything else fades to 25% opacity over the specimen background by default. Use
the **Normal / Emphasised** toggle to compare. Append `?view=normal` or
`?view=emphasised` to open a particular view, including from `file://` URLs.
There is no duplicate HTML: `subjects.js` identifies each component's elements,
and `emphasis.js` measures them and applies the view without changing layout.
Use the normal view when judging actual scene contrast.

The **Context** slider adjusts the remaining opacity (lower means more dimming).
The **Outline** button enables a neutral dashed inspection guide with an 8px
clear gap around each subject. Guides default to off, including in renders,
and never replace the component's own focus styling. Viewer settings survive a
reload through URL parameters, for example
`BADGE.html?view=emphasised&context=20&outlines=1`.

For generated images, set `--mockup-context-opacity` (0–1),
`--mockup-outline-display` (`none` or `block`), and
`--mockup-outline-offset` (pixels, minimum 4px) in the source CSS. These work on
`:root` or `#specimen`. To force guides off regardless of viewer settings, use
`.mockup-emphasis-outlines { display: none !important; }`. See the
[style guide](STYLE-GUIDE.md#two-views-of-one-source) for the full CSS example.

The source page is a fixed 1200 × 760 CSS-pixel composition. It must expose
exactly one `#specimen`, load the bundled Inter and JetBrains Mono faces, and
set `window.__mockupReady = true` only after its deterministic static state is
ready. The shared stylesheet and script own the Default Dark palette and the
common desktop context. A contextual component such as a scrollbar, tab strip,
or breadcrumb bar belongs in enough surrounding chrome to explain its place;
the screenshot still clips to that same fixed specimen frame. Keep this context
minimal: a full Settings page is usually too much framing for one checkbox.

Install the pinned dependency and Chromium once:

```sh
cd docs/ui/mockups
npm ci
npx playwright install chromium
```

Render everything after changing a page or shared mockup styling:

```sh
npm run render
```

Useful variants:

```sh
npm run render -- --only BREADCRUMBS,TABS
npm run update-docs
npm run determinism
npm run check
npm run check -- --only BADGE,CHECKBOX,SCROLL-AREA --determinism
```

`render.mjs` starts a temporary `127.0.0.1` server rooted at the repository,
blocks every browser request outside that single origin, uses Chromium at
1200 × 760 CSS pixels / device scale factor 2, and closes both browser and
server on success or failure. It fixes dark color scheme, reduced motion,
`en-US`, and UTC; it waits for fonts and `__mockupReady`, then disables
animations, transitions, and caret painting before capture.
Each page is captured in both views, with the viewer toggle excluded using
`capture=1`. Opacity and outline settings come from each page's CSS; interactive
adjustments in another browser tab do not change capture settings.
Normal captures retain the original `renders/COMPONENT.png` name;
emphasised captures use `renders/COMPONENT-emphasised.png`.

The renderer checks source pages, documents, default-theme provenance and every
applied Default Dark CSS token, font loading, page errors, failed requests,
visual frame bounds, accidental interior overflow, PNG dimensions, and visible
subject matches for every registered selector.
`npm run check` is read-only and additionally verifies byte-for-byte PNG
freshness for both variants plus the one managed chapter reference immediately
below each H1. That block shows the emphasised image first and links to the
normal PNG and both HTML views. `npm run determinism` renders both variants of a
fixed subset twice in fresh contexts and prints matching SHA-256 checksums.
`--check --determinism` additionally checks the existing files without rewriting
them. The byte comparison is
meaningful for the same pinned Playwright Chromium and operating system; a
different browser build or platform can legitimately require regeneration. If
Default Dark changes, regenerate the shared theme data before rendering:

```sh
cd ../../..
python3 prototypes/generate-performance-themes.py
```

`catalog.json` is the single list of the 41 component chapters. It deliberately
excludes repository guides, research, crosswalk, roadmap, and component-index
documents; `FOUNDATIONS` stays included because it provides the visual context
for the rest of the catalog. Keep render names uppercase and stable:
`renders/BREADCRUMBS.png` and `renders/BREADCRUMBS-emphasised.png` pair with
`BREADCRUMBS.html` and `../BREADCRUMBS.md`. Subject selectors are maintained in
`subjects.js`; review them whenever a page's composition or states change.
