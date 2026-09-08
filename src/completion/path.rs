//! Pure path-context recognition and request snapshots. Directory I/O belongs
//! to the runtime; code literals wait for current syntax before requesting it.

use std::path::PathBuf;

use crate::model::{Cursor, Document, DocumentId, EditorId};
use crate::syntax::{LanguageId, HIGHLIGHT_NAMES};

const MAX_LINE_CHARS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
/// A directory resolved lexically, with home expansion deferred to the runtime.
pub enum PathDirectory {
    Local(PathBuf),
    HomeRelative(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Source range and syntax needed to insert one filename component.
pub struct PathContext {
    pub directory: PathDirectory,
    pub query: String,
    pub start: Cursor,
    pub end: Cursor,
    quote: Option<char>,
    markdown: bool,
    continues_directory: bool,
    pub(crate) ready: bool,
}

#[derive(Debug, Clone)]
/// Immutable identity snapshot returned with speculative directory results.
pub struct PathRequest {
    pub explicit: bool,
    pub document_id: DocumentId,
    pub editor_id: EditorId,
    pub revision: u64,
    pub language: LanguageId,
    pub cursors: Vec<Cursor>,
    pub active_cursor_index: usize,
    pub file_path: Option<PathBuf>,
    pub workspace_root: Option<PathBuf>,
    pub context: PathContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// One representable filesystem entry; directory traversal is never recursive.
pub struct PathEntry {
    pub name: String,
    pub is_directory: bool,
}

#[derive(Debug, Clone, Default)]
/// Bounded candidates, with an explicit indication that scanning stopped early.
pub struct PathResults {
    pub entries: Vec<PathEntry>,
    pub truncated: bool,
}

impl PathContext {
    pub(crate) fn at(
        document: &Document,
        cursor: Cursor,
        workspace_root: Option<&std::path::Path>,
        explicit: bool,
    ) -> Option<Self> {
        if cursor.line >= document.line_count() || cursor.column > MAX_LINE_CHARS {
            return None;
        }
        if document.buffer.line(cursor.line).len_chars() > MAX_LINE_CHARS {
            return None;
        }
        let line: Vec<char> = document
            .buffer
            .line(cursor.line)
            .chars()
            .take(MAX_LINE_CHARS + 1)
            .collect();
        if line.len() > MAX_LINE_CHARS || cursor.column > line.len() {
            return None;
        }
        let before = &line[..cursor.column];
        let prose = matches!(
            document.language,
            LanguageId::PlainText | LanguageId::Markdown
        );
        let mut quoted = None;
        let mut escaped = false;
        for (index, &ch) in before.iter().enumerate() {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if let Some((quote, _)) = quoted {
                if ch == quote {
                    quoted = None;
                }
            } else if matches!(ch, '\'' | '"')
                && (!prose || index == 0 || !before[index - 1].is_alphanumeric())
            {
                quoted = Some((ch, index + 1));
            }
        }
        let markdown_start = (document.language == LanguageId::Markdown)
            .then(|| {
                before
                    .windows(2)
                    .rposition(|pair| pair == [']', '('])
                    .map(|index| index + 2)
            })
            .flatten()
            .filter(|&start| !before[start..].contains(&')'));
        let (quote, start, markdown) = if let Some(start) = markdown_start {
            (None, start, true)
        } else if let Some((quote, start)) = quoted {
            (Some(quote), start, false)
        } else if prose {
            let start = before
                .iter()
                .rposition(|ch| ch.is_whitespace())
                .map_or(0, |index| index + 1);
            (None, start, false)
        } else {
            return None;
        };
        let typed: String = before[start..].iter().collect();
        if quote.is_none() && !markdown && typed.contains(['\'', '"']) {
            return None;
        }
        if markdown && typed.contains(['#', '?']) {
            return None;
        }
        // Never turn URLs, escaped source strings, UNC shares, or shell
        // substitutions into speculative filesystem requests.
        if !supported_path_text(&typed)
            || (quote.is_none() && typed.chars().any(char::is_whitespace))
        {
            return None;
        }
        if !typed.contains('/') && !(explicit && (quote.is_some() || markdown)) {
            return None;
        }
        let split = typed.rfind('/').map_or(0, |index| index + 1);
        let (directory, query) = typed.split_at(split);
        let decode = |value: &str| {
            if markdown {
                decode_url_path(value)
            } else {
                Some(value.to_owned())
            }
        };
        let directory = decode(directory)?;
        let query = decode(query)?;
        // URL decoding must not introduce a different path form or separator
        // than the source recognizer allowed. Validate before creating PathBufs.
        if !supported_path_text(&directory)
            || !supported_path_text(&query)
            || query.contains(['/', ':'])
        {
            return None;
        }
        let directory = if let Some(tail) = directory.strip_prefix("~/") {
            PathDirectory::HomeRelative(PathBuf::from(tail.trim_start_matches('/')))
        } else {
            let path = PathBuf::from(directory);
            let path = if path.is_absolute() {
                path
            } else {
                document
                    .file_path
                    .as_deref()
                    .and_then(std::path::Path::parent)
                    .or(workspace_root)?
                    .join(path)
            };
            PathDirectory::Local(path)
        };
        let query_start = start + typed[..split].chars().count();
        let end = cursor.column
            + line[cursor.column..]
                .iter()
                .take_while(|&&ch| {
                    ch != '/'
                        && !ch.is_control()
                        && Some(ch) != quote
                        && !(quote.is_none()
                            && (ch.is_whitespace() || (markdown && matches!(ch, ')' | '"' | '\''))))
                })
                .count();
        let continues_directory = line.get(end) == Some(&'/');
        let end = end + usize::from(continues_directory);
        let ready = if prose {
            true
        } else {
            let fresh = document.syntax_highlights.as_ref().filter(|syntax| {
                syntax.revision == document.revision && syntax.language == document.language
            });
            if let Some(syntax) = fresh {
                let kind = syntax
                    .get_line(cursor.line)
                    .and_then(|line| {
                        line.highlight_at(start).or_else(|| {
                            start
                                .checked_sub(1)
                                .and_then(|column| line.highlight_at(column))
                        })
                    })
                    .and_then(|id| HIGHLIGHT_NAMES.get(id as usize));
                if !kind.is_some_and(|kind| *kind == "string") {
                    return None;
                }
                true
            } else {
                false
            }
        };
        Some(Self {
            directory,
            query,
            start: Cursor::at(cursor.line, query_start),
            end: Cursor::at(cursor.line, end),
            quote,
            markdown,
            continues_directory,
            ready,
        })
    }

    pub(crate) fn compatible_with(&self, other: &Self) -> bool {
        self.directory == other.directory
            && self.query == other.query
            && self.quote == other.quote
            && self.markdown == other.markdown
            && self.continues_directory == other.continues_directory
    }

    /// Preserve source syntax; omit names that require language-specific escape
    /// rules rather than inserting a malformed string or link.
    pub fn insertion(&self, entry: &PathEntry) -> Option<String> {
        if !supported_path_text(&entry.name) {
            return None;
        }
        if self.continues_directory && !entry.is_directory {
            return None;
        }
        if entry
            .name
            .chars()
            .any(|ch| ch.is_control() || matches!(ch, '/' | '\\') || Some(ch) == self.quote)
        {
            return None;
        }
        let mut name = if self.markdown {
            let mut encoded = String::new();
            for ch in entry.name.chars() {
                if matches!(
                    ch,
                    ' ' | '%' | '(' | ')' | '"' | '\'' | '#' | '?' | '<' | '>'
                ) {
                    use std::fmt::Write;
                    let _ = write!(encoded, "%{:02X}", ch as u32);
                } else {
                    encoded.push(ch);
                }
            }
            encoded
        } else {
            if self.quote.is_none()
                && (entry.name.chars().any(char::is_whitespace) || entry.name.contains(['\'', '"']))
            {
                return None;
            }
            entry.name.clone()
        };
        if entry.is_directory {
            name.push('/');
        }
        Some(name)
    }
}

fn decode_url_path(value: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(value.len());
    let mut input = value.bytes();
    while let Some(byte) = input.next() {
        bytes.push(if byte == b'%' {
            let high = char::from(input.next()?).to_digit(16)?;
            let low = char::from(input.next()?).to_digit(16)?;
            (high * 16 + low) as u8
        } else {
            byte
        });
    }
    String::from_utf8(bytes).ok()
}

fn supported_path_text(text: &str) -> bool {
    !text.starts_with("//")
        && !text.contains("://")
        && !text
            .chars()
            .any(|ch| ch.is_control() || matches!(ch, '\\' | '$' | '`' | '<' | '>'))
        && (!text.contains(':')
            || (cfg!(windows)
                && text.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
                && text.as_bytes().get(1) == Some(&b':')
                && text.as_bytes().get(2) == Some(&b'/')
                && !text[2..].contains(':')))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(marked: &str, language: LanguageId, explicit: bool) -> Option<PathContext> {
        let byte = marked.find('|').unwrap();
        let column = marked[..byte].chars().count();
        let mut doc = Document::with_text(&marked.replace('|', ""));
        doc.file_path = Some(PathBuf::from("/project/src/main.rs"));
        doc.language = language;
        doc.syntax_highlights = Some(crate::syntax::ParserState::new().parse_and_highlight(
            &doc.buffer.to_string(),
            language,
            DocumentId(1),
            doc.revision,
        ));
        PathContext::at(&doc, Cursor::at(0, column), None, explicit)
    }

    #[test]
    fn path_completion_context_recognizes_paths_not_code_comments_or_urls() {
        for (text, language, explicit, query) in [
            ("let s = \"./ass|\";", LanguageId::Rust, false, "ass"),
            (
                "let s = r#\"../assets/a-b.|\"#;",
                LanguageId::Rust,
                false,
                "a-b.",
            ),
            (
                "open(\"assets/hello w|\")",
                LanguageId::Python,
                false,
                "hello w",
            ),
            ("open('./hé|')", LanguageId::Python, false, "hé"),
            ("see ./as|", LanguageId::PlainText, false, "as"),
            (
                "[image](../assets/hé%20w|)",
                LanguageId::Markdown,
                false,
                "hé w",
            ),
            ("let s = \"ass|\";", LanguageId::Rust, true, "ass"),
            ("[image](|)", LanguageId::Markdown, true, ""),
        ] {
            let found =
                context(text, language, explicit).unwrap_or_else(|| panic!("missing path: {text}"));
            assert!(found.ready, "{text}");
            assert_eq!(found.query, query, "{text}");
        }
        for (text, language, explicit) in [
            ("builder.fi|", LanguageId::Rust, true),
            (r#"const r = /["./as|]/;"#, LanguageId::JavaScript, true),
            ("a / b|", LanguageId::Rust, true),
            ("// see \"./ass|\"", LanguageId::Rust, true),
            ("let s = \"hello|\";", LanguageId::Rust, false),
            ("let s = \"./assets\"|;", LanguageId::Rust, true),
            ("see \"./assets/\"|", LanguageId::PlainText, true),
            ("https://example.test/ass|", LanguageId::PlainText, true),
            ("//server/share/ass|", LanguageId::PlainText, true),
            ("[image](./a%ZZ|)", LanguageId::Markdown, true),
            ("[image](./dir%5Cname/as|)", LanguageId::Markdown, true),
            ("[image](./dir%00name/as|)", LanguageId::Markdown, true),
            ("[image](./as%2Fname|)", LanguageId::Markdown, true),
            ("[image](./a.md#section|)", LanguageId::Markdown, true),
            ("[image](./a.md?query|)", LanguageId::Markdown, true),
            ("let s = \"./a\\nb|\";", LanguageId::Rust, true),
        ] {
            assert!(
                context(text, language, explicit).is_none(),
                "unexpected path: {text}"
            );
        }
    }

    #[test]
    fn path_completion_context_bounds_and_requires_current_syntax() {
        let mut doc = Document::with_text("let s = \"./assets\";");
        doc.language = LanguageId::Rust;
        doc.file_path = Some(PathBuf::from("/project/main.rs"));
        let cursor = Cursor::at(0, 13);
        assert!(!PathContext::at(&doc, cursor, None, false).unwrap().ready);
        doc.syntax_highlights = Some(crate::syntax::ParserState::new().parse_and_highlight(
            &doc.buffer.to_string(),
            doc.language,
            DocumentId(1),
            doc.revision,
        ));
        assert!(PathContext::at(&doc, cursor, None, false).unwrap().ready);
        doc.revision += 1;
        assert!(!PathContext::at(&doc, cursor, None, false).unwrap().ready);
        doc.buffer = format!("\"./{}\"", "a".repeat(MAX_LINE_CHARS)).into();
        assert!(PathContext::at(&doc, Cursor::at(0, MAX_LINE_CHARS), None, true).is_none());
    }

    #[test]
    fn path_completion_resolves_file_workspace_and_explicit_roots_without_io() {
        let mut doc = Document::with_text("../as");
        let cursor = Cursor::at(0, 5);
        assert!(PathContext::at(&doc, cursor, None, false).is_none());
        let workspace = std::path::Path::new("/workspace");
        assert_eq!(
            PathContext::at(&doc, cursor, Some(workspace), false)
                .unwrap()
                .directory,
            PathDirectory::Local(workspace.join("../"))
        );
        doc.file_path = Some("/project/src/main.txt".into());
        assert_eq!(
            PathContext::at(&doc, cursor, Some(workspace), false)
                .unwrap()
                .directory,
            PathDirectory::Local(std::path::Path::new("/project/src").join("../"))
        );
        doc.file_path = None;
        doc.buffer = "Here's ./as".into();
        assert_eq!(
            PathContext::at(&doc, Cursor::at(0, 11), Some(workspace), false)
                .unwrap()
                .directory,
            PathDirectory::Local(workspace.join("./"))
        );
        doc.buffer = "~//Doc".into();
        assert_eq!(
            PathContext::at(&doc, Cursor::at(0, 6), None, false)
                .unwrap()
                .directory,
            PathDirectory::HomeRelative(PathBuf::new())
        );
        #[cfg(unix)]
        {
            doc.buffer = "/tmp/as".into();
            assert_eq!(
                PathContext::at(&doc, Cursor::at(0, 7), None, false)
                    .unwrap()
                    .directory,
                PathDirectory::Local("/tmp/".into())
            );
        }
    }

    #[test]
    fn path_completion_insertion_preserves_unicode_spaces_and_component_suffixes() {
        let quoted = context("let s = \"./hé|old\";", LanguageId::Rust, false).unwrap();
        assert_eq!(quoted.end.column - quoted.start.column, 5);
        let file = PathEntry {
            name: "hé llo.md".into(),
            is_directory: false,
        };
        assert_eq!(quoted.insertion(&file).unwrap(), "hé llo.md");
        let markdown = context("[x](./hé|)", LanguageId::Markdown, false).unwrap();
        assert_eq!(markdown.insertion(&file).unwrap(), "hé%20llo.md");
        let bare = context("./hé|", LanguageId::PlainText, false).unwrap();
        assert!(bare.insertion(&file).is_none());
        let middle = context("./as|/file", LanguageId::PlainText, false).unwrap();
        assert!(middle.insertion(&file).is_none());
        assert_eq!(middle.end.column, 5);
        assert_eq!(
            middle
                .insertion(&PathEntry {
                    name: "assets".into(),
                    is_directory: true
                })
                .unwrap(),
            "assets/"
        );
    }
}
