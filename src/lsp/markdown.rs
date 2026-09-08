//! Markdown -> [`StyledText`] for completion, hover and signature cards.
//! Uses the preview's CommonMark parser, but emits native text/spans only:
//! HTML is literal text, links keep their labels and images keep their alt
//! text. No resource loading or executable markup enters an overlay.

use crate::model::{SpanStyle, StyledText};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// Reduces Markdown to the overlay's shared text and non-overlapping spans.
/// Lists, quotes and tables retain readable text structure; code remains
/// literal. Unsupported visual effects degrade to text (e.g. dim strikeout).
pub fn markdown_to_styled(markdown: &str) -> StyledText {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH;
    let mut text = CardText::default();
    for (event, range) in Parser::new_ext(markdown, options).into_offset_iter() {
        // Keep paragraph separation without inventing extra rows between
        // adjacent blocks. Indentation/list markers are owned by the parser.
        if matches!(event, Event::Start(_)) {
            let preceding = markdown[..range.start]
                .chars()
                .rev()
                .take_while(|c| c.is_whitespace())
                .filter(|&c| c == '\n')
                .take(2)
                .count();
            if preceding == 2 {
                text.boundary(2);
            }
        }
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { .. } | Tag::TableHead => {
                    text.boundary(1);
                    text.strong += 1;
                    text.column = 0;
                }
                Tag::Strong | Tag::Emphasis => text.strong += 1,
                Tag::Strikethrough => text.dim += 1,
                Tag::CodeBlock(_) => {
                    text.boundary(1);
                    text.code = true;
                }
                Tag::BlockQuote(_) => {
                    text.boundary(1);
                    text.quotes += 1;
                }
                Tag::List(start) => {
                    text.boundary(1);
                    text.lists.push(start);
                }
                Tag::Item => {
                    text.boundary(1);
                    let indent = "  ".repeat(text.lists.len().saturating_sub(1));
                    text.push(&indent, None);
                    let marker = match text.lists.last_mut() {
                        Some(Some(number)) => {
                            let marker = format!("{number}. ");
                            *number = number.saturating_add(1);
                            marker
                        }
                        _ => "• ".to_owned(),
                    };
                    text.push(&marker, None);
                }
                Tag::Table(_) | Tag::TableRow => {
                    text.boundary(1);
                    text.column = 0;
                }
                Tag::TableCell => {
                    if text.column > 0 {
                        text.push(" | ", None);
                    }
                    text.column += 1;
                }
                Tag::FootnoteDefinition(label) => {
                    text.boundary(1);
                    text.push(&format!("[{label}] "), None);
                }
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Heading(_) | TagEnd::TableHead => {
                    text.strong -= 1;
                    text.boundary(1);
                }
                TagEnd::Strong | TagEnd::Emphasis => text.strong -= 1,
                TagEnd::Strikethrough => text.dim -= 1,
                TagEnd::CodeBlock => {
                    text.code = false;
                    text.boundary(1);
                }
                TagEnd::BlockQuote(_) => {
                    text.quotes -= 1;
                    text.boundary(1);
                }
                TagEnd::List(_) => {
                    text.lists.pop();
                    text.boundary(1);
                }
                TagEnd::Paragraph
                | TagEnd::Item
                | TagEnd::TableRow
                | TagEnd::Table
                | TagEnd::FootnoteDefinition
                | TagEnd::HtmlBlock => text.boundary(1),
                _ => {}
            },
            Event::Text(value) | Event::Html(value) | Event::InlineHtml(value) => {
                text.push(&value, text.style());
            }
            Event::Code(value) | Event::InlineMath(value) | Event::DisplayMath(value) => {
                text.push(&value, Some(SpanStyle::Code));
            }
            // Source wrapping is not a visual line break in a paragraph.
            // Explicit hard breaks and literal fenced-code newlines remain.
            Event::SoftBreak => text.push(" ", text.style()),
            Event::HardBreak => text.boundary(1),
            Event::Rule => text.boundary(2),
            Event::TaskListMarker(checked) => {
                text.push(if checked { "[x] " } else { "[ ] " }, None)
            }
            Event::FootnoteReference(label) => text.push(&format!("[{label}]"), None),
        }
    }
    // A parser code/HTML event can include a final newline. Keep internal
    // blank lines and indentation, but don't leave an empty trailing card row.
    let end = text.out.text.trim_end_matches('\n').len();
    text.out.text.truncate(end);
    for span in &mut text.out.spans {
        span.range.end = span.range.end.min(end);
    }
    text.out.spans.retain(|span| !span.range.is_empty());
    text.out
}

