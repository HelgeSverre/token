# Line Operations Follow-ups

Status: partially implemented; remaining commands are not implemented.

Line/selection duplication is implemented. The
[archived proposal](../archived/line-operations.md) retains the original design,
implementation checklist and test cases; this note keeps its unfinished scope
in the active index, without duplicating that design.

- Join lines: support cursor and selection ranges, normalize whitespace at joins,
  and handle multiple cursors without joining the same boundary twice.
- Trim trailing whitespace: support the current line, selected lines and the
  whole document. Coordinate with [Whitespace Management](whitespace-management.md)
  so trimming has one command implementation, not separate feature-local paths.
- Route both through shared editing and position mapping, with one Undo step per
  action and no Undo entry for a no-op. Add palette actions and documented,
  configurable keybindings; historical proposed shortcuts are not shipped bindings.
- Retain the original edge-case and manual verification checklist, including
  empty/last lines, overlapping cursors and selections, Unicode and Undo/Redo.
  Check CRLF and split-pane position mapping against the current editing contracts.

Archival of the original proposal is not completion of these requirements.
