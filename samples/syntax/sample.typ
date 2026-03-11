// Typst Syntax Highlighting Test
// A conference paper template with custom functions and styling.

#let version = "1.0.0"

// ============================================================
// Template configuration
// ============================================================

#let conf-paper(
  title: none,
  authors: (),
  abstract: none,
  keywords: (),
  date: datetime.today(),
  bibliography-file: none,
  body,
) = {
  // Document settings
  set document(title: title, author: authors.map(a => a.name))
  set page(
    paper: "us-letter",
    margin: (top: 2.5cm, bottom: 2.5cm, left: 2cm, right: 2cm),
    header: context {
      if counter(page).get().first() > 1 [
        #set text(size: 9pt, fill: gray)
        #title
        #h(1fr)
        #counter(page).display("1 / 1", both: true)
      ]
    },
    footer: context {
      if counter(page).get().first() == 1 [
        #set text(size: 8pt, fill: gray)
        Draft — #date.display("[month repr:long] [day], [year]")
      ]
    },
  )

  set text(font: "New Computer Modern", size: 10pt, lang: "en")
  set par(justify: true, leading: 0.65em, first-line-indent: 1.2em)
  set heading(numbering: "1.1")

  // Custom heading styles
  show heading.where(level: 1): it => {
    set text(size: 14pt, weight: "bold")
    v(1.2em)
    block(it)
    v(0.6em)
  }

  show heading.where(level: 2): it => {
    set text(size: 12pt, weight: "bold")
    v(0.8em)
    block(it)
    v(0.4em)
  }

  // Code block styling
  show raw.where(block: true): it => {
    set text(font: "JetBrains Mono", size: 8.5pt)
    block(
      fill: luma(245),
      stroke: 0.5pt + luma(200),
      radius: 3pt,
      inset: 10pt,
      width: 100%,
      it,
    )
  }

  show raw.where(block: false): box.with(
    fill: luma(240),
    inset: (x: 3pt, y: 0pt),
    outset: (y: 3pt),
    radius: 2pt,
  )

  // Link styling
  show link: set text(fill: rgb("#2563eb"))

  // Figure styling
  show figure: it => {
    set align(center)
    v(0.5em)
    it
    v(0.5em)
  }

  show figure.caption: it => {
    set text(size: 9pt)
    it
  }

  // ---- Title block ----
  align(center)[
    #text(size: 18pt, weight: "bold")[#title]
    #v(1em)
    #for (i, author) in authors.enumerate() {
      let comma = if i < authors.len() - 1 [, ] else []
      [#text(weight: "bold")[#author.name]#super[#str(i + 1)]#comma]
    }
    #v(0.5em)
    #for (i, author) in authors.enumerate() [
      #text(size: 9pt, fill: gray)[#super[#str(i + 1)] #author.affiliation] #linebreak()
    ]
  ]

  // ---- Abstract ----
  if abstract != none {
    v(1em)
    block(inset: (left: 2em, right: 2em))[
      #text(weight: "bold")[Abstract. ]
      #text(size: 9.5pt)[#abstract]
    ]
  }

  // ---- Keywords ----
  if keywords.len() > 0 {
    block(inset: (left: 2em, right: 2em))[
      #text(weight: "bold", size: 9pt)[Keywords: ]
      #text(size: 9pt)[#keywords.join(", ")]
    ]
  }

  v(1.5em)

  // ---- Body ----
  columns(2, gutter: 1.2em, body)

  // ---- Bibliography ----
  if bibliography-file != none {
    bibliography(bibliography-file, style: "ieee")
  }
}

// ============================================================
// Custom components
// ============================================================

