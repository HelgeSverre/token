# Editor Configuration Reference

General editor settings for Token.

---

## Configuration File

| Platform | Path                                 |
| -------- | ------------------------------------ |
| macOS    | `~/.config/token-editor/config.yaml` |
| Linux    | `~/.config/token-editor/config.yaml` |
| Windows  | `%APPDATA%\token-editor\config.yaml` |

---

## Settings

Settings is an opaque preferences page. It uses the theme's panel background
color but ignores that color's transparency; other overlays remain theme-driven.

The page uses the standard themed scrollbar: drag its thumb or click the track
to jump. Scrolling leaves the selected setting unchanged and moves continuously
in pixels, including partially visible rows. Trackpad deltas retain their pixel
distance; keyboard navigation reveals the selected row without snapping the
rest of the page to section boundaries.

Open the separate Settings page with `Cmd+,` (`Ctrl+,` on Windows/Linux) or
the **Open Settings** command. Categories appear in the left navigation, with
compact category buttons in small windows. Search filters the selected category.
Use Up/Down to select a row, Left/Right to cycle presets, or click a checkbox or
preset control. Clicking the row label only selects it.
Preset changes save immediately; Escape closes the page without undoing them. The Theme
row opens the existing theme picker.

**Completion** controls the master switch (including manual requests), automatic
menu opening, local-word fallback/mixing, minimum local candidate length, and AI
inline suggestions with their typing delay and line-tail limit. These settings
are independent of hover timing and inlay hints in **Editor**. Turning off the
automatic menu leaves manual completion available; turning off the master switch
disables both dropdown and inline requests. AI suggestions remain off by default
and require a configured provider.

