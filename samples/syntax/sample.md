# Markdown and GFM Syntax Highlighting Test

This fixture is a readable stress test for Markdown highlighting, syntax-aware
selection, and fenced-language injection. It mixes ordinary prose with
**strong emphasis**, _quiet emphasis_, ~~obsolete advice~~, and `inline code`
so that neighboring syntax nodes can be inspected without opening a synthetic
micro-example.

Markdown paragraphs can contain [named links](https://www.rust-lang.org/),
autolinks such as <https://github.com/commonmark/commonmark-spec>, and bare GFM
URLs such as https://example.com/docs/getting-started. Escaped punctuation like
\*this is literal\* should remain text, while a hard break follows this line.\
The next sentence should therefore begin on a new rendered line.

An image has useful boundaries too:
![A tiny placeholder landscape](https://example.com/landscape.png "Example image")

## Heading Shapes

### ATX level three

#### ATX level four with `code`

Setext level two
----------------

The prose below is deliberately longer than one line. Expanding a selection
from a word should move through emphasis, links, sentences, and paragraphs
before reaching a surrounding section. This makes the fixture useful for both
manual testing and automated selection snapshots.

## GitHub-Flavored Markdown

GFM adds several constructs commonly found in issue descriptions and project
documentation. The crossed-out phrase in ~~ship on Friday afternoon~~ is one
example; task lists, tables, and autolinks provide larger structural cases.

### Task lists

- [x] Parse the outer Markdown document
- [x] Detect an injected language from fence info
- [ ] Preserve useful selection boundaries
  - [x] Strings remain smaller than expressions
  - [ ] Attributes can join the Rust item they decorate
- [ ] Test mixed content
  - [ ] Quoted lists
  - [ ] Tables containing `inline_code()`

### Tables

| Runtime | Fence name | Example value | Ready? |
| :------ | :--------: | ------------: | :----: |
| Rust    |   `rust`   |      `42_u64` |   ✅   |
| Python  |  `python`  |        `3.14` |   ✅   |
| Sema    |   `sema`   |      `'(a b)` |   ✅   |
| Scheme  |  `scheme`  |          `#t` |   ✅   |

| Feature      | Notes                                                                  |
| ------------ | ---------------------------------------------------------------------- |
| Escaped pipe | A table cell may contain `left \| right`                               |
| Formatting   | Cells can contain **bold**, _italic_, and [links](https://example.com) |

### Alerts and blockquotes

> [!NOTE]
> GitHub-style alerts are blockquotes with a marker on their first line.
> Their bodies may contain **formatting**, `code`, and multiple sentences.

> A regular quotation can span several lines and more than one paragraph. It
> is useful for checking whether selection expands to the quoted paragraph
> before consuming the complete blockquote.
>
> - Quoted lists are still lists.
> - Their items can contain [links](https://commonmark.org/).
>   1. They may also nest ordered items.
>   2. Each prefix contributes another syntax boundary.
>
> > A nested quotation introduces another scope.
> >
> >     Indented code can live inside it as well.
>
> The outer quotation resumes after the nested block.

> [!WARNING]
> Editing a delimiter at the edge of a selection is an excellent way to expose
> stale parse trees, so this fixture intentionally uses many delimiter styles.

## Lists with Rich Content

1. The first item is a short paragraph.
2. The second item has more structure.

   Its continuation is a separate paragraph, but remains part of the same list
   item. This distinction is useful when walking parents in the syntax tree.

   - A nested bullet contains **formatted prose**.
   - Another contains a small code span: `items.iter().map(transform)`.

3. The final item contains a thematic transition in its prose—not a rule.

## Fenced Code Injections

The examples are intentionally substantial enough to exercise strings,
comments, attributes, nested scopes, and language-specific punctuation.

### Rust

```rust
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
struct TokenCount {
    token: String,
    count: usize,
}

fn count_tokens(input: &str) -> Vec<TokenCount> {
    let mut counts = BTreeMap::<String, usize>::new();

    for token in input
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_lowercase)
    {
        *counts.entry(token).or_default() += 1;
    }

    counts
        .into_iter()
        .map(|(token, count)| TokenCount { token, count })
        .collect()
}

#[test]
fn counts_repeated_words_without_losing_attributes() {
    let actual = count_tokens("Trees, syntax trees, and more trees!");
    assert!(actual.contains(&TokenCount {
        token: "trees".into(),
        count: 3,
    }));
}
```

### Python

```python
from collections import defaultdict
from dataclasses import dataclass
from typing import Iterable, Iterator


@dataclass(frozen=True, slots=True)
class Reading:
    sensor: str
    value: float


def moving_average(values: Iterable[float], window: int) -> Iterator[float]:
    if window < 1:
        raise ValueError("window must be positive")

    pending: list[float] = []
    for value in values:
        pending.append(value)
        if len(pending) > window:
            pending.pop(0)
        if len(pending) == window:
            yield sum(pending) / window


def summarize(readings: Iterable[Reading]) -> dict[str, float]:
    grouped: dict[str, list[float]] = defaultdict(list)
    for reading in readings:
        grouped[reading.sensor].append(reading.value)
    return {name: sum(values) / len(values) for name, values in grouped.items()}
```

### Sema

```sema
(define readings
  '({:sensor "north" :value 18}
    {:sensor "south" :value 21}
    {:sensor "north" :value 24}))

(define (reading-value reading)
  (get reading :value))

(define (average values)
  (if (= (length values) 0)
      0
      (/ (reduce + values) (length values))))

(define summarize
  (pipe
    (fn (items) (map reading-value items))
    (fn (values) (filter (fn (value) (>= value 0)) values))
    average))

(define result (summarize readings))
(println (format "average sensor reading: ~a" result))
```

### Scheme

```scheme
(define (partition predicate items)
  (let loop ((rest items) (yes '()) (no '()))
    (cond
      ((null? rest) (list (reverse yes) (reverse no)))
      ((predicate (car rest))
       (loop (cdr rest) (cons (car rest) yes) no))
      (else
       (loop (cdr rest) yes (cons (car rest) no))))))

(define (quicksort items)
  (if (or (null? items) (null? (cdr items)))
      items
      (let* ((pivot (car items))
             (parts (partition (lambda (value) (< value pivot))
                               (cdr items)))
             (smaller (car parts))
             (larger (cadr parts)))
        (append (quicksort smaller)
                (list pivot)
                (quicksort larger)))))

(display (quicksort '(8 3 5 1 13 2 1)))
(newline)
```

## HTML Embedded in Markdown

<details>
<summary>Open the raw HTML example</summary>

This Markdown paragraph sits inside a collapsible HTML element.

<div class="syntax-sample" data-language="markdown">
  <strong>Raw HTML</strong> may contain attributes and nested elements.
</div>

</details>

## References, Footnotes, and Less Common Extensions

Reference-style links keep their destination away from the prose. Read the
[CommonMark specification][commonmark] or return to the [project page][project].
The same paragraph includes a footnote about portability.[^portability]

[commonmark]: https://spec.commonmark.org/ "CommonMark specification"
[project]: https://github.com/

[^portability]:
    Footnotes are a useful extension, though they are not part of
    core GFM. This continuation line checks indentation within a footnote.

Term with inline code
: A definition-list entry for `selection_range`; definition lists are another
extension and may be treated as plain paragraphs by strict GFM parsers.

Emoji shortcodes may be interpreted by a host renderer: :sparkles: :crab:
:snake: :lisp:

## Math-Like Text

Inline math extensions may recognize $E = mc^2$, while a plain Markdown parser
should still preserve the punctuation as text.

$$
\operatorname{mean}(x) = \frac{1}{n}\sum_{i=1}^{n} x_i
$$

---

End of fixture. The thematic break above should be selectable independently
before expansion reaches this final section or the full document.
