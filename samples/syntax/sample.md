# Markdown Syntax Highlighting Test

This file demonstrates various Markdown syntax elements.

## Headers

### Level 3 Header
#### Level 4 Header
##### Level 5 Header

## Text Formatting

This is **bold text** and this is *italic text*.
You can also use __bold__ and _italic_ with underscores.
Here's some ***bold italic*** text.
And some ~~strikethrough~~ text.

## Links and Images

[Link to Rust](https://rust-lang.org)
[Link with title](https://example.com "Example Site")

![Alt text for image](https://example.com/image.png)

## Lists

### Unordered List
- Item one
- Item two
  - Nested item
  - Another nested item
- Item three

### Ordered List
1. First item
2. Second item
   1. Nested numbered
   2. Another nested
3. Third item

### Task List
- [x] Completed task
- [ ] Incomplete task
- [ ] Another task

## Code

Inline `code` looks like this.

```rust
fn main() {
    println!("Hello, World!");
}
```

```python
def greet(name):
    return f"Hello, {name}!"
```

```javascript
const greet = (name) => `Hello, ${name}!`;
```

## Blockquotes

> This is a blockquote.
> It can span multiple lines.
>
> > Nested blockquotes are possible too.

## Tables

| Column 1 | Column 2 | Column 3 |
|----------|:--------:|---------:|
| Left     | Center   | Right    |
| aligned  | aligned  | aligned  |
| data     | data     | data     |

## Horizontal Rules

---

***

___

## HTML in Markdown

<div style="color: red;">
  This is raw HTML content.
</div>

## Footnotes

Here's a sentence with a footnote.[^1]

[^1]: This is the footnote content.

## Definition Lists

Term 1
: Definition 1

Term 2
: Definition 2a
: Definition 2b

## Emoji (GitHub-flavored)

:smile: :rocket: :heart:

## Math (if supported)

Inline math: $E = mc^2$

Block math:
$$
\sum_{i=1}^{n} x_i = x_1 + x_2 + \cdots + x_n
$$
