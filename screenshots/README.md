# Screenshot scenarios

Token's screenshot binary creates real, headless editor frames from YAML. It uses
the bundled code/UI fonts, production layout, Tree-sitter, overlays, and terminal
renderer. Do not redraw application chrome or retouch feature states in PNGs.

## Regenerate

From the repository root:

```sh
just screenshots
# A single scenario, using the binary built above:
target/release/screenshot --scenario screenshots/scenarios/showcase-completion.yaml
# Keep experimental/QA renders out of the website:
target/release/screenshot --scenario screenshots/scenarios/showcase-completion.yaml \
  --theme github-light --out-dir target/screenshots
cd website && npm run build
```

Builds stay in `target/`, where `cargo clean` can remove them. The default PNG
destination is `website/src/assets/screenshots/`, not `website/public/`.
`--all` includes QA scenarios too; generating an image does not publish it or add
it to the gallery. Malformed scenarios fail the command instead of leaving a
silently stale image behind.

## Website coverage inventory (2026-09-10)

| Feature | Previous coverage | Scenario used now |
| --- | --- | --- |
| Member completion and side documentation | Missing | `showcase-completion` |
| Hover documentation | Older sample framing | `showcase-documentation`: highlighted header, examples, scrollbar |
| Go signature help | Missing | `showcase-signature-help`: active parameter, light theme |
| Diagnostics and Problems | Missing | `showcase-diagnostics`: squiggles, gutter, overview, grouped messages |
| Multiline inline suggestions | QA text only, not linked | `showcase-ghost-text`: real code, virtual rows, two alternatives |
| Folding and Outline | Missing | `showcase-folding`: three collapsed methods and symbol navigation |
| Go LSP preferences | QA only, not linked | `showcase-language-servers` |
| Terminal tabs and selection | Missing | `showcase-terminal`: server, requests, shell, selected output |
| External-file protection | Generated but not linked | `file-conflict` |
| Docked Find/Replace | Current image, stale “dialog” caption | `showcase-find-replace`, corrected caption |
| Themes, splits, previews, CSV, multi-cursor, Settings | Already covered | Retained and regenerated |

Session restore, auto-save, smooth scrolling, partial suggestion acceptance,
definition/reference jumps, and formatting are interactions best demonstrated in
a recording rather than inferred from a still. These are not claims of missing
application support. A terminal still shows selection and tabs; actually opening
links/copying text still requires interactive verification.

The gallery inventory is `website/src/data/showcase.ts`. Add an imported image and
its title, description, tags and optional shortcut there. `homepageLabel` opts an
entry into the homepage showcase; both pages share the same content. Gallery
filters are derived from the entries, so new feature tags need no separate list.

## Fixture fields

- `files[].path` supplies the language and tab name. `content: |` optionally
  supplies in-memory source instead of reading a file; it never writes that path.
  Cursor, selection, and `collapsed_lines` positions are zero-based character
  coordinates. The last file is the focused split.
- `lsp.completion` contains `query_start_column`, optional `selected`, and native
  LSP `CompletionItem` objects in `items`. `kind: 2` means Method. `documentation`
  can use `{ kind: markdown, value: ... }`. Token's real converter and ranking
  build the menu and side card.
- `lsp.signature_help` uses LSP `SignatureHelp`, including `activeParameter` and
  parameter labels. Native conversion computes the highlighted parameter.
- `lsp.diagnostics` uses LSP `Diagnostic` objects: ranges are zero-based UTF-16,
  severity 1 is Error and 2 is Warning. `lsp.problems: true` opens Problems. A
  hidden workspace (`workspace: { root: ., sidebar_visible: false }`) keeps
  displayed paths relative and portable.
- `lsp: {}` marks the focused file's registered server Ready for a demonstration.
  It does not start a server. Responses are authored fixtures, not recordings of
  any specific server version or model's output.
- `hover: |` supplies Markdown. `inline_suggestion` accepts one string or a list
  of alternatives. Both use the real overlay/ghost renderer and leave source
  untouched; no provider connection or model download is needed.
- `outline: true` opens Outline. Folds and symbols come from the actual syntax
  parser, not a second screenshot-specific implementation.
- `terminal.sessions` holds `{ title, output, selection? }`; `terminal.active`
  selects a tab. Output accepts ANSI sequences and LF line breaks. Selection
  endpoints are visible zero-based terminal cells, with the last cell included.
  The terminal emulator consumes the bytes through an in-memory PTY transport:
  no shell, network request, or sample command is executed.
- A Settings `modal` accepts a `category` label, `input` query, `selected_index`,
  and `scroll_pixels`, applied through the normal Settings update messages.

The native-state fixture test checks the featured surfaces and verifies that
setting them up leaves source text unchanged. Inspect generated images as well:
valid state alone does not guarantee good framing or readable text.

## Verification (2026-09-10)

- `just screenshots`: all 48 scenarios rendered with the release binary. The
  existing HTML-preview scene used the native fallback after Chrome rendering
  failed; it is not a capture of the styled browser preview.
- Website build: passed, with 37 gallery entries and 11 homepage slides. All
  referenced built assets exist, and all scenario PNG dimensions match the YAML.
- Featured images: inspected for cursor placement, overlay framing, readable
  text, and correspondence with their captions.
- Fixture smoke test, strict Clippy, and doctests: passed. The full nextest run
  passed 2,694 tests with six skipped; the unchanged native file-watcher test
  `external_change_watcher_survives_atomic_file_and_parent_replacement` timed out.
- Local preview returned HTTP 200. Interactive website checks were unavailable
  because this session had no browser; a successful build is not browser QA.
