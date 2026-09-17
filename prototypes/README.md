# UI prototypes

## Performance

Open [debug-performance.html](debug-performance.html) directly in a browser.
This explores a performance panel in the right dock, with a floating comparison,
resizable divider, frame-budget chart, stage sparklines, and glyph-cache summary.

- Change workloads, 60/120 Hz budgets, and 10/30/60-second history windows.
- Switch between all 15 repository themes and the original custom study palette.
  `Study (built-in theme)` applies `themes/study.yaml`; `Original study (custom)`
  keeps the initial visual experiment available for comparison. Theme selection
  is remembered locally; `?theme=github-light` overrides it. `?theme=study`
  remains the original study, while `?theme=builtin-study` selects the native
  theme-backed version.
- Compare Study typography with Token defaults independently of colors, using
  `?typography=native` for the latter. This reproduces key font roles and sizes,
  while retaining browser rasterization and the proposed panel layout.
- Pause/resume, clear history, generate a new seed, and expand the stage list.
- F2 toggles the panel. The focused divider supports arrow keys, Home, and End.
- Each series uses independently seeded Perlin noise with different frequencies
  and delays. Shared workload bursts connect stage activity; positive skew and
  occasional spikes approximate timing distributions. Values are illustrative,
  not calibrated measurements. Totals, percentiles, and cache ratios are derived
  from the simulated samples.
- Reduced-motion preferences start the capture paused. Background tabs and
  closed panels suspend animation. All state stays in the browser tab.

The HTML uses CSS layout as a visual reference for a future native panel. It does
not run the Rust Clay-inspired layout engine or change the application.

The [component decision record](../docs/ui/PROTOTYPE-COMPONENTS.md) names the
proposed breadcrumbs, pane headers/footers, dockable panels, and deferred rails.
The [native Performance plan](../docs/feature/performance-panel.md) omits the
demo's live/pause/reload row and decorative header icon; those remain in this
study as simulation controls. The [editor polish plan](../docs/feature/editor-visual-polish.md)
covers typography, tabs, and status-bar dimensions. The breadcrumb strip now
uses a subtle theme-aware bottom border.

See [the visual fidelity investigation](debug-performance-fidelity.md) for the
native comparison, measured typography differences, and limitations. After editing
theme YAML, regenerate the offline palette (requires PyYAML):

```sh
python3 prototypes/generate-performance-themes.py
```

The generated [theme data](debug-performance-themes.js) records theme source paths,
field mappings, and SHA-256 hashes. Keep it beside the HTML when copying the prototype.
See [the Study theme mapping](study-theme-mapping.md) for the native-role inventory
and the intentional choices where the visual study had more surfaces than Token's
theme schema.

## Settings

Open [settings.html](settings.html) directly in a browser. No server, build, or
network connection is needed. The fonts use the repository's existing assets.

Following Zest's `prototypes/` approach, this is a clickable design reference,
not an application implementation. All state is held in the browser tab.

- One list and editor for all language servers. No privileged built-in entries.
- Presets prefill an ordinary draft; every resulting field remains editable.
- Add, edit, disable, remove, validate, save, and discard drafts.
- Persistent Save/Cancel/Remove actions, with advanced fields collapsed initially.
- AI providers use the same list/detail pattern and remain opt-in.
- Preview controls switch between populated, empty, add, and missing-executable
  states. Dark/light themes and narrower browser widths can be compared.

Executable browsing and setup/connection checks are explicitly simulated. The
configuration preview is illustrative, not an export. This does not install
servers, read credentials, launch processes, or write Token's configuration.

Deep links: `?view=lsp`, `?view=add`, `?view=empty`, `?view=missing`, `?view=ai`,
`?view=editor`, optionally with `&theme=light`.
