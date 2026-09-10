# Language servers

Language servers provide completion, diagnostics, navigation, rename, formatting,
and documentation. They are separate programs installed on your computer; Token
starts a matching server when you open a supported file. No server or AI model is
downloaded automatically. Syntax highlighting works without a language server.

## Start in Settings

Open **Settings** with Cmd+, (Ctrl+, on Windows/Linux), then select **LSP**.

- **Language servers** switches all servers on or off.
- **Add language server…** registers another installed server; it is not limited
  to the built-in entries. Choose a unique ID, executable, arguments, languages,
  and optional project-root markers.
- Each server has its own enabled switch, executable row, and live status.
- **Configure…** opens a draft form inside Settings. Edit the executable (or
  Browse for it), arguments, initialization options, and server settings.
  **Apply & Restart** saves that server's configuration and rebinds matching open
  documents. If language assignments change, the affected previous servers are
  also restarted; unrelated servers are left alone. Cancel or Escape
  returns to the settings list without applying the draft. A failed save leaves
  the running configuration unchanged.
- The **Editor** category has Mouse hover, Hover delay, and Inlay hints controls.
  Inlay hints are off by default and currently affect Sema's line-end parameter
  annotations. Explicit Run output is independent of this switch.

Switches save immediately. Status is updated live: **Not started** means no
process has started, **Missing** means its executable could not be found, and
**Failed** means startup or repeated restart failed. **Ready** means the server
initialized; project indexing can still take time.

In the configuration form, Tab/Shift+Tab moves between controls. Text fields use
the code font and support selection, clipboard operations, undo, and multiline
editing. Arguments are a JSON/YAML list of strings, for example `["--stdio"]`;
`[]` means no arguments, and an empty field restores the server's default list.
Initialization options accept JSON-compatible JSON/YAML; server settings require
an object. Empty advanced fields use defaults. Syntax is checked before applying,
but only the server can validate the meaning of its own options.

The form shows the executable found using the application's PATH and the live
server state. **Open log** opens Token's log for startup details and retains the
draft; reopen Settings to return to it. Applying retries a previously missing
server for matching open files, and turning its form switch off stops it. No
installer is run automatically.

## Built-in servers

| Files                            | Configuration ID             | Default executable and arguments     | Project markers     |
| -------------------------------- | ---------------------------- | ------------------------------------ | ------------------- |
| Rust                             | `rust-analyzer`              | `rust-analyzer`                      | `Cargo.toml`        |
| Go                               | `gopls`                      | `gopls`                              | `go.work`, `go.mod` |
| JavaScript, TypeScript, JSX, TSX | `typescript-language-server` | `typescript-language-server --stdio` | `package.json`      |
| Python                           | `pyright`                    | `pyright-langserver --stdio`         | `pyproject.toml`    |
| PHP, Blade                       | `phpantom`                   | `phpantom_lsp`                       | `composer.json`     |
| Sema                             | `sema`                       | `sema lsp`                           | `sema.toml`         |

For files inside the open workspace, that workspace is the project root. For
other files, Token searches upward for the server's project markers, then falls
back to the file's directory.
Server IDs, not language names, are the keys under `lsp.servers`.

## Add a custom server

In **Settings → LSP → Add language server…**:

1. Choose an unused ID such as `clangd` or `lua-language-server`.
2. Choose the installed executable and its argument list. Token communicates over
   standard input/output; add a server's stdio argument when it requires one.
3. Enter comma-separated **Languages**, such as `C, C++` or `Lua`. The form accepts
   supported language names and code-fence aliases; unknown names are rejected.
4. Optionally supply **Root markers**, a JSON/YAML list of file or directory names,
   for example `[compile_commands.json, .git]`. An open workspace takes precedence;
   otherwise the nearest matching ancestor is used, falling back to the file's
   directory. `[]` means no markers; an empty field restores built-in defaults.
5. **Apply & Restart** validates and saves the entry. It then appears alongside the
   built-ins with its own Configure button, enabled switch, and live status.

