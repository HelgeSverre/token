# Mermaid preview

Open the Markdown preview to render these diagrams. The renderer loads on demand
from a pinned CDN version; without a connection the source remains readable.

## Flowchart

```mermaid
flowchart LR
    A[Markdown source] --> B[Preview]
    B --> C{Mermaid fence?}
    C -->|Yes| D[Render diagram]
    C -->|No| E[Render text or code]
```

## Sequence

```mermaid
sequenceDiagram
    participant User
    participant Editor
    participant Preview
    User->>Editor: Edit Markdown
    Editor->>Preview: Update content
    Preview-->>User: Show diagram
```

## State transitions

This self-loop needs extra rank spacing to keep its transition label clear of the
outgoing edge label. Diagram-local configuration leaves other diagrams unchanged.

```mermaid
---
config:
  state:
    nodeSpacing: 80
    rankSpacing: 120
---
stateDiagram-v2
    [*] --> Source
    Source --> Diagram: Renderer ready
    Source --> Source: Offline or invalid syntax
    Diagram --> [*]
```
