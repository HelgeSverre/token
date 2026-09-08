//! Context-sensitive filters applied on every serve, after cache lookup.
//! Grammar definitions stay in the syntax registry; no ad-hoc string lexer.

use std::fmt;
use std::ops::Range;
use std::time::{Duration, Instant};

use ropey::Rope;
use tree_sitter::{ParseOptions, Parser, Tree};

use crate::model::Document;
use crate::syntax::{registry, LanguageId};
use crate::util::text::{char_col_to_visual_col, TABULATOR_WIDTH};
use crate::util::ByteSize;

const SOURCE_LIMIT: ByteSize = ByteSize::mebibytes(1);
const PARSE_BUDGET: Duration = Duration::from_millis(50);

/// Local-only immutable analysis input. Providers receive InlineRequest, never
/// this source. The worker cache deliberately does not retain this snapshot.
#[derive(Clone)]
pub struct InlineContext {
    source: Rope,
    cursor: usize,
    language: LanguageId,
}

impl fmt::Debug for InlineContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InlineContext")
            .field("source_bytes", &self.source.len_bytes())
            .field("cursor", &self.cursor)
            .field("language", &self.language)
            .finish()
    }
}

impl InlineContext {
    pub fn capture(document: &Document, cursor: (usize, usize)) -> Option<Self> {
        (document.buffer.len_bytes() <= SOURCE_LIMIT.as_usize()
            && registry::language(document.language).parser.is_some()
            && !matches!(
                document.language,
                LanguageId::Markdown | LanguageId::PlainText
            ))
        .then(|| Self {
            source: document.buffer.clone(),
            cursor: document.cursor_to_offset(cursor.0, cursor.1),
            language: document.language,
        })
    }
}

/// Reuses one worker-owned parser, without changing the editor's syntax caches.
#[derive(Default)]
pub struct InlinePostprocessor {
    parser: Parser,
}

impl InlinePostprocessor {
    /// All alternatives share one cooperative work budget, including cache hits.
    pub fn serve_all(&mut self, texts: &[String], context: Option<&InlineContext>) -> Vec<String> {
        let deadline = Instant::now() + PARSE_BUDGET;
        texts
            .iter()
            .filter_map(|text| self.serve(text, context, deadline))
            .collect()
    }

    /// Unknown syntax or exhausted work budget preserves the original candidate.
    /// `None` means the known-invalid candidate has no useful remainder.
    fn serve(
        &mut self,
        text: &str,
        context: Option<&InlineContext>,
        deadline: Instant,
    ) -> Option<String> {
        if Instant::now() >= deadline {
            return Some(text.to_owned());
        }
        let Some(context) = context else {
            return Some(text.to_owned());
        };
        let Some(definition) = registry::language(context.language).parser else {
            return Some(text.to_owned());
        };
        if text.len() > SOURCE_LIMIT.as_usize()
            || self.parser.set_language(&(definition.grammar)()).is_err()
        {
            return Some(text.to_owned());
        }
        let original = context.source.to_string();
        let insertion = context.source.char_to_byte(context.cursor);
        let mut source = String::with_capacity(original.len() + text.len());
        source.push_str(&original[..insertion]);
        source.push_str(text);
        source.push_str(&original[insertion..]);
        let mut cancelled = |_: &tree_sitter::ParseState| Instant::now() >= deadline;
        let tree = self.parser.parse_with_options(
            &mut |offset, _| &source.as_bytes()[offset..],
            None,
            Some(ParseOptions::new().progress_callback(&mut cancelled)),
        );
        // A cancelled parse must never resume on a different candidate.
        self.parser.reset();
        let Some(tree) = tree else {
            return Some(text.to_owned());
        };
        let Some(analysis) = analyze(&tree, &source, insertion..insertion + text.len(), deadline)
        else {
            return Some(text.to_owned());
        };
        let text = &text[..analysis.keep];
        if text.trim().is_empty() {
            return None;
        }
        // In indentation-sensitive grammars, tab expansion can have semantics
        // different from the editor's visual tab stops (Python uses eight).
        // Only normalize grammars whose whitespace/literals are covered here.
        let style = matches!(
            context.language,
            LanguageId::Rust
                | LanguageId::Go
                | LanguageId::JavaScript
                | LanguageId::C
                | LanguageId::Cpp
        )
        .then(|| {
            infer_style(
                &original,
                insertion,
                source.len() - original.len(),
                &analysis.opaque,
            )
        })
        .flatten();
        Some(match style {
            Some(style) => normalize(
                text,
                &original[..insertion],
                insertion,
                style,
                &analysis.opaque,
            ),
            None => text.to_owned(),
        })
    }
}

