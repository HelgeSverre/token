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
target/debug/token automate open src/main.rs:42:7 README.md
```

`open` is the request the `token` command itself uses: paths are made
absolute in the client, `file:line[:column]` suffixes are 1-indexed, an
already-open file is focused instead of duplicated, and a directory starts a
separate editor process. The same request carries `"wait": true` for
`token --wait`: the editor holds the response (with no timeout) until every
document it opened has been closed in every group, or until it exits, and
the client treats a closed connection during a wait as success.

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

It provides `list_instances`, `get_state`, `get_document`, `list_actions`, `open_paths`,
`insert_text`, `set_cursor`, `set_selection`, `execute_action`, `scroll`,
`profile_frames`, and `profile_syntax`. The bridge connects to an already-running Token window.
Document reads are bounded to 3 MiB; larger documents return a descriptive
error instead of producing an oversized IPC/MCP response.

## Instances

Every editor window is its own process, and every process listens on its own
endpoint, so any number of windows can be automated from one client. List them
with:

```bash
target/debug/token automate instances
```

Each entry carries `instance_id` (the process id, also reported as
`instance_id` in every `state` response), `workspace_root`, `document_name`,
and `focused_at_ms`. Commands go to the most recently focused editor unless you
name one:

```bash
target/debug/token automate --instance 12345 state
```

`open` without `--instance` picks the editor whose workspace contains the first
path, and a directory argument focuses the editor already showing that
workspace instead of starting another. The MCP bridge exposes the same thing as
`list_instances`, and every other tool accepts an optional `instance` argument.

On Unix each instance advertises itself as
`$TMPDIR/token-<effective-user-id>/instances/<pid>.sock`; the directories and
sockets are created with owner-only permissions, an editor removes its own
socket on exit, and clients delete advertisements whose process is gone.
Windows binds a loopback port per instance and records it in
`%TEMP%\token\instances\<pid>.port`. Setting `TOKEN_AUTOMATION_SOCKET` (a
socket path on Unix, `IP:port` on Windows) pins one editor and its clients to
exactly that endpoint and turns discovery off, which is how tests keep an
editor under test apart from the one you are working in.
