# Files

- [Editor Model, Documents, Cursors, and Undo](editor-state.md) - Explains the authoritative document and pane model, including shared buffers, pane-local selections, planned line and text edits, cursor mapping, and exact multi-pane undo/redo restoration. Defines the history and revision invariants that keep editing, saving, and asynchronous projections coherent.
- [Syntax, Parsing, Completion, and LSP State](syntax-and-language-services.md) - Explains how buffers acquire languages, how Tree-sitter-derived syntax state is produced and projected, and how language-server processes, document synchronization, and asynchronous editor features are coordinated safely.