struct Analysis {
    keep: usize,
    opaque: Vec<Range<usize>>,
}

fn opaque_kind(kind: &str) -> bool {
    kind.contains("string")
        || kind.contains("comment")
        || kind.contains("heredoc")
        || matches!(
            kind,
            "char_literal" | "character_literal" | "regex" | "raw_text" | "cdata"
        )
}

fn analyze(
    tree: &Tree,
    source: &str,
    candidate: Range<usize>,
    deadline: Instant,
) -> Option<Analysis> {
    let mut result = Analysis {
        keep: candidate.len(),
        opaque: Vec::new(),
    };
    let mut stack = Vec::new();
    let mut reliable_prefix = true;
    let mut cursor = tree.walk();
    loop {
        if Instant::now() >= deadline {
            return None;
        }
        let node = cursor.node();
        // Recovery may expose an unfinished literal's contents as punctuation
        // siblings rather than a string node. Do not guess where that literal
        // ends. This also conservatively skips errors containing valid quotes.
        if node.is_error() {
            let recovered = &source[node.byte_range()];
            if recovered.contains(['"', '\'', '`', '/']) {
                return None;
            }
        }
        let opaque = opaque_kind(node.kind());
        if opaque {
            result.opaque.push(node.byte_range());
        } else if node.child_count() == 0 && !node.is_missing() && node.start_byte() < candidate.end
        {
            match node.kind() {
                "(" | "[" | "{" => stack.push(node.kind()),
                ")" | "]" | "}" => {
                    let expected = match node.kind() {
                        ")" => "(",
                        "]" => "[",
                        _ => "{",
                    };
                    if stack.last().copied() == Some(expected) {
                        stack.pop();
                    } else if node.start_byte() < candidate.start {
                        reliable_prefix = false;
                        stack.clear();
                    } else if reliable_prefix || !stack.is_empty() {
                        result.keep = result.keep.min(node.start_byte() - candidate.start);
                    }
                }
                _ => {}
            }
        }
        if !opaque && cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return Some(result);
            }
        }
    }
}

fn is_opaque(byte: usize, ranges: &[Range<usize>]) -> bool {
    let index = ranges.partition_point(|range| range.end <= byte);
    ranges.get(index).is_some_and(|range| range.contains(&byte))
}

#[derive(Clone, Copy)]
enum IndentStyle {
    Tabs,
    Spaces,
}

fn infer_style(
    original: &str,
    insertion: usize,
    added: usize,
    opaque: &[Range<usize>],
) -> Option<IndentStyle> {
    let (mut tabs, mut spaces, mut byte) = (0, 0, 0);
    for line in original.split_inclusive('\n') {
        let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
        let content = byte + indent;
        let edited = content + if content >= insertion { added } else { 0 };
        if indent > 0 && !line.trim().is_empty() && !is_opaque(edited, opaque) {
            if line[..indent].contains('\t') {
                tabs += 1;
            } else {
                spaces += 1;
            }
        }
        byte += line.len();
    }
    match tabs.cmp(&spaces) {
        std::cmp::Ordering::Greater => Some(IndentStyle::Tabs),
        std::cmp::Ordering::Less => Some(IndentStyle::Spaces),
        std::cmp::Ordering::Equal => None,
    }
}

