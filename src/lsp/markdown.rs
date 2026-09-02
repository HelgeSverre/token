//! Light markdown -> [`StyledText`] for the overlay cards. Not a markdown
//! parser: the monospace cards render a handful of constructs — fenced and
//! inline code, emphasis, headings, links, thematic breaks — and everything
//! else passes through as text. Markers only become spans when they pair
//! up; a lone `*` or an intraword `_` (`snake_case`) stays literal, which
//! the old strip-everything flattener got wrong.

use crate::model::{SpanStyle, StyledText};

/// Reduces `markdown` to text + spans. Code fences keep their contents
/// verbatim as `Code` lines; headings become `Strong` lines; thematic
/// breaks become blank separators.
pub fn markdown_to_styled(markdown: &str) -> StyledText {
    let mut out = StyledText::default();
    let mut in_fence = false;
    let mut first = true;
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if !first {
            out.push_str("\n");
        }
        first = false;
        if in_fence {
            out.push_styled(line, SpanStyle::Code);
            continue;
        }
        let is_break =
            trimmed.len() >= 3 && trimmed.chars().all(|c| matches!(c, '-' | '*' | '_' | ' '));
        if is_break && !trimmed.is_empty() {
            continue;
        }
        let heading = trimmed.trim_start_matches('#');
        if heading.len() != trimmed.len() && heading.starts_with(' ')
            || heading.is_empty() && !trimmed.is_empty() && trimmed.starts_with('#')
        {
            let body = strip_inline_links(heading.trim_start());
            let mut inner = StyledText::default();
            push_inline(&mut inner, &body);
            // A heading is Strong throughout: replace inner emphasis with
            // one span (code chips inside a heading stay chips).
            let start = out.text.len();
            out.extend(&inner);
            strong_around(&mut out, start);
            continue;
        }
        push_inline(&mut out, &strip_inline_links(line));
    }
    out
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

/// Inline constructs of one line: `` `code` `` (any backtick run length,
/// no nesting), `**strong**` / `__strong__`, `*em*` / `_em_`. Emphasis
/// contents are parsed recursively but rendered as one `Strong` run.
fn push_inline(out: &mut StyledText, line: &str) {
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    let mut literal = String::new();
    let flush = |out: &mut StyledText, literal: &mut String| {
        if !literal.is_empty() {
            out.push_str(literal);
            literal.clear();
        }
    };
    while i < chars.len() {
        let c = chars[i];
        if c == '`' {
            let run = chars[i..].iter().take_while(|&&c| c == '`').count();
            if let Some(close) = find_run(&chars, i + run, '`', run) {
                flush(out, &mut literal);
                let code: String = chars[i + run..close].iter().collect();
                out.push_styled(code.trim_matches(' '), SpanStyle::Code);
                i = close + run;
                continue;
            }
            literal.extend(&chars[i..i + run]);
            i += run;
            continue;
        }
        if c == '*' || c == '_' {
            let run = chars[i..].iter().take_while(|&&x| x == c).count().min(2);
            let word_bound_open = c == '*' || i == 0 || !chars[i - 1].is_alphanumeric();
            if word_bound_open {
                if let Some(close) = find_emphasis_close(&chars, i + run, c, run) {
                    if close > i + run {
                        flush(out, &mut literal);
                        let inner: String = chars[i + run..close].iter().collect();
                        let mut styled = StyledText::default();
                        push_inline(&mut styled, &inner);
                        let start = out.text.len();
                        out.extend(&styled);
                        strong_around(out, start);
                        i = close + run;
                        continue;
                    }
                }
            }
            literal.extend(&chars[i..i + run]);
            i += run;
            continue;
        }
        literal.push(c);
        i += 1;
    }
    flush(out, &mut literal);
}

/// Makes `out.text[start..]` `Strong`: nested emphasis spans are dropped
/// (one weight only) and code chips are kept, with `Strong` filling the
/// gaps between them so spans stay non-overlapping.
fn strong_around(out: &mut StyledText, start: usize) {
    let end = out.text.len();
    out.spans
        .retain(|s| s.range.start < start || s.style == SpanStyle::Code);
    let keep: Vec<std::ops::Range<usize>> = out
        .spans
        .iter()
        .filter(|s| s.range.start >= start)
        .map(|s| s.range.clone())
        .collect();
    let mut at = start;
    for code in keep.iter().chain(std::iter::once(&(end..end))) {
        if at < code.start {
            out.spans.push(crate::model::Span {
                range: at..code.start,
                style: SpanStyle::Strong,
            });
        }
        at = code.end;
    }
    out.spans.sort_by_key(|s| s.range.start);
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

/// Closing emphasis run: same marker and length, not followed by an
/// alphanumeric for `_` (intraword underscores are literal), and not
/// preceded by whitespace (`* ` is a bullet, not a closer).
fn find_emphasis_close(chars: &[char], from: usize, marker: char, len: usize) -> Option<usize> {
    let mut i = from;
    while i < chars.len() {
        if chars[i] == marker {
            let run = chars[i..].iter().take_while(|&&c| c == marker).count();
            let after_ok = marker == '*' || chars.get(i + run).is_none_or(|c| !c.is_alphanumeric());
            let before_ok = i > 0 && !chars[i - 1].is_whitespace();
            if run == len && after_ok && before_ok {
                return Some(i);
            }
            i += run;
        } else if chars[i] == '`' {
            // Skip over inline code so a backtick-quoted `*` can't close.
            let run = chars[i..].iter().take_while(|&&c| c == '`').count();
            match find_run(chars, i + run, '`', run) {
                Some(close) => i = close + run,
                None => i += run,
            }
        } else {
            i += 1;
        }
    }
    None
}

/// `[text](url)` -> `text`, non-greedy, leaving unmatched brackets intact
/// (rustdoc's bare `[refs]` are plain intra-doc names).
fn strip_inline_links(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find('[') {
        let Some(close_rel) = rest[open..].find(']') else {
            break;
        };
        let close = open + close_rel;
        if rest[close + 1..].starts_with('(') {
            if let Some(paren_rel) = rest[close + 1..].find(')') {
                out.push_str(&rest[..open]);
                out.push_str(&rest[open + 1..close]);
                rest = &rest[close + 1 + paren_rel + 1..];
                continue;
            }
        }
        out.push_str(&rest[..close + 1]);
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    out
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
    fn fences_headings_breaks_and_links() {
        let t = markdown_to_styled("## Signature\n```rust\nfn f(a: *const u8)\n```\nsee [docs](https://x) and [valid]\n---\nend");
        assert_eq!(
            t.text,
            "Signature\nfn f(a: *const u8)\nsee docs and [valid]\n\nend"
        );
        assert_eq!(
            spans(&t),
            vec![
                ("Signature", SpanStyle::Strong),
                ("fn f(a: *const u8)", SpanStyle::Code),
            ]
        );
        // Old flattener behaviour retained for the plaintext view.
        assert_eq!(markdown_to_plain_text("a\n* * *\nb"), "a\n\nb");
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
}
