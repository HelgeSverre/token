# Editor Configuration Reference

General editor settings for Token.

---

## Configuration File

| Platform | Path |
|----------|------|
| macOS | `~/.config/token-editor/config.yaml` |
| Linux | `~/.config/token-editor/config.yaml` |
| Windows | `%APPDATA%\token-editor\config.yaml` |

---

## Settings

The page uses the standard themed scrollbar: drag its thumb or click the track
to jump. Scrolling leaves the selected setting unchanged. Wheel scrolling uses
the incoming scroll amount rather than a fixed palette-style three-row step.

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

### `cursor_blink_ms`

Cursor blink interval in milliseconds.

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

### `completion.inline` and `completion.providers`

Ghost-text suggestions from a local [llama.cpp](https://github.com/ggml-org/llama.cpp)
server. Off until you point Token at one:

```yaml
completion:
  inline:
    enabled: true
    provider: local        # key into providers
    debounce_ms: 300       # quiet time after the last keystroke
    max_line_suffix: 8     # auto-trigger only near the end of the line
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

## Example Configuration

```yaml
# ~/.config/token-editor/config.yaml
theme: "fleet-dark"
cursor_blink_ms: 500
auto_surround: true
bracket_matching: true
```