fn normalize(
    text: &str,
    prefix: &str,
    insertion: usize,
    style: IndentStyle,
    opaque: &[Range<usize>],
) -> String {
    let first_prefix = prefix.rsplit_once('\n').map_or(prefix, |(_, line)| line);
    let mut byte = insertion;
    let mut result = String::with_capacity(text.len());
    for (index, line) in text.split_inclusive('\n').enumerate() {
        let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
        let leading = index > 0 || first_prefix.chars().all(|ch| matches!(ch, ' ' | '\t'));
        if leading && indent > 0 && !is_opaque(byte, opaque) && !is_opaque(byte + indent, opaque) {
            let before = if index == 0 { first_prefix } else { "" };
            let start = char_col_to_visual_col(before, before.chars().count());
            let combined = format!("{before}{}", &line[..indent]);
            let end = char_col_to_visual_col(&combined, combined.chars().count());
            let mut column = start;
            while column < end {
                let next = column + TABULATOR_WIDTH - column % TABULATOR_WIDTH;
                if matches!(style, IndentStyle::Tabs) && next <= end {
                    result.push('\t');
                    column = next;
                } else {
                    result.push(' ');
                    column += 1;
                }
            }
            result.push_str(&line[indent..]);
        } else {
            result.push_str(line);
        }
        byte += line.len();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(language: LanguageId, marked: &str) -> InlineContext {
        let (prefix, suffix) = marked.split_once('¦').unwrap();
        let mut document = Document::with_text(&format!("{prefix}{suffix}"));
        document.language = language;
        InlineContext::capture(
            &document,
            (
                prefix.bytes().filter(|b| *b == b'\n').count(),
                prefix.rsplit('\n').next().unwrap().chars().count(),
            ),
        )
        .unwrap()
    }

    fn serve(language: LanguageId, marked: &str, text: &str) -> Option<String> {
        InlinePostprocessor::default().serve(
            text,
            Some(&context(language, marked)),
            Instant::now() + PARSE_BUDGET,
        )
    }

    #[test]
    fn bracket_sanity_uses_prefix_scope_and_truncates_first_impossible_closer() {
        for (marked, text, expected) in [
            ("fn main() { ¦ }", "call()); leaked();", Some("call()")),
            ("fn main() { ¦ }", "] leaked();", None),
            ("fn main() { foo(¦); }", "value)", Some("value)")),
            (
                "fn main() { ¦ }",
                "let x = [1, 2); bad();",
                Some("let x = [1, 2"),
            ),
            (
                "fn main() { ¦ }",
                "if true {\n    next();",
                Some("if true {\n    next();"),
            ),
        ] {
            assert_eq!(
                serve(LanguageId::Rust, marked, text).as_deref(),
                expected,
                "{text}"
            );
        }
    }

    #[test]
    fn literals_comments_regex_and_raw_strings_do_not_supply_brackets() {
        for (language, marked, text) in [
            (
                LanguageId::Rust,
                "fn f() { ¦ }",
                "let s = r###\" ) ] } \"###; let c = '}';",
            ),
            (
                LanguageId::Rust,
                "fn f() { ¦ }",
                "/* } /* ] */ ) */ call(); // ]",
            ),
            (
                LanguageId::JavaScript,
                "function f() { ¦ }",
                "const r = /[})]/; const s = ` ) } `;",
            ),
            (
                LanguageId::Python,
                "def f():\n    ¦\n",
                "s = \"\"\" ) ] } \"\"\" # )",
            ),
            (
                LanguageId::Cpp,
                "void f() { ¦ }",
                "auto s = R\"tag( ) ] } )tag\";",
            ),
        ] {
            assert_eq!(
                serve(language, marked, text).as_deref(),
                Some(text),
                "{language:?}"
            );
        }
    }

    #[test]
    fn incomplete_literals_and_comments_preserve_the_candidate() {
        for (language, marked, text) in [
            (LanguageId::Rust, "fn f() { ¦", "let s = \" ) ] }"),
            (LanguageId::Rust, "fn f() { ¦", "let s = r###\" ) ] }"),
            (LanguageId::Rust, "fn f() { ¦", "/* ) ] }"),
            (LanguageId::Rust, "fn f() { /* unfinished ¦", ") ] }"),
            (
                LanguageId::JavaScript,
                "function f() { ¦",
                "const s = ` ) ] }",
            ),
            (
                LanguageId::JavaScript,
                "function f() { ¦",
                "const r = /[ ) ] }",
            ),
            (LanguageId::Python, "def f():\n    ¦", "s = \"\"\" ) ] }"),
        ] {
            assert_eq!(
                serve(language, marked, text).as_deref(),
                Some(text),
                "{language:?}: {marked} / {text}"
            );
        }
    }

    #[test]
    fn expired_budget_preserves_input_and_does_not_poison_the_next_serve() {
        let mut processor = InlinePostprocessor::default();
        let ctx = context(LanguageId::Rust, "fn f() { ¦ }");
        assert_eq!(
            processor.serve("]", Some(&ctx), Instant::now()).as_deref(),
            Some("]")
        );
        assert!(processor.serve_all(&["]".into()], Some(&ctx)).is_empty());
    }

    #[test]
    fn indentation_preserves_visual_columns_and_first_line_spacing() {
        assert_eq!(
            serve(
                LanguageId::Python,
                "def f():\n    old()\n    ¦\n",
                "next()\n\tmore()"
            )
            .as_deref(),
            Some("next()\n\tmore()"),
            "visual tab stops must not rewrite indentation-sensitive syntax"
        );
        assert_eq!(
            serve(
                LanguageId::Rust,
                "fn f() {\n    old();\n    ¦\n}\n",
                "next();\r\n\tmore();\r\n"
            )
            .as_deref(),
            Some("next();\r\n    more();\r\n")
        );
        assert_eq!(
            serve(
                LanguageId::Go,
                "func f() {\n\told()\n  ¦\n}\n",
                "  next()\n      more()\n"
            )
            .as_deref(),
            Some("\tnext()\n\t  more()\n")
        );
        assert_eq!(
            serve(
                LanguageId::Rust,
                "fn f() {\n    let value =¦;\n}\n",
                "  next()"
            )
            .as_deref(),
            Some("  next()")
        );
    }

    #[test]
    fn multiline_literal_contents_and_ambiguous_style_are_unchanged() {
        let text = "let s = r#\"first\n\tinside\n\"#;\n\toutside();";
        assert_eq!(
            serve(LanguageId::Rust, "fn f() {\n    old();\n    ¦\n}\n", text).as_deref(),
            Some("let s = r#\"first\n\tinside\n\"#;\n    outside();")
        );
        let mixed = "fn f() {\n    spaces();\n\ttabs();\n¦\n}";
        assert_eq!(
            serve(LanguageId::Rust, mixed, "\tthing();").as_deref(),
            Some("\tthing();")
        );
    }

    #[test]
    fn full_local_context_handles_openers_and_literals_beyond_prompt_window() {
        let padding = "a".repeat(5000);
        let marked = format!("fn f() {{ let s = r#\"{padding}¦\"#; }}");
        assert_eq!(
            serve(LanguageId::Rust, &marked, " ) ] }").as_deref(),
            Some(" ) ] }")
        );
        let marked = format!("fn f() {{ foo(/* {padding} */¦); }}");
        assert_eq!(
            serve(LanguageId::Rust, &marked, "value)").as_deref(),
            Some("value)")
        );
    }

    #[test]
    fn unsupported_and_oversize_contexts_are_bounded_and_debug_is_metadata_only() {
        let ctx = context(LanguageId::Rust, "fn f() { /* SECRET_CANARY */ ¦ }");
        assert!(!format!("{ctx:?}").contains("SECRET_CANARY"));
        let document = Document::with_text("plain text");
        assert!(InlineContext::capture(&document, (0, 0)).is_none());
        let mut document = Document::with_text(&" ".repeat(SOURCE_LIMIT.as_usize() + 1));
        document.language = LanguageId::Rust;
        assert!(InlineContext::capture(&document, (0, 0)).is_none());
        assert_eq!(
            InlinePostprocessor::default()
                .serve("literal ]", None, Instant::now() + PARSE_BUDGET)
                .as_deref(),
            Some("literal ]")
        );
    }
}