Enabled explicit language assignments override the built-in mapping. Only one
server is chosen per language; Settings rejects overlapping enabled explicit
assignments. Disable the previous custom server before replacing it. Disabling a
custom server restores the built-in default, if one exists and is enabled.
For conflicting hand-written YAML, the alphabetically first enabled explicit
server ID wins. This does not add new syntax grammars or install server binaries.

Equivalent YAML for an installed C/C++ server:

```yaml
lsp:
  servers:
    clangd:
      command: clangd
      args: [--background-index]
      languages: [c, cpp]
      root_markers: [compile_commands.json, .git]
```

YAML language identifiers are lowercase registry names, such as `rust`, `cpp`,
`python`, `typescript`, `tsx`, `lua`, and `markdown`. The form writes these names
for you. Built-ins retain their default associations and root markers when these
fields are omitted.

## Configure paths and server options

Configuration lives in `~/.config/token-editor/config.yaml` on macOS/Linux, or
`%APPDATA%\token-editor\config.yaml` on Windows. On macOS/Linux,
`XDG_CONFIG_HOME` overrides the `.config` directory.

For example:

```yaml
lsp:
  enabled: true
  inlay_hints: false
  servers:
    gopls:
      command: /Users/you/go/bin/gopls
    sema:
      command: /Users/you/.cargo/bin/sema
      args: [lsp]
    pyright:
      enabled: false
```

`command` is an executable name or path, not a shell command. Put each argument
in the `args` list. Arguments replace the server's defaults; omit `args` to keep
defaults such as `--stdio`. Use an absolute path if an app launched from Finder
does not see a program that is available in your terminal.

After editing YAML, **save**, run **Reload Configuration**, then return to a file
of the affected language and run **Restart Language Server**. Reload reads the
preferences; it does not replace already-running server processes. After enabling
a server, opening or editing a matching file permits startup. Use the Settings
switches to stop running servers immediately.

Each server also accepts:

- `initialization_options`: JSON-compatible YAML passed as `initializationOptions`
  when its process initializes.
- `settings`: JSON-compatible YAML used to answer `workspace/configuration`.
  Requested section names are dotted paths within this object; missing sections
  return `null`. Include the section wrapper the server requests.

For example, a server asking for `rust-analyzer` reads from
`lsp.servers.rust-analyzer.settings.rust-analyzer`. These options are
server-specific, not universal editor preferences. Consult that server's own
configuration reference and restart it after changing them. Token's Settings
switches preserve unrelated YAML keys, but saving through Settings does not
preserve YAML comments or formatting.

## A quieter editor

These optional settings keep explicit documentation and completion available:

```yaml
hover_on_mouse: false # Show Hover still works on demand
hover_delay_ms: 600 # useful instead of disabling mouse hover
format_on_save: false
lsp:
  enabled: true
  inlay_hints: false
completion:
  enabled: true
  menu:
    enabled: false # Ctrl+Space still opens completion
```

This is an alternative profile, not the defaults. Mouse hover remains enabled
with a 300 ms delay, and automatic completion remains enabled. Auto-save,
format-on-save, and inline AI suggestions are opt-in. Sema semantic highlighting
is automatic, but Run actions live in Code Actions (Alt+Enter), not beside every
form, and only run when explicitly chosen. Inline AI providers are separate from
language servers; see [editor configuration](config-editor.md).

## Troubleshooting

1. Check **Settings → LSP** for the master switch, server switch, executable,
   and status. Installing a server alone does not override a disabled switch.
2. Open the project's directory, not just an unrelated file, and allow indexing
   to finish. Verify the executable runs from your terminal.
3. If it is **Missing**, use an absolute executable path, reload configuration,
   then reopen/edit a matching file. If it is **Failed**, use Restart Language
   Server after correcting its configuration.
4. Use **Open Log File** for startup and protocol errors. Paths and source-related
   diagnostics can appear in logs; review them before sharing.
5. Missing server features are not supplied by the local word fallback. In code,
   member completion is LSP-only; it does not guess methods from arbitrary words.

See also [completion and documentation controls](config-editor.md#hover-documentation).
