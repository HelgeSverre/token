# Native UI-polish baseline

This Phase 0 harness renders the checked-in `screenshots/polish/*.yaml` fixtures
through the production CPU `screenshot` binary. It does not use browser mocks or
the user's Token configuration.

## Capture

Build once, then pass that exact binary to the harness:

```sh
cargo build --release --bin screenshot
node scripts/ui-polish-baseline.mjs \
  --binary target/release/screenshot \
  --out-dir target/verification/ui-polish-baseline
```

The default matrix is ten named cases × Default Dark, GitHub Light, and Jake
(the near-black/high-contrast representative) × 1×, 1.5×, and 2×: 90 PNGs.
Scale overrides preserve the scenario's logical window size; physical dimensions
grow with display density, so scaling does not silently shrink the viewport.
To verify fixture compatibility with every embedded theme (discovered from the
binary rather than duplicated in this script):

```sh
node scripts/ui-polish-baseline.mjs --binary target/release/screenshot \
  --out-dir target/verification/ui-polish-all-themes --all-themes
```

Each PNG has a JSON sidecar measured from its rendered `AppModel`: physical and
logical window size, theme and scale, focused file/caret/scroll position,
bundled font names and sizes, line/character/tab/status metrics, and solved
sidebar/editor/dock rectangles. `manifest.json` records the command, platform,
Git commit, binary, fixture and referenced source-file hashes, and PNG hashes. `XDG_CONFIG_HOME` is
redirected beneath the output directory so user themes cannot override built-ins.
Delete the output directory before comparing matrices when fixtures are removed.

## Fixture inventory

- `source-no-workspace`: representative Rust source without a workspace.
- `deep-explorer`: explorer, deep indentation, and a long synthetic path/tab.
- `unicode-wrap`: wrapping, digits, combining marks, wide scripts, and a long tab.
- `right-outline`: a populated right dock.
- `splits`: one split and two editor groups.
- `find-selection-diagnostics`: Find/Replace, multiple selections/cursors,
  diagnostics, and the Problems bottom dock.
- `csv`: the production CSV view with a small in-memory data fixture.
- `tabs-overflow`: real tabs, an edit-created dirty tab, selected last tab, and
  production overflow/reveal behavior.
- `image`: the real image loader and native image viewer state.
- `binary`: the native unsupported-binary placeholder using a bundled font file.

All positions are zero-based. In-memory `content` gives deterministic source;
the path still controls language and displayed labels.

## Explicit gaps

The harness installs tabs and special document state synchronously; it does not
exercise asynchronous runtime file-open commands, filesystem watching, image
load failures, animated images, or interactive image pan/zoom. Binary coverage
is the unsupported-file placeholder, not a hex editor. Inline ghost text is
intentionally absent: storing an inline suggestion does not by itself
prove that the project-visible ghost is correct. Add these only after the
screenshot binary can reproduce their actual native state.

## First surface trial

Document strips have a 32-logical-pixel minimum, retaining the existing
font-driven height for larger fonts. Dock, terminal and preview headers, status
height, code line pitch, fonts and theme colors remain unchanged. Compare the
same fixtures before and after; the metadata reports both compact chrome and
document-strip heights. This is the first Phase 1 slice, not completion of the
broader typography, contrast or pane-chrome plans.