#[derive(Default)]
struct CardText {
    out: StyledText,
    breaks: usize,
    strong: usize,
    dim: usize,
    code: bool,
    quotes: usize,
    lists: Vec<Option<u64>>,
    column: usize,
}

impl CardText {
    fn style(&self) -> Option<SpanStyle> {
        if self.code {
            Some(SpanStyle::Code)
        } else if self.dim > 0 {
            Some(SpanStyle::Dim)
        } else if self.strong > 0 {
            Some(SpanStyle::Strong)
        } else {
            None
        }
    }

    fn boundary(&mut self, lines: usize) {
        self.breaks = self.breaks.max(lines);
    }

    fn push(&mut self, value: &str, style: Option<SpanStyle>) {
        if value.is_empty() {
            return;
        }
        if !self.out.text.is_empty() {
            let trailing = self
                .out
                .text
                .chars()
                .rev()
                .take_while(|&c| c == '\n')
                .take(self.breaks)
                .count();
            for _ in trailing..self.breaks {
                self.out.push_str("\n");
            }
        }
        self.breaks = 0;
        for part in value.split_inclusive('\n') {
            if part != "\n" && (self.out.text.is_empty() || self.out.text.ends_with('\n')) {
                for _ in 0..self.quotes {
                    self.out.push_str("│ ");
                }
            }
            let start = self.out.text.len();
            self.out.push_str(part);
            if let Some(style) = style {
                // Parser events may split at entities/escapes. Coalesce equal
                // adjacent styles rather than allocating one span per event.
                if let Some(last) = self
                    .out
                    .spans
                    .last_mut()
                    .filter(|last| last.style == style && last.range.end == start)
                {
                    last.range.end = self.out.text.len();
                } else {
                    self.out.spans.push(crate::model::Span {
                        range: start..self.out.text.len(),
                        style,
                    });
                }
            }
        }
    }
}

/// Only backtick-quoted runs become `Code` spans; everything else is
/// verbatim — for diagnostic messages, which quote identifiers in
/// backticks but are not markdown (a `*` in "expected `*const u8`" is
/// text, never emphasis).
pub fn code_spans_only(text: &str) -> StyledText {
    let mut out = StyledText::default();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut literal = String::new();
    while i < chars.len() {
        if chars[i] == '`' {
            let run = chars[i..].iter().take_while(|&&c| c == '`').count();
            if let Some(close) = find_run(&chars, i + run, '`', run) {
                out.push_str(&literal);
                literal.clear();
                let code: String = chars[i + run..close].iter().collect();
                out.push_styled(&code, SpanStyle::Code);
                i = close + run;
                continue;
            }
            literal.extend(&chars[i..i + run]);
            i += run;
            continue;
        }
        literal.push(chars[i]);
        i += 1;
    }
    out.push_str(&literal);
    out
}

/// The plaintext of [`markdown_to_styled`] — for callers that only need
/// text (status transients, automation snapshots).
pub fn markdown_to_plain_text(markdown: &str) -> String {
    markdown_to_styled(markdown).text
}

