# Settings prototype

Open [settings.html](settings.html) directly in a browser. No server, build, or
network connection is needed. The fonts use the repository's existing assets.

Following Zest's `prototypes/` approach, this is a clickable design reference,
not an application implementation. All state is held in the browser tab.

- One list and editor for all language servers. No privileged built-in entries.
- Presets prefill an ordinary draft; every resulting field remains editable.
- Add, edit, disable, remove, validate, save, and discard drafts.
- Persistent Save/Cancel/Remove actions, with advanced fields collapsed initially.
- AI providers use the same list/detail pattern and remain opt-in.
- Preview controls switch between populated, empty, add, and missing-executable
  states. Dark/light themes and narrower browser widths can be compared.

Executable browsing and setup/connection checks are explicitly simulated. The
configuration preview is illustrative, not an export. This does not install
servers, read credentials, launch processes, or write Token's configuration.

Deep links: `?view=lsp`, `?view=add`, `?view=empty`, `?view=missing`, `?view=ai`,
`?view=editor`, optionally with `&theme=light`.