Tab/Shift+Tab cycles categories, including [Keymap](config-keymap.md#editing-in-settings),
or moves between controls while a configuration form is open.
The Keymap category records shortcuts with an explicit Save/Cancel step; its base-preset
chips save immediately. General controls and keymap overrides use separate files.

The file accepts values outside the offered presets. Such values show no active
chip and stay unchanged until you choose a preset. Saves preserve unknown YAML
keys, but not comments or formatting.

The **Language servers** category includes a master switch and a list of editable
servers, plus **+ Add** for custom executables, languages, arguments and root
markers. The master switch applies immediately; each record's enabled checkbox
is applied with **Save**.
A server's switch retains its own preference when the
master switch is Off. Selecting an unchanged choice does not save again.

Executable rows show the saved command. Presets become ordinary editable records
when configuration is initialized or migrated. Their
list entries open an adjacent draft editor for paths, arguments and advanced
JSON/YAML options, with Browse, executable lookup, live status and Open log.
Save saves that server and rebinds documents affected by its language
assignments; Cancel/Escape discards only
unapplied edits. Validation or save failures keep the draft and leave the live
configuration unchanged. Live process-state rows remain read-only. Status updates without
reopening Settings. The existing Language Servers picker remains available.
The Editor category's **Inlay hints** switch controls Sema line-end parameter
annotations (`lsp.inlay_hints`, off by default).

See [Language servers](language-servers.md) for setup, supported executables,
server-specific settings, quieter-editor options, and troubleshooting.

Go files use `gopls` from your `PATH`. No configuration is needed when it is
installed; to override its location, set `lsp.servers.gopls.command` in the
config file. Go projects use the open workspace root, falling back to the
nearest `go.work` or `go.mod` when opening a file outside a workspace.

### Auto-save

Auto-save is off by default. The **Editor** category offers focus-loss, idle,
and combined modes, along with delay presets and an independent formatter switch:

```yaml
auto_save:
  mode: after_delay
  delay_ms: 1000
  format_on_save: false
```

Modes are `off`, `on_focus_loss`, `after_delay`, and `on_focus_loss_and_delay`.
Idle means time since the last edit in each file, including background tabs.
Cursor movement and scrolling do not reset it. Focus loss means leaving the
native window, not switching tabs or panes. Custom YAML delays are preserved;
their effective range is 100 ms through 24 hours.

Auto-save writes modified files with a path, including new named files. It skips
untitled and image/binary documents, known external conflicts, and pending file
operations. CSV saving waits until every pane's cell draft has been committed or
cancelled; losing focus never commits the draft. Automatic formatting is separate
from the manual `format_on_save` setting.

Write failures preserve unsaved contents and show `!` in the tab plus **Save
failed** in the status bar. They do not repeatedly retry the same revision;
new edits or a successful manual save allow saving again. The writer retains
its external-change checks. Auto-save does not recover untitled text and is not
crash recovery.

### Closing unsaved files

Closing an unsaved tab, split group or window asks to **Save**, **Discard Changes**
or **Cancel**. Multiple affected documents offer Save All and Discard All Changes.
Save waits for the disk write to succeed; untitled documents open Save As one at a
time. Cancelling a dialog or a failed write keeps the tabs open. Ordinary Save
also opens Save As for an untitled document.

Closing one view of a document does not prompt if another view keeps its edits.
Uncommitted CSV cell text belongs to its pane and is protected separately: Save
commits it before writing. Conflicting edits to the same cell in two views, or a
cell removed from the source, stop closing and retain the drafts for review.
This confirmation is independent of auto-save; it is not crash recovery.

### Session restore

The **Session** category controls two independent options, both enabled by default:

```yaml
session:
  restore: true
  save_on_exit: true
```

Token remembers saved-file tabs, split direction/ratios, the active tab in each
pane, the focused pane, selections/multiple cursors, soft wrapping and scroll
positions. CSV tabs retain their grid mode and selected cell, not an unfinished
cell edit. Each workspace has its own session; windows without a workspace use
the default session. Moving a workspace creates a new session identity.

`--new` (`-n`) starts empty without restoring. Otherwise explicit command-line
files open after the session and the first successfully opened argument takes
focus; an explicit line/column takes precedence over the saved cursor.
`--new` does not disable saving the new session on exit.

Session JSON lives in `sessions/` beside `config.yaml`: `default.json` for
non-workspace windows and a stable workspace-path hash for each workspace.
Writes atomically replace the previous metadata after queued file saves finish.
Multiple windows for the same workspace share that file; the last window to exit
wins. There is no periodic crash snapshot or session history.

Restoration reads current file contents from disk, skips missing/unreadable files,
collapses empty panes and clamps positions to changed files. Unsaved text and undo
history are **not** recovered. Untitled tabs, terminal sessions, preview panes,
sidebar expansion and window geometry are not persisted by this feature.

Malformed, unsupported or oversized metadata is left untouched rather than
overwritten on exit. Restore/save failures are logged; restore failures also
appear in the status bar. Move the affected JSON file out of `sessions/` to start
saving a fresh session. Metadata is limited to 4 MiB, 512 tabs, 128 panes and
4,096 selections per tab.

### External file changes

`auto_reload: true` (the default) reloads open, unmodified text files when they
change outside Token. Turn off **Editor → Reload external changes** to ask before
reloading even clean buffers. Checks run on filesystem notifications and window
refocus, including files outside the workspace and ignored files.

Unsaved edits are never silently replaced. A `!` in the tab title indicates an
unresolved outside change or deletion. The dialog defaults to **Keep Editing**;
**Reload from Disk** discards local edits, while **Overwrite Disk with My Version**
requires the disk still to match the version the dialog detected. Deleted files
offer **Recreate File with My Version** instead. **Save My Version As…** saves to
another destination through the normal native dialog. Save or the **Resolve
External File Change** command reopens a deferred conflict.

CSV grids retain their mode on reload. Finish or cancel an active cell edit
before resolving a conflict. Unreadable, binary, or oversized replacements leave
the buffer intact and offer Keep Editing / Save As; text reads retain the normal
50 MiB limit. Image and binary-placeholder tabs do not participate in automatic
text reload. These checks are not an atomic lock against concurrent writers.

### Workspace symbol search

With a workspace open and a running language server that supports workspace
symbols, Search Everywhere enables its **Symbols** tab. Select that tab or type
`@` into an empty query. The **All** tab also includes up to five symbol results.
Use arrows/Page Up/Page Down, the mouse wheel, and Enter or a row click to
navigate. Jumps share the normal Back/Forward history and preserve UTF-16 cursor
positions when opening another file.

Search uses the language server's symbol index; it does not launch additional
servers or scan source files itself. Queries debounce for 150 ms and each server
has a five-second response timeout. Partial failures remain visible alongside
usable results. Change the query to retry. Results are deduplicated and limited
to 2,000 rows; queries are limited to 256 characters. Refine the query when a limit
is reported. Servers must return complete file locations; lazy symbol resolution
is not currently supported.

### Usages

Run **Find Usages** at a symbol in a named text file to open a persistent Usages
dock panel. It uses the file's language server and groups results by file; it
does not scan the workspace itself. **Show Usages** keeps the transient popup
behavior. Starting either action supersedes the pending references search.

In the panel, use Up/Down or Page Up/Page Down to select rows, Left/Right to
collapse/expand a file, and Enter to toggle a file or open a location. A single
click selects, a double click opens, and a file's chevron toggles its group.
The mouse wheel scrolls; Escape returns focus to the editor. Results remain
available after navigation or closing the dock; **View: Toggle Usages** reopens
it without rerunning the search.

Results are a snapshot, not a live index. Run Find Usages again after edits.
Changing or closing the source while a search is pending cancels that search;
moving the caret, changing focus or closing the panel does not. Late responses
never reopen the panel or steal focus. Loading, empty results, unavailable
servers, timeouts and the 200-location result limit are shown in the panel.
Previews use unsaved open buffers when available; bounded background reads may
leave other previews blank without preventing navigation.

### `theme`

The active theme ID.

- **Type:** `string`
- **Default:** `"default-dark"`
- **Example:** `theme: "fleet-dark"`

See [config-theme.md](config-theme.md) for available themes and customization.

### `editor_font` and `ui_font`

Font families are configured independently in `config.yaml`:

```yaml
editor_font: "JetBrains Mono"
ui_font: "Inter"
```

These defaults are bundled; no font installation or runtime download is needed.
`editor_font` controls code, terminal grids, file explorer text, tab titles and
all text inputs, including search boxes, Settings search, Find/Replace, symbol
rename and CSV cells. `ui_font` controls non-editable UI text: Settings labels,
menus, buttons and status text. Status-bar size remains
independently configurable with `status_bar_font_size`.

Either setting accepts an installed font-family name. The editor family must
be monospaced. Missing, unreadable or proportional editor fonts fall back to
JetBrains Mono; unavailable UI fonts fall back to Inter. A warning is logged,
and the saved preference is retained. System fonts are discovered in standard
platform font directories; neither setting installs fonts.

Use **Reload Configuration** or restart after editing the file. Font family
selection is currently file-configured, not a font picker in Settings.

### `indent_guides`

Show faint vertical lines at the document's inferred indentation stops. Enabled
by default; toggle **Indent guides** under Settings → Appearance, or set
`indent_guides: false` in `config.yaml`.

Guides follow spaces and expanded tabs, including mixed indentation, and are
clipped to the text viewport. Spacing is inferred from common indentation
increases in a bounded sample of the first 200 lines, falling back to four
columns; tab expansion itself remains four columns. They appear only on the
first visual row of a soft-wrapped line. Whitespace-only lines show their actual indentation; empty
lines do not infer a surrounding scope. CSV, image, binary and terminal views
are unaffected. Set the color through
[`ui.editor.indent_guide`](config-theme.md#editor).

### `cursor_blink_ms`

Cursor blink interval in milliseconds. Set `0` for a steady, non-blinking caret.

- **Type:** `integer`
- **Default:** `600`
- **Example:** `cursor_blink_ms: 500`

### `auto_surround`

Automatically surround selected text when typing an opening bracket or quote character. When enabled, selecting text and typing `(`, `[`, `{`, `"`, `'`, or `` ` `` wraps the selection instead of replacing it (e.g., `hello` → `(hello)`). Works with multi-cursor selections.

- **Type:** `boolean`
- **Default:** `true`
- **Example:** `auto_surround: false`

### `bracket_matching`

Highlight matching brackets when the cursor is adjacent to `(`, `)`, `[`, `]`, `{`, or `}`. Both the bracket under/before the cursor and its matching pair are highlighted with a background color (configurable via the `bracket_match_background` theme color).

- **Type:** `boolean`
- **Default:** `true`
- **Example:** `bracket_matching: false`

---

### Hover documentation

Resting over text in the focused editor opens hover documentation after
`hover_delay_ms` (300 ms by default). Moving within a word keeps that target;
empty space, selections, dragging and other open documentation surfaces do not
start automatic hover. Leaving a mouse card or its target allows 300 ms to move
into the card before it closes.

Show Hover requests documentation immediately at the caret, independently of
`hover_on_mouse`. Incidental pointer movement does not dismiss it. Editing,
moving the caret, scrolling the editor, switching panes, losing focus or pressing
Escape closes documentation and cancels its pending request. Empty automatic
results are silent; an explicit request still reports when information is unavailable.

Completion documentation takes precedence while its menu is open. Signature
help yields visually until the menu closes, and automatic hover waits while
signature help is visible.

Hover and completion documentation share reading controls: scroll over the card,
use Alt+PageUp/PageDown to move a page, or click the footer/press F1 to
expand or collapse it. Long cards show the visible row range in the footer;
short cards omit unnecessary controls. These actions keep the card open and
leave the caret and completion selection unchanged;
long signatures and prose are scrollable rather than cut off. Signature-help
calltips remain non-interactive.

### Completion dropdown

The dropdown uses language-server results for member access such as `builder.`.
Local buffer words and snippets are excluded in that context; they cannot infer
the receiver's methods. The server's ordering is retained, and typing a prefix
filters its results. Selected items can show a signature and documentation.
Badges distinguish methods (`M`), functions (`f`) and modules (`m`).

Documentation cards share Markdown formatting with hover and signature help:
headings and emphasis, literal code examples, numbered/nested lists and task
markers, quoted lines and text tables. Links show their labels; images show alt
text, without fetching resources. HTML stays literal text, and strikethrough is
shown dimmed. Plain-text server documentation is not parsed as Markdown.

The reading controls described above also apply to the completion side card.
Ordinary PageUp/PageDown still navigates the suggestion list. Choosing a
different suggestion resets its documentation view. Cards narrow to fit beside
the menu and stay within the window height; if no text area fits, enlarge the
window to see the card.

For example, `ar_flag` and `archiver` after `cc::Build::new().` are valid builder
methods, not words extracted from your file. Typing `comp` narrows that list to
compiler-related methods. `completion.menu.words` controls local word suggestions
(`fallback`, `enabled` or `disabled`), but does not make them semantic member
completions.

```yaml
completion:
  enabled: true # master switch for dropdown and inline suggestions
  menu:
    enabled: true # automatically open the dropdown while typing
    min_word_length: 3 # candidate identifier length, in characters
    words: fallback # fallback | enabled | disabled
```

`completion.menu.enabled: false` stops new automatic dropdowns, including on
server trigger characters and paths. Ctrl+Space still opens one, and an already
open session continues to filter while typing. Accepting a directory from a
manual path session continues into its children. Signature help and configured
inline suggestions remain available. `completion.enabled: false` retains its
master-switch behavior, disabling both automatic and explicit completion.

`min_word_length` filters local candidate identifiers, not language-server items,
snippets or filenames. It counts characters rather than UTF-8 bytes, defaults to
three and treats zero as one. It does not change the two-character typed-prefix
threshold for automatic word completion. The existing syntax/context restrictions,
scan window and result cap still apply.

Legacy `completion.words` remains readable. An explicitly supplied
`completion.menu.words` wins if both keys exist; otherwise the legacy value is
used, including with a partial `menu` block. Saving writes only the nested key
while preserving unknown settings. Invalid settings are not overwritten by save.

Single-cursor LSP acceptance includes additional edits such as auto-imports,
whether supplied in the initial response or by item resolution. The primary
completion and non-overlapping additional edits form one undo step. If resolution
fails, edits already supplied by the server are retained. Multi-cursor acceptance
currently uses the item's plain-text fallback; absolute additional edits are not
repeated at each cursor.

Typing a commit character explicitly declared by the selected LSP item (or its
server default) accepts that item, then inserts the character. Token does not
guess a punctuation set for local words, snippets or filesystem paths. The result, character and
known auto-imports form one Undo step; single-cursor snippet caret placement is
preserved. Multi-cursor acceptance uses the same plain-text fallback described
above, with the character appended at each completed prefix.

When item resolution is still pending, the character appears immediately. If
you type again, navigate, dismiss the menu, switch panes/focus or change the file,
the late reply cannot replace your newer work. Paste, text/IME insertion messages
and multi-character keyboard payloads do not accept a highlighted menu item.
An ordinary one-character keyboard event can commit; native IME/platform behavior
still needs the separate platform verification matrix.

#### File path suggestions

The dropdown recognizes forward-slash paths such as `./assets/`, `../src/`,
`~/Documents/` and absolute paths. Relative paths use the current file's parent
directory, or the workspace root for an untitled buffer. Without either base,
untitled buffers can still complete absolute and home-relative paths.

Code paths must be inside an ordinary quoted string identified by current syntax; local directory
reads wait for parsing. Plain-text paths and Markdown link destinations also
work. Ctrl+Space can explicitly list a directory inside a string or Markdown
link even without a slash; ordinary prose words and code expressions do not
trigger filesystem suggestions. Comments, regexes, URLs and Markdown fragments/query strings are
excluded. Language-server completions remain available, including import aliases
that have no corresponding local directory, and matching local/server insertions
are deduplicated in favor of the server.

File (`F`) and directory (`/`) badges distinguish entries. Selecting a filesystem
directory inserts its trailing slash and requests its children. Acceptance
replaces the current filename component, including an existing suffix after the
caret, as one Undo step. Multiple carets must have compatible path contexts;
peer panes follow the shared edit transaction. Spaces work inside quoted strings
and are percent-encoded in Markdown links; accented filenames use literal prefix
matching. Hidden names appear only when the component starts with `.`.
Already-complete local filenames are omitted when acceptance would do nothing.

The source reads one directory, never recursively, on a speculative worker that
does not delay saves. It keeps at most one active and one pending request, up to
500 returned names and 1 MiB of filename bytes. Scanning stops after 20,000 entries
or a cooperative 50 ms budget, reporting when results are limited. A blocked OS
filesystem call can exceed that budget, but neither typing nor shutdown waits
for it. No completion data is persisted or sent to an inline provider.

Path recognition is limited to lines of at most 4,096 characters. Escaped source
paths, backslash/UNC forms, angle-bracket Markdown destinations and names requiring
language-specific quoting are omitted; use forward slashes and ordinary Markdown
destinations. Non-UTF-8 names and unsupported filesystem entry types are omitted.
Changing the file, pane, language, focus, selection or revision invalidates old
results; Escape prevents an outstanding reply from reopening the menu.

### `completion.inline` and `completion.providers`

Inline results have an in-memory, per-window cache: at most 256 entries and
8 MiB of retained source/result payload, plus bounded entry metadata. Matching
automatic requests can replay after backspace/retype, or reuse a longer
alternative's remainder after typing or acceptance. The full bounded prefix,
suffix, document/file, language and provider configuration must match. Request
revisions are refreshed and normal visibility/staleness checks still apply.
The ordinary debounce is unchanged. An explicit “Trigger Inline Suggestion”
request bypasses reuse and asks the backend again. Entries are evicted by recent
use and disappear when the window's worker exits; nothing is persisted.

After cache lookup, inline results receive syntax-aware bracket and indentation
checks. Recognized literals/comments are left alone; an impossible closing
bracket truncates the candidate. In Rust, Go, JavaScript, C and C++, leading
tabs/spaces follow the document's majority style without changing visual columns.
Other languages and mixed/tied styles keep their indentation unchanged; visual
tab stops must not rewrite indentation-sensitive syntax. This is not a formatter
or a semantic correctness guarantee.

Analysis uses a local document snapshot, never added to the provider's HTTP
request or retained in the result cache. Documents and individual candidates
over 1 MiB, unsupported syntax, ambiguous recovery involving quotes/slashes,
and exhausted analysis budgets preserve the original result. Alternatives share
a cooperative 50 ms parsing/traversal budget on the existing worker. Exact cache
hits are checked against fresh local context; partial replay matches the
normalized text that was shown, including tabs/spaces.

Ghost-text suggestions are off by default. In **Settings → AI completion**, choose
**+ Add**, enter a unique name, and select your service's transport.
Select an existing provider from the list to edit it alongside the list.

The form exposes the base URL, model, API-key environment-variable name (never
the key itself), token/request limits, prompt format, extra source context, and
managed local llama-server options. **Save** updates that provider without
changing the current selection; **Save & Use** also selects it for inline
completion. Neither action turns suggestions on: enable **AI inline suggestions**
separately. Extra context and managed process startup remain opt-in. Expand
**Advanced** for generation limits and local-process options. Cancel restores
the saved record without applying the draft.

Validation checks configuration, not connectivity, installed files, or whether
the credential variable is currently set. Install servers/models yourself and
ensure the variable is available to the Token process. **Remove** requires
confirmation; removing the selected provider also disables inline suggestions.
A connection-test button is not yet exposed. The configuration file remains
available for every option.

For example, connect to a local
[llama.cpp](https://github.com/ggml-org/llama.cpp) server:

```yaml
completion:
  inline:
    enabled: true
    provider: local # key into providers
    statistics: true # local aggregate counts; disable to opt out
    debounce_ms: 300 # quiet time after the last keystroke
    max_line_suffix: 8 # auto-trigger only near the end of the line
  providers:
    local:
      transport: llama_cpp
      url: http://127.0.0.1:8012
      max_tokens: 128
      timeout_ms: 5000
```

Start the server with a fill-in-the-middle model, for example
`llama-server -m Qwen2.5-Coder-1.5B-Instruct-Q8_0.gguf --port 8012`. Tab
accepts the ghost text, Escape dismisses it, and Option+\\ asks for one
without waiting for the debounce. After three failed requests Token pauses
automatic suggestions until you trigger one manually.

Suggestions show all their lines as dimmed text in the focused pane, wrapping
with that pane's soft-wrap setting. Existing text after the cursor and following
lines move out of the way; the document is unchanged until you accept. Clicking
the speculative text places the cursor at its insertion point and dismisses it.
Navigation also dismisses it. Typing matching characters consumes the preview
without opening a competing automatic dropdown. Other panes keep their normal
document view. Explicit requests work mid-line; automatic requests still respect
`completion.inline.max_line_suffix`.

### Managed local llama-server

To have Token own the server, add `local_server` to a `llama_cpp` provider:

```yaml
completion:
  inline:
    enabled: true
    provider: local
  providers:
    local:
      transport: llama_cpp
      url: http://127.0.0.1:8012
      timeout_ms: 5000
      local_server:
        executable: /opt/homebrew/bin/llama-server
        model_path: /absolute/path/to/fim-model.gguf
        startup_timeout_ms: 120000
        context_size: 8192
        # gpu_layers: 0   # optional; omit to use the server's default
```

Both paths must be absolute; `~`, environment substitutions and shell commands
are not expanded. Install a FIM-capable GGUF model and llama-server yourself.
Token does not download either and starts the server with `--offline`. The
managed URL must use `http://127.0.0.1` with no proxy path, credentials, query or
fragment. Choose an unused port; an existing listener is not adopted or killed.
Each Token window owns its own child, so simultaneous managed windows need
different ports. Use an externally managed server to share one model process.

The first suggestion request starts the model. Token waits for the server's
[`/health` readiness response](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md#api-endpoints)
before sending source text. Startup has its own deadline (default 120 seconds,
maximum 600 seconds); `timeout_ms` still controls generation separately.
Cancellation interrupts generation but retains the model. Changing the selected
provider configuration, disabling completion/inline suggestions, or closing the
window stops its child. A failed startup or crashed child is not automatically
restarted on every keystroke: “Trigger Inline Suggestion” retries explicitly.

Inherited `LLAMA_*` settings are cleared so they cannot silently change the
managed model or server policy. The optional `api_key_env` credential reference
is forwarded to the child as `LLAMA_API_KEY`; no credential is placed in its
command line. Other OS/GPU environment remains available. Child output is
discarded, not saved alongside source code. Errors identify startup, exit,
timeout or port failure without including backend output; for detailed model
diagnostics or additional llama.cpp flags, run the server yourself and omit
`local_server`. Older server builds must support the documented `--offline` flag.

### Local completion statistics

`completion.inline.statistics` defaults to `true`; inline completion itself
remains disabled by default. Settings → Completion → Local completion statistics
controls the same value. No source, suggested text, file paths, endpoint settings,
credentials, or network telemetry are collected. The JSON contains only a format
version, the configured provider names, and these aggregate counters:

- `accepted`: the first successful explicit acceptance of any portion of a
  response, including word/line acceptance.
- `dismissed`: a displayed response ends without explicit acceptance, such as
  Escape, navigation, changing focus, or incompatible typing.
- `typed_through`: the entire suggestion was typed manually without explicit
  acceptance.

Each response contributes at most once. Cycling alternatives, accepting more
portions, and undoing an acceptance do not add or reverse outcomes. Pending,
empty, failed and stale responses do not count. Unresolved offers lost in a crash
do not count either, so the sum is completed observations, not all requests or
all generated alternatives. Counts alone are not a controlled quality comparison.

“Open Inline Completion Statistics” in the command palette opens
`inline-statistics.json` in the editor's configuration directory. This is a normal
file tab, not a live dashboard: close and reopen it to read newer counts. Reusing
an open tab preserves its buffer and unsaved edits. Do not save an old snapshot
over newer counts; to reset counts, close the tab and remove the JSON while the
editor is stopped. Leave the stable `inline-statistics.lock` sidecar alone while
any editor window is running.

Writes run on the ordered background file worker and merge across windows under
a bounded lock. Storage is capped at 256 KiB, 256 provider names and 256 UTF-8
bytes per name; counters saturate rather than wrap. Malformed, unsupported-version
and symlinked statistics files are left untouched. Failed writes show a non-modal
status message and are not replayed; editing and completion continue normally.
Turning collection off discards the current observation; turning it on starts
counting with newly requested responses. Existing stored counts are retained.

## Example Configuration

### Additional inline providers

Providers share request cancellation, deadlines and a 1 MiB response limit.
By default, the OpenAI-compatible transport uses native `prompt` + `suffix`
fields, not Chat Completions or Responses. Native mode needs a server/model that
supports suffix insertion. Raw FIM is an explicit alternative described below;
neither mode makes arbitrary chat models compatible. The [OpenAI Completions
reference](https://developers.openai.com/api/reference/resources/completions/methods/create)
documents the wire shape and model-specific suffix restriction.

```yaml
completion:
  inline:
    enabled: true
    provider: compatible
  providers:
    compatible:
      transport: open_ai_compat
      url: http://127.0.0.1:8080/v1
      model: your-suffix-capable-model
      # api_key_env: TOKEN_COMPLETION_KEY  # optional environment-variable name
      max_tokens: 128
      n: 1 # 1..8; higher values request more alternatives and may cost more
      timeout_ms: 5000
```

For [Mistral FIM](https://docs.mistral.ai/api/endpoint/fim), set the selected provider
to a separately configured entry such as:

```yaml
transport: mistral_fim
url: https://api.mistral.ai/v1
model: codestral-latest
api_key_env: TOKEN_MISTRAL_KEY
max_tokens: 128
timeout_ms: 5000
```

`url` is a base URL: the appropriate `/v1/completions` or `/v1/fim/completions`
route is appended, without duplicating a trailing `/v1`. Gateway path prefixes
are preserved. Do not include the complete endpoint, query parameters or
credentials in the URL.

Set the named credential variable in the environment inherited by the editor
process before starting it. A GUI launcher may not inherit your shell environment.
The config stores only its name; the worker resolves its value when starting a
request. Missing, empty or invalid credentials fail with a status message and no
request. No ambient API key is discovered automatically. Authenticated non-loopback
URLs must use HTTPS, and redirects are never followed. Selecting a hosted provider
sends the bounded source prefix/suffix to that provider and may incur API charges;
support does not enable it or change your selected provider automatically.

### TabbyML

For [TabbyML](https://tabby.tabbyml.com/api/completion/), select a provider such as:

```yaml
transport: tabby
url: http://127.0.0.1:8080
# api_key_env: TOKEN_TABBY_KEY  # when your server requires a bearer token
timeout_ms: 5000
```

The native `/v1/completions` request uses `segments.prefix` and `segments.suffix`.
Model selection and generation length are configured on the Tabby server;
the common `model`, `max_tokens` and `keep_alive` fields are not sent. Use
`prompt_format: native` and `n: 1`; raw prompts and requesting multiple results
are rejected. Returned alternatives still share filtering, cycling, partial
acceptance and Undo with other providers. The same base-path, credential,
deadline, cancellation and response-size rules above apply.
An empty `choices` list means no suggestion and does not trigger error backoff.

The adapter sends a language ID when known, but no absolute file paths, clipboard,
user identifier, Git URL or acceptance telemetry. Workspace-relative path and
declaration retrieval are not yet attached. Opt-in recent-buffer context uses
the existing commented-prefix fallback, not Tabby's declaration/search fields.
The adapter does not start a server, download a model or change your selected
provider. Automated local HTTP fixtures cover the contract; live Tabby model
quality has not been validated.

### Raw FIM prompts

`prompt_format: native` is the default and preserves the transport's existing
prefix/suffix handling. On `ollama` and `open_ai_compat`, an explicit format
instead serializes both parts into one raw prompt and omits the native `suffix`
field. Ollama receives `raw: true` to bypass its model template. Both transports
receive format-specific stop strings and retain existing cancellation, limits,
credentials and acceptance behavior. Raw formats are rejected for `llama_cpp`
and `mistral_fim` or `tabby`, which build FIM prompts server-side.

| `prompt_format` | Layout                                                                  |
| --------------- | ----------------------------------------------------------------------- |
| `qwen`          | Qwen FIM tokens, prefix then suffix                                     |
| `star_coder`    | StarCoder FIM tokens, prefix then suffix                                |
| `code_llama`    | CodeLlama prefix-first infilling, including marker word-boundary spaces |
| `deep_seek`     | DeepSeek's Unicode FIM tokens, prefix then suffix                       |
| `codestral`     | Suffix then prefix                                                      |
| `mellum`        | Suffix then prefix, with optional current-file basename                 |

For example, with a compatible raw-completion server already serving this model:

```yaml
completion:
  inline:
    enabled: true
    provider: raw
  providers:
    raw:
      transport: open_ai_compat
      url: http://127.0.0.1:8080/v1
      model: Qwen/Qwen2.5-Coder-7B
      prompt_format: qwen
```

`infer` recognizes bounded family names for Qwen2.5-Coder, StarCoder/StarCoder2,
CodeLlama 7B/13B (including their infilling-capable Instruct variants),
DeepSeek-Coder, Codestral and Mellum. Unknown aliases and unrecognized/ambiguous
variants fail with an explicit message; use an explicit format when you know
the served model's FIM vocabulary. Inference is a name mapping, not a capability
probe. It does not load a model or contact a hosted service to test compatibility.

The server must recognize the format's special tokens and provide its model's
normal beginning-of-sequence handling; Token does not add a second BOS tag or a
chat template. Prefix/suffix bytes are retained, with the format's own separators.
Raw source containing that format's control tokens is rejected before HTTP to
avoid ambiguous prompt boundaries. Mellum includes only a usable basename, not
an absolute path; names with control characters or angle brackets are omitted.
Changing `prompt_format` invalidates result-cache reuse. Live model/server quality
still needs validation for the selected deployment.

### Extra context from recently visited buffers

Extra context is off by default. To opt in for one provider, add:

```yaml
completion:
  providers:
    local: # your existing inline provider
      context:
        strategy: recency_ring # none (default) | recency_ring
        max_chunks: 8 # 1–32
        chunk_lines: 64 # 1–256
```

This permits sending snippets from other open text buffers, including **unsaved
text**, to that provider. It does not scan unopened files or persist a context
index. With a workspace open, named files must have a resolved identity inside
its root; symlinks resolving outside are excluded. Without a workspace, the
scope is the current window's open text buffers. Untitled buffers are included.
Labels use workspace-relative paths or buffer-qualified basenames, never absolute
directories; invalid/oversized labels are omitted.

Enabling the strategy, switching files, saving, or jumping at least `chunk_lines`
logical lines queues positions. After 750 ms without cursor/document changes,
the ring takes up to `chunk_lines` around each queued position, capped at 8 KiB
per snippet and 1 KiB per filename. Pending positions and retained chunks are
both bounded by `max_chunks`. The maximum text payload is 256 KiB, plus labels
and bounded metadata. A runtime-only index retains sorted token byte ranges so
pairwise deduplication does not re-tokenize existing chunks. On 64-bit builds its
retained range storage is bounded above by 2 MiB at the maximum configuration,
plus bounded capture scratch space, separate from the source payload; indexes
are not transmitted or copied into the result
cache. Empty snippets are skipped; token-set similarity strictly
above 90% removes the older snippet. Revisiting an overlapping region of the same
buffer replaces its old snapshot even after a substantial rewrite or deletion.
Ordering is by recency, not relevance ranking.

Committed text remains stable during ordinary typing. It represents an idle
snapshot, not a continuously updated copy; a subsequent save, switch or large
jump refreshes context after idle. Closing/renaming a source removes its old ring
entry. Disabling inline completion or changing provider settings/workspace clears
the ring. These actions cannot retract requests already sent to the provider.

llama.cpp receives `input_extra`. Other transports prepend line-commented snippets
to the active prefix, including in raw FIM mode. Common `//`, `#`, `--` and `;`
languages are supported. Languages without a supported comment fallback, such
as JSON, HTML and CSS, report an explicit configuration error when extra context
is present; use native llama.cpp or `strategy: none` for those languages.
The active prefix/suffix and cursor positions remain unchanged internally.
Extra context participates in exact and partial result-cache equality and the
existing 8 MiB cache payload limit. Changing context therefore prevents reuse of
an answer generated with different snippets; there is no new cache setting.

### Workspace retrieval context

For relevant declarations from unopened workspace files, use this **opt-in**
alternative to `recency_ring` on your existing provider:

```yaml
context:
  strategy: workspace_retrieval
  max_chunks: 8 # 1–32
  chunk_lines: 64 # 1–256
```

This permits sending workspace source, including unsaved text from eligible
open files, to the configured provider. It is not a secret detector: keep private
source excluded by ignore rules, or leave context disabled. No extra source is
collected without a workspace, or for a named active file outside that workspace.

The background worker respects `.gitignore` (also outside Git repositories),
`.ignore`, Git excludes and global Git ignores. It skips symlinks, hidden paths,
`target`, `node_modules`, `vendor`, `dist`, `build`, unknown-language files,
binary/invalid UTF-8 files and the active file. Reported ignore/traversal errors yield no
extra context. Open text snapshots replace disk content; oversized or special
open buffers are excluded, not silently replaced by their saved versions.

Existing Tree-sitter grammars and outline extractors identify declarations;
languages without an outline use line windows. BM25 ranks exact identifiers from
the last 1,024 prefix characters and first 256 suffix characters, excluding common
syntax words and taking at most 128 terms, closest prefix terms first.
Only positive-overlap snippets are sent; overlapping regions and
identical snippets are removed. This is lexical relevance, not type resolution
or a guarantee of useful model output. Selected chunks use stable path/text order,
but selection can change while typing and reduce the server's prompt-cache reuse.

Collection is bounded to 512 files, 256 KiB per file, 8 MiB of source, depth 20
and 20,000 traversal entries. It checks a 250 ms collection deadline between
entries; parsers have a 25 ms cancellation budget. These are cooperative limits,
not hard filesystem latency guarantees. Large workspaces may contribute only a
subset. Each file contributes at most 128 chunks; ranking examines at most 8 MiB
of chunk text. The usual 8 KiB/snippet and 1 KiB/filename limits still apply.

The window-local cache retains source and declaration ranges only in memory.
Each request rechecks ignore rules and source text, so changed, deleted and newly
ignored files cannot reuse old cached chunks. Cancellation supersedes background
work; document/cursor, provider and workspace identity are checked again before
HTTP submission. Switching away from retrieval or disabling inline completion
drops the cache. Requests already sent cannot be retracted. Transport formatting
and result-cache rules are the same as for recency context above.

### Cycling inline alternatives

For `open_ai_compat`, `n` selects 1–8 results per request and defaults to 1.
Other transports currently require `n: 1`; they do not fan out extra requests.
All returned alternatives pass through the same output filters. Empty and
duplicate suggestions are omitted, with at most eight retained in provider order.

While ghost text is visible, Alt+] selects the next compatible alternative and
Alt+[ the previous one (Option on macOS), wrapping at either end. The indicator
`[2/3]` means the second of three currently compatible choices. Cycling does not
edit, create an undo entry or ask the provider again. After typing through or
partially accepting a suggestion, only alternatives with the exact same inserted
prefix and a nonempty remainder remain eligible. Backspacing can make previously
incompatible choices eligible again. Tab accepts the selected remainder.

`NextInlineSuggestion` and `PrevInlineSuggestion` are named actions for automation
and user keybindings. Automation reports `inline_choice: [position, count]` only
while a suggestion is visible, alongside the existing remaining ghost text.

### General settings example

```yaml
# ~/.config/token-editor/config.yaml
theme: "fleet-dark"
cursor_blink_ms: 500
auto_surround: true
bracket_matching: true
```

## Per-file text settings

See [EditorConfig and text settings](editorconfig.md) for project rules, user
defaults, line-ending handling, and undoable save cleanup.
