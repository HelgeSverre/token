# EditorConfig and text settings

Token reads `.editorconfig` files for text documents by default. Rules in a nearer
file override matching rules in its ancestors; later sections override earlier
sections. Discovery stops at `root = true` or the filesystem root. A workspace
boundary does not stop discovery. Existing symlink aliases use the physical
file's rules, so all views of one document share the same settings.

```ini
root = true

[*]
indent_style = space
indent_size = 2
end_of_line = lf
trim_trailing_whitespace = true
insert_final_newline = true

[*.go]
indent_style = tab
indent_size = tab
tab_width = 4
```

`indent_size` controls each indentation step. `tab_width` controls the display
width of a hard tab. They can differ: tabs with an indentation size of 4 and a
tab width of 8 use four spaces for the first level and a tab for the second.
Typing, block indentation, wrapping, cursor placement, and formatting requests
use the document's effective settings. Model character offsets and LSP UTF-16
positions remain independent of display columns.

Token supports `indent_style`, `indent_size`, `tab_width`, `end_of_line`,
`trim_trailing_whitespace`, and `insert_final_newline`. Standard property names
and values are case-insensitive. `unset` removes a project preference and falls
back to your user preference. Unknown properties are ignored. Widths must be
between 1 and 256; invalid values produce a diagnostic and use the fallback.

Changes to a consulted `.editorconfig`, including creation or deletion, update
open documents. A failed reload retains the last valid rules and reports the
problem. Individual config reads are limited to 1 MiB; one ancestor chain is
limited to 4 MiB. Changing a rule does not mark a document modified or rewrite a
clean file. An explicit Save applies the current rules even to a clean document.

Save cleanup runs after optional formatting as an undoable edit. It removes
trailing spaces and tabs when requested, converts LF/CRLF/CR endings when a rule
specifies one, and then captures the saved text. `insert_final_newline = true`
preserves existing extra blank lines and leaves an empty file empty. `false`
removes terminal line endings. An absent property preserves the existing text.
Failed writes retain the cleanup in the buffer and undo history.

Enter uses the configured line ending. Without a rule, it uses the most common
ending loaded from the file; the first occurrence breaks ties. Files without
line endings default to LF. Clipboard text keeps its original endings until a
configured save conversion. CSV table serialization does not receive text
whitespace cleanup.

Save As resolves the destination's rules before formatting and cleanup. Those
rules become the document's settings only after a successful write. A failed
write retains the source settings and leaves cleanup undoable. A source-language
formatter is skipped when saving under a different language's extension.

The text path remains UTF-8. Existing UTF-8 BOM bytes are preserved. `charset`
requests that would add/remove a BOM or convert encodings are shown in policy
diagnostics but do not convert the file.

The status bar shows the effective indentation step and line ending. Use
**Show File Text Settings** in the command palette for a copyable report of the
resolved values, source files and line numbers, diagnostics, and consulted
config locations. The report is a snapshot in an untitled tab.

Settings → Editor includes the EditorConfig switch and default indentation, tab
width, and line-ending controls. YAML can specify custom values:

```yaml
editorconfig: true
text:
  indent_style: space
  indent_size: 3
  tab_width: 8
  end_of_line: crlf
  trim_trailing_whitespace: false
```

Without explicit preferences, Token uses hard tabs with an indentation step and
display width of four. Indentation guides still infer a step from the document
when no explicit indentation rule applies. LSP formatting now receives these
same indentation preferences; it previously always requested four spaces.

The parser targets EditorConfig 0.17.2. See the
[fixture notes](../../tests/fixtures/editorconfig/README.md) for the pinned core
compatibility checks and their scope.
