# Offline preview assets

These are unmodified, self-contained browser distributions, embedded by
`src/markdown/renderer.rs`. Normal Cargo builds never download JavaScript or
require Node. The renderer includes each library only when its content is used.

| File | Source | SHA-256 |
| --- | --- | --- |
| `mermaid.min.js` | https://cdn.jsdelivr.net/npm/mermaid@11.17.2/dist/mermaid.min.js | `581ed7d74bd9048d0e3a91363927d72ef22942d7722546b27f7cc29e35390eb8` |
| `highlight.min.js` | https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/highlight.min.js | `837a6fa5b0c736b52bbde2b2b6190f305da3fc9ed41681db5321507057b5c846` |
| `github-dark.min.css` | https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/styles/github-dark.min.css | `9f208d022102b1d0c7aebfecd8e42ca7997d5de636649d2b31ea63093d809019` |

Mermaid is MIT-licensed (`Mermaid_LICENSE.txt`); Highlight.js and its stylesheet
use BSD-3-Clause (`Highlight_LICENSE.txt`). Upstream bundled dependency notices
are retained in the JavaScript. Ship both license files with application packages.

When updating, use the full Mermaid browser bundle, not the ESM entry point (it
loads additional chunks). Verify checksums, retain licenses and run the preview
tests plus a browser smoke check with networking blocked. Keep strict Mermaid
mode, escaped diagram source and invalid-syntax fallback. Document-authored
remote images/HTML are independent of these bundled renderer dependencies.
