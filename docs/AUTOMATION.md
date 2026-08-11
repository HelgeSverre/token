# Token automation

Token exposes its real update and render loop through a local automation endpoint.
The interface is intended for deterministic tests, performance measurements, and
MCP clients; it does not move the system cursor.

Start the editor with a deterministic Rust document:

```bash
cargo run -- --demo
```

In another shell, inspect or drive the running process:

```bash
target/debug/token automate state
target/debug/token automate document
target/debug/token automate actions
target/debug/token automate text "hello"
target/debug/token automate cursor 4 8
target/debug/token automate selection 4 0 4 8
target/debug/token automate action DeleteBackward
target/debug/token automate scroll 10
target/debug/token automate profile 120
target/debug/token automate syntax-profile " "
```

`profile` forces full frames through the real `Renderer` and softbuffer surface,
then returns rolling frame and stage timings including buffer copy and present.
Frame counts are bounded to 1–10,000.
Stage timings are populated in debug builds, matching the existing performance
instrumentation. Release builds still execute the requested frames but report
an empty timing history.

`syntax-profile` inserts the supplied text and waits for the resulting syntax
revision to be presented. It reports Rope snapshot time, worker queue delay,
tree-sitter parsing, highlight-query traversal, outline extraction, main-thread
application, and total edit-to-present latency.

Cursor and selection coordinates are zero-based and are clamped to the active
document. `action` accepts the same PascalCase command names used by Token's
keymap, while `actions` reports the commands bound in the running editor. Both
semantic positioning and named actions flow through the normal update loop.

Run the stdio MCP bridge with:

```bash
target/debug/token mcp
```

It provides `get_state`, `get_document`, `list_actions`, `insert_text`,
`set_cursor`, `set_selection`, `execute_action`, `scroll`, `profile_frames`, and
`profile_syntax`. The bridge connects to an already-running Token window.
Document reads are bounded to 3 MiB; larger documents return a descriptive
error instead of producing an oversized IPC/MCP response.

On Unix, the endpoint defaults to
`$TMPDIR/token-<effective-user-id>/automation.sock`; its directory and socket
are created with owner-only permissions. Override it in both processes with
`TOKEN_AUTOMATION_SOCKET`. Windows uses loopback TCP and accepts the same
environment variable as an `IP:port` value.
