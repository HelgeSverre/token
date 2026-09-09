# Code folding

Use the chevron beside a foldable line to collapse or expand its body. A
right-facing chevron means collapsed; a downward-facing chevron means expanded.
With soft wrap enabled, the chevron appears on the header's first row. A collapsed
header also shows an ellipsis and hidden-line count; click it to expand the fold.

The command palette provides **Toggle Fold**, **Collapse Fold**, **Expand Fold**,
**Collapse All Folds**, and **Expand All Folds**. A command inside a block acts on
the innermost containing block. These actions have no default shortcuts; bind
`ToggleFold`, `CollapseFold`, `ExpandFold`, `CollapseAllFolds`, or `ExpandAllFolds`
in Keybindings.

Folding works in ordinary text tabs. It does not modify text, mark a file dirty,
or add an undo step. Each split pane keeps independent collapse choices; a new
split starts with the source pane's choices. Expanding a parent retains its nested
collapse choices.

Arrow and page movement skip hidden rows. Find, go-to-line and other explicit
navigation reveal their destination. Collapsing a block moves an empty caret
inside it to the header. A block overlapping a nonempty selection stays expanded.
Copying or editing a logical selection includes its hidden text. Editing a fold
expands it; unaffected folds follow edits above them, including undo and redo.
Diagnostics in a hidden body are represented on its header, and Find and diagnostic
overview marks include hidden text.

## Detection

| Files                            | Folding regions                                                                                                     |
| -------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| Rust                             | Functions, impls, traits, structs, enums, modules, blocks, collections, block comments and raw strings              |
| JavaScript, TypeScript, JSX, TSX | Functions, methods, classes, interfaces, blocks, collections, multiline comments, template strings and JSX elements |
| Python                           | Functions, classes, control-flow statements, collections and multiline strings                                      |
| JSON and YAML                    | Collections and multiline values                                                                                    |
| HTML and CSS                     | Elements, rules, blocks and multiline comments                                                                      |
| Markdown                         | Heading sections, fenced/indented code, lists, block quotes and HTML blocks                                         |
| Other text                       | Indentation, using the file's effective tab width                                                                   |

Supported embedded languages in Markdown fences and HTML are included. Malformed
syntax falls back to indentation. Blank lines do not create or close indentation
blocks, and trailing blank lines remain visible. Candidate detection is asynchronous;
a freshly opened or changed file may briefly wait for updated chevrons. Files above
32 MiB currently have no fold candidates.

## Reopening and restarting

Saved-file sessions retain each pane's folds. Closing a pane also remembers that
file's most recent choices for ordinary reopening. The recent list is bounded to
128 files and 1 MiB of metadata, with at most 4,096 saved regions per pane.

Persistence records fingerprints and region descriptions, not source text. An
unchanged file restores matching regions; a changed file restores only unique
fingerprint/context matches. Deleted or ambiguous regions stay expanded. Dirty
files retain the last fold record captured against saved content. Navigation
before detection finishes cancels late restoration, and restored carets always
remain visible.

This uses the existing session store. Disabling session saving prevents new fold
state from being written on exit; disabling session restoration prevents loading
it at startup. Close/reopen memory within the running window continues to work.