/// Index of the next run of exactly `len` `marker` chars at or after
/// `from` (longer runs don't match, so ``` `` ` ``` pairs correctly).
fn find_run(chars: &[char], from: usize, marker: char, len: usize) -> Option<usize> {
    let mut i = from;
    while i < chars.len() {
        if chars[i] == marker {
            let run = chars[i..].iter().take_while(|&&c| c == marker).count();
            if run == len {
                return Some(i);
            }
            i += run;
        } else {
            i += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans(t: &StyledText) -> Vec<(&str, SpanStyle)> {
        t.spans
            .iter()
            .map(|s| (&t.text[s.range.clone()], s.style))
            .collect()
    }

    #[test]
    fn inline_code_and_emphasis_become_spans() {
        let t = markdown_to_styled("call `foo()` then **twice** or *once* and __also__");
        assert_eq!(t.text, "call foo() then twice or once and also");
        assert_eq!(
            spans(&t),
            vec![
                ("foo()", SpanStyle::Code),
                ("twice", SpanStyle::Strong),
                ("once", SpanStyle::Strong),
                ("also", SpanStyle::Strong),
            ]
        );
    }

    #[test]
    fn unmatched_markers_and_intraword_underscores_stay_literal() {
        let t = markdown_to_styled("snake_case_name and 5 * 3 * 2 and a_b");
        assert_eq!(t.text, "snake_case_name and 5 * 3 * 2 and a_b");
        assert!(t.spans.is_empty());
        // A backtick-quoted star can't close emphasis.
        let t = markdown_to_styled("*a `*` b*");
        assert_eq!(t.text, "a * b");
        assert_eq!(
            spans(&t),
            vec![
                ("a ", SpanStyle::Strong),
                ("*", SpanStyle::Code),
                (" b", SpanStyle::Strong),
            ]
        );
    }

    #[test]
    fn a_hash_without_a_space_is_not_a_heading() {
        // CommonMark: `#[derive(Debug)]` and `#include` are text, `# Title`
        // is a heading. The old flattener stripped every leading `#`.
        let t = markdown_to_styled("#[derive(Debug)]\n# Title");
        assert_eq!(t.text, "#[derive(Debug)]\nTitle");
        assert_eq!(spans(&t), vec![("Title", SpanStyle::Strong)]);
    }

    #[test]
    fn a_bullet_star_never_opens_emphasis() {
        let t = markdown_to_styled("* first item with *em*\n* second");
        assert_eq!(t.text, "• first item with em\n• second");
        assert_eq!(spans(&t), vec![("em", SpanStyle::Strong)]);
    }

    #[test]
    fn fences_headings_breaks_and_links() {
        let t = markdown_to_styled("## Signature\n```rust\nfn f(a: *const u8)\n```\nsee [docs](https://x) and [valid]\n\n---\nend");
        assert_eq!(
            t.text,
            "Signature\nfn f(a: *const u8)\nsee docs and [valid]\n\nend"
        );
        assert_eq!(
            spans(&t),
            vec![
                ("Signature", SpanStyle::Strong),
                ("fn f(a: *const u8)\n", SpanStyle::Code),
            ]
        );
        // Old flattener behaviour retained for the plaintext view.
        assert_eq!(markdown_to_plain_text("a\n* * *\nb"), "a\n\nb");
        assert_eq!(
            markdown_to_plain_text("wrapped\nprose  \nnext\n\n```\na\nb\n```"),
            "wrapped prose\nnext\n\na\nb"
        );
    }

    #[test]
    fn code_spans_only_leaves_everything_but_backticks_alone() {
        let t = code_spans_only("expected `*const u8`, found *x* and [a](b)");
        assert_eq!(t.text, "expected *const u8, found *x* and [a](b)");
        assert_eq!(spans(&t), vec![("*const u8", SpanStyle::Code)]);
    }

    #[test]
    fn double_backtick_code_can_contain_single_backticks() {
        let t = markdown_to_styled("use `` a`b `` here");
        assert_eq!(t.text, "use a`b here");
        assert_eq!(spans(&t), vec![("a`b", SpanStyle::Code)]);
    }

    #[test]
    fn card_markdown_keeps_links_and_shorter_fences_literal_inside_code() {
        let t =
            markdown_to_styled("````md\n```rust\n[x](url) &amp; \\*\n~~~\n````\nAfter **code**.");
        assert_eq!(t.text, "```rust\n[x](url) &amp; \\*\n~~~\nAfter code.");
        let (code, prose) = t.split_leading_code();
        assert_eq!(code.unwrap().text, "```rust\n[x](url) &amp; \\*\n~~~");
        assert_eq!(prose.text, "After code.");
        let t = markdown_to_styled("`[x](url)` and `&amp;` outside &amp;");
        assert_eq!(t.text, "[x](url) and &amp; outside &");
        assert_eq!(
            spans(&t),
            vec![("[x](url)", SpanStyle::Code), ("&amp;", SpanStyle::Code)]
        );
    }

    #[test]
    fn card_markdown_resolves_links_escapes_and_nested_styles_without_resources() {
        let t = markdown_to_styled("[**hé `猫`**](https://example.invalid/a_(b)) \\*literal\\* &lt;x&gt; [ref][r] ![alt](image.png)\n\n[r]: https://example.invalid/ref");
        assert_eq!(t.text, "hé 猫 *literal* <x> ref alt");
        assert_eq!(
            spans(&t),
            vec![("hé ", SpanStyle::Strong), ("猫", SpanStyle::Code)]
        );
        let html = "<script>alert('text only')</script>";
        assert_eq!(markdown_to_styled(html).text, html);
    }

    #[test]
    fn card_markdown_preserves_nested_list_numbers_tasks_and_quotes() {
        let t = markdown_to_styled(
            "3. first\n   - [x] **nested**\n4. second\n\n> quoted *text*\n> next line",
        );
        assert_eq!(
            t.text,
            "3. first\n  • [x] nested\n4. second\n\n│ quoted text next line"
        );
        assert_eq!(
            spans(&t),
            vec![("nested", SpanStyle::Strong), ("text", SpanStyle::Strong)]
        );
    }

    #[test]
    fn card_markdown_tables_have_separate_cells_and_styled_headers() {
        let t = markdown_to_styled(
            "| Name | Value |\n| --- | --- |\n| `hé` | **yes** |\n| next | ~~old~~ |",
        );
        assert_eq!(t.text, "Name | Value\nhé | yes\nnext | old");
        assert_eq!(
            spans(&t),
            vec![
                ("Name", SpanStyle::Strong),
                ("Value", SpanStyle::Strong),
                ("hé", SpanStyle::Code),
                ("yes", SpanStyle::Strong),
                ("old", SpanStyle::Dim)
            ]
        );
    }

    #[test]
    fn card_markdown_setext_headings_and_indented_code_are_not_discarded() {
        let t = markdown_to_styled(
            "Title\n=====\n\n    [literal](url)\n    *code*\n\nText[^n]\n\n[^n]: the note",
        );
        assert_eq!(
            t.text,
            "Title\n\n[literal](url)\n*code*\n\nText[n]\n\n[n] the note"
        );
        assert_eq!(t.spans[0].style, SpanStyle::Strong);
        assert_eq!(&t.text[t.spans[0].range.clone()], "Title");
        assert!(t
            .spans
            .iter()
            .any(|s| s.style == SpanStyle::Code && t.text[s.range.clone()].contains("*code*")));
    }

    #[test]
    fn card_markdown_spans_stay_ordered_and_utf8_aligned() {
        for source in [
            "***hé `猫` **inner** end***",
            "## **Header** &amp; `x`",
            "`unclosed **thing",
            "~~~\nα\n\nβ\n~~~",
            "",
            "\n",
            "***",
        ] {
            let t = markdown_to_styled(source);
            let mut previous_end = 0;
            for span in &t.spans {
                assert!(previous_end <= span.range.start, "{source}: {t:?}");
                assert!(span.range.start < span.range.end);
                assert!(t.text.get(span.range.clone()).is_some());
                previous_end = span.range.end;
            }
        }
    }

    #[test]
    fn card_markdown_keeps_paragraph_spacing_and_multiline_code_indentation() {
        let t = markdown_to_styled("First paragraph.\n\nSecond **paragraph**.\n\n```rust\nfn f() {\n    call();\n\n    again();\n}\n```\n\nLast.");
        assert_eq!(t.text, "First paragraph.\n\nSecond paragraph.\n\nfn f() {\n    call();\n\n    again();\n}\n\nLast.");
        assert_eq!(&t.text[t.spans[0].range.clone()], "paragraph");
        assert!(t
            .spans
            .iter()
            .any(|s| s.style == SpanStyle::Code
                && t.text[s.range.clone()].contains("\n\n    again();")));
    }
}