#let definition(term, body) = {
  block(
    stroke: (left: 2pt + rgb("#3b82f6")),
    inset: (left: 10pt, rest: 8pt),
    [*Definition (#term).* #body],
  )
}

#let theorem(name: none, body) = {
  let title = if name != none [Theorem (#name)] else [Theorem]
  block(
    fill: rgb("#f0f9ff"),
    stroke: 0.5pt + rgb("#93c5fd"),
    radius: 3pt,
    inset: 10pt,
    width: 100%,
    [*#title.* #emph(body)],
  )
}

#let proof(body) = {
  [_Proof._ #body #h(1fr) $square.stroked$]
}

#let note(body) = {
  block(
    fill: rgb("#fefce8"),
    stroke: 0.5pt + rgb("#fbbf24"),
    radius: 3pt,
    inset: 10pt,
    width: 100%,
    [#emoji.lightbulb *Note.* #body],
  )
}

#let algorithm(caption: none, body) = figure(
  kind: "algorithm",
  supplement: [Algorithm],
  caption: caption,
  block(
    stroke: 0.5pt + luma(150),
    radius: 3pt,
    inset: 10pt,
    width: 100%,
    align(left, body),
  ),
)

// Helper for algorithm steps
#let step(n, body) = [#text(weight: "bold")[#n.] #body \ ]

// ============================================================
// Document content
// ============================================================

#show: conf-paper.with(
  title: "Efficient Text Buffer Architectures for Modern Code Editors",
  authors: (
    (name: "Alice Chen", affiliation: "Dept. of CS, University of Examples"),
    (name: "Bob Smith", affiliation: "Research Labs, EditorCorp"),
  ),
  abstract: [
    We present a comparative analysis of text buffer data structures
    for code editors, focusing on rope-based and piece-table approaches.
    Our benchmarks demonstrate that rope structures with B-tree backing
    achieve $O(log n)$ edit operations while maintaining cache-friendly
    memory access patterns. We introduce _adaptive chunking_, a technique
    that dynamically adjusts node sizes based on edit locality.
  ],
  keywords: ("text editor", "rope", "data structures", "performance"),
)

= Introduction

Modern code editors must handle documents ranging from single-line
configuration files to multi-million-line codebases. The choice of
text buffer data structure fundamentally impacts editor performance
across three dimensions:

+ *Edit latency* — time to insert or delete text
+ *Memory efficiency* — overhead per character stored
+ *Rendering speed* — time to extract visible lines

#note[
  This paper focuses on _single-document_ performance. Multi-file
  indexing and project-wide search are orthogonal concerns.
]

= Background

#definition("Rope")[
  A rope is a binary tree where leaf nodes contain short strings
  and internal nodes store the combined weight (character count)
  of their left subtree.
]

The key insight is that string concatenation becomes $O(log n)$
by creating a new parent node, avoiding the $O(n)$ copy required
by contiguous arrays.

== Complexity Comparison

#figure(
  caption: [Time complexity of buffer operations],
  table(
    columns: 4,
    align: (left, center, center, center),
    stroke: 0.5pt + luma(200),
    inset: 6pt,
    table.header(
      [*Operation*], [*Array*], [*Rope*], [*Piece Table*],
    ),
    [Insert at cursor], [$O(n)$], [$O(log n)$], [$O(1)$#super[\*]],
    [Delete range], [$O(n)$], [$O(log n)$], [$O(1)$#super[\*]],
    [Index by line], [$O(n)$], [$O(log n)$], [$O(m)$],
    [Memory overhead], [1x], [~1.5x], [~1.2x],
  ),
) <tab:complexity>

#super[\*] Amortized. Piece table appends to a buffer and adjusts a descriptor table.

= Adaptive Chunking

#theorem(name: "Chunk Balance")[
  For a rope with adaptive chunk sizes $c_i in [c_min, c_max]$
  where $c_max = 2 c_min$, the tree height is bounded by
  $ h <= ceil(log_2 (n / c_min)) + 1 $
]

#proof[
  By induction on the number of nodes. The base case is trivial
  for $n <= c_max$. For the inductive step, splitting a full chunk
  produces two chunks of size $>= c_min$, maintaining the invariant.
]

#algorithm(caption: [Adaptive chunk splitting])[
  #step(1)[*Input:* chunk $C$ of size $|C| > c_max$]
  #step(2)[Find split point $s$ near $|C| / 2$ at a line boundary]
  #step(3)[*if* no line boundary in range $[|C|/3, 2|C|/3]$ *then*]
  #step(4)[#h(1em) Split at $|C| / 2$ (hard split)]
  #step(5)[*else* Split at $s$ (soft split)]
  #step(6)[Rebalance parent if height difference $> 2$]
]

= Results

Our benchmarks on a 1M-line file show:

```rust
// Rope insert benchmark (simplified)
fn bench_insert(rope: &mut Rope, rng: &mut ThreadRng) {
    let pos = rng.gen_range(0..rope.len_chars());
    rope.insert(pos, "benchmark text\n");
}
```

The adaptive approach reduces worst-case insert latency by
*47%* compared to fixed-size chunks, as shown in @tab:complexity.

= Conclusion

Rope structures with adaptive chunking provide the best trade-off
between edit performance and memory efficiency for general-purpose
code editors. Future work includes exploring CRDT-compatible rope
variants for real-time collaboration.
