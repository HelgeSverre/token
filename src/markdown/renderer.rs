//! Markdown to HTML renderer using pulldown-cmark

use pulldown_cmark::{html, Event, Options, Parser, Tag, TagEnd};

use super::PreviewTheme;
use crate::syntax::LanguageId;

/// Convert markdown to a complete HTML document with styling
pub fn markdown_to_html(markdown: &str, theme: &PreviewTheme) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;

    let parser = Parser::new_ext(markdown, options);

    // Add line markers for scroll sync
    let parser_with_markers = add_line_markers(parser, markdown);

    let mut html_output = String::new();
    html::push_html(&mut html_output, parser_with_markers);

    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/styles/github-dark.min.css">
    <style>{}</style>
</head>
<body>
    <div id="content">{}</div>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/highlight.min.js"></script>
    <script>{}</script>
</body>
</html>"#,
        generate_css(theme),
        html_output,
        SCROLL_SYNC_JS
    )
}

/// Convert content to preview HTML based on language type
///
/// For Markdown files, renders the markdown to styled HTML.
/// For HTML files, displays the raw HTML content directly.
/// Returns None for languages that don't support preview.
pub fn content_to_preview_html(
    content: &str,
    language: LanguageId,
    theme: &PreviewTheme,
) -> Option<String> {
    match language {
        LanguageId::Markdown => Some(markdown_to_html(content, theme)),
        LanguageId::Html => Some(html_to_preview(content)),
        _ => None,
    }
}

/// Wrap raw HTML content for preview display
///
/// For HTML files, we display the content as-is without additional styling.
/// This allows the HTML's own styles to take effect.
fn html_to_preview(html_content: &str) -> String {
    // Check if the content is a complete HTML document
    let trimmed = html_content.trim_start().to_lowercase();
    if trimmed.starts_with("<!doctype") || trimmed.starts_with("<html") {
        // Already a complete document - use as-is
        html_content.to_string()
    } else {
        // Fragment - wrap in minimal document structure
        format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
</head>
<body>
{}
</body>
</html>"#,
            html_content
        )
    }
}

/// Generate CSS from theme colors
fn generate_css(theme: &PreviewTheme) -> String {
    format!(
        r#"
* {{
    box-sizing: border-box;
}}

body {{
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif;
    font-size: 14px;
    line-height: 1.6;
    color: {text};
    background: {background};
    padding: 20px;
    max-width: 800px;
    margin: 0 auto;
}}

h1, h2, h3, h4, h5, h6 {{
    color: {heading};
    margin-top: 24px;
    margin-bottom: 16px;
    font-weight: 600;
    line-height: 1.25;
}}

h1 {{
    font-size: 2em;
    border-bottom: 1px solid {border};
    padding-bottom: 0.3em;
}}

h2 {{
    font-size: 1.5em;
    border-bottom: 1px solid {border};
    padding-bottom: 0.3em;
}}

h3 {{
    font-size: 1.25em;
}}

h4 {{
    font-size: 1em;
}}

h5 {{
    font-size: 0.875em;
}}

h6 {{
    font-size: 0.85em;
    color: {muted};
}}

p {{
    margin-top: 0;
    margin-bottom: 16px;
}}

code {{
    background: {code_background};
    padding: 0.2em 0.4em;
    border-radius: 3px;
    font-family: "SF Mono", "Fira Code", Consolas, "Liberation Mono", Menlo, Courier, monospace;
    font-size: 0.9em;
}}

pre {{
    background: {code_background};
    padding: 16px;
    border-radius: 6px;
    overflow-x: auto;
    margin-top: 0;
    margin-bottom: 16px;
}}

pre code {{
    background: none;
    padding: 0;
    font-size: 0.875em;
    line-height: 1.45;
}}

blockquote {{
    border-left: 4px solid {accent};
    margin: 0 0 16px 0;
    padding: 0 16px;
    color: {muted};
}}

blockquote > :first-child {{
    margin-top: 0;
}}

blockquote > :last-child {{
    margin-bottom: 0;
}}

a {{
    color: {link};
    text-decoration: none;
}}

a:hover {{
    text-decoration: underline;
}}

ul, ol {{
    padding-left: 2em;
    margin-top: 0;
    margin-bottom: 16px;
}}

li {{
    margin-bottom: 0.25em;
}}

li > p {{
    margin-top: 16px;
}}

li + li {{
    margin-top: 0.25em;
}}

hr {{
    height: 0.25em;
    padding: 0;
    margin: 24px 0;
    background-color: {border};
    border: 0;
}}

table {{
    border-collapse: collapse;
    border-spacing: 0;
    margin-bottom: 16px;
    width: 100%;
    overflow: auto;
}}

th, td {{
    padding: 6px 13px;
    border: 1px solid {border};
}}

th {{
    font-weight: 600;
    background: {code_background};
}}

tr:nth-child(2n) {{
    background: {code_background};
}}

img {{
    max-width: 100%;
    box-sizing: content-box;
}}

input[type="checkbox"] {{
    margin-right: 0.5em;
}}

.task-list-item {{
    list-style-type: none;
}}

.task-list-item + .task-list-item {{
    margin-top: 3px;
}}

.task-list-item input {{
    margin: 0 0.2em 0.25em -1.6em;
    vertical-align: middle;
}}

del {{
    color: {muted};
}}

[data-line] {{
    scroll-margin-top: 20px;
}}
"#,
        text = theme.text,
        background = theme.background,
        heading = theme.heading,
        link = theme.link,
        code_background = theme.code_background,
        border = theme.border,
        accent = theme.accent,
        muted = theme.muted,
    )
}

/// JavaScript for scroll synchronization and syntax highlighting
const SCROLL_SYNC_JS: &str = r#"
// Initialize syntax highlighting
if (typeof hljs !== 'undefined') {
    hljs.highlightAll();
}

// Scroll to a specific source line
window.scrollToLine = function(line) {
    const el = document.querySelector(`[data-line="${line}"]`);
    if (el) {
        el.scrollIntoView({ behavior: 'smooth', block: 'start' });
    }
};

// Report scroll position back to editor
let scrollTimeout = null;
window.addEventListener('scroll', function() {
    if (scrollTimeout) clearTimeout(scrollTimeout);
    scrollTimeout = setTimeout(function() {
        const elements = document.querySelectorAll('[data-line]');
        let visibleLine = null;
        const viewportTop = window.scrollY;
        
        for (const el of elements) {
            const rect = el.getBoundingClientRect();
            if (rect.top >= 0) {
                visibleLine = parseInt(el.getAttribute('data-line'), 10);
                break;
            }
        }
        
        if (visibleLine !== null && window.webkit && window.webkit.messageHandlers) {
            window.webkit.messageHandlers.scrollSync.postMessage({ line: visibleLine });
        }
    }, 100);
});
"#;

/// Add data-line attributes to block-level elements for scroll sync
fn add_line_markers<'a>(parser: Parser<'a>, markdown: &'a str) -> impl Iterator<Item = Event<'a>> {
    let mut current_line = 1;
    let mut last_offset = 0;

    parser.into_offset_iter().flat_map(move |(event, range)| {
        // Update line number based on offset (guard against non-monotonic offsets)
        if range.start >= last_offset {
            let new_lines = markdown[last_offset..range.start]
                .chars()
                .filter(|c| *c == '\n')
                .count();
            current_line += new_lines;
            last_offset = range.start;
        }

        match &event {
            Event::Start(tag) => {
                let line = current_line;
                match tag {
                    Tag::Heading { .. }
                    | Tag::Paragraph
                    | Tag::BlockQuote(_)
                    | Tag::CodeBlock(_)
                    | Tag::List(_)
                    | Tag::Item => {
                        // Insert a span with data-line before the element
                        vec![
                            Event::Html(format!(r#"<span data-line="{}"></span>"#, line).into()),
                            event,
                        ]
                    }
                    _ => vec![event],
                }
            }
            Event::End(TagEnd::Heading(_))
            | Event::End(TagEnd::Paragraph)
            | Event::End(TagEnd::BlockQuote(_))
            | Event::End(TagEnd::CodeBlock)
            | Event::End(TagEnd::List(_))
            | Event::End(TagEnd::Item) => vec![event],
            _ => vec![event],
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_to_html_basic() {
        let md = "# Hello\n\nWorld";
        let theme = PreviewTheme::default();
        let html = markdown_to_html(md, &theme);

        assert!(html.contains("<h1>"));
        assert!(html.contains("Hello"));
        assert!(html.contains("<p>"));
        assert!(html.contains("World"));
    }

    #[test]
    fn test_markdown_to_html_code_block() {
        let md = "```rust\nfn main() {}\n```";
        let theme = PreviewTheme::default();
        let html = markdown_to_html(md, &theme);

        assert!(html.contains("<pre>"));
        assert!(html.contains("<code"));
        assert!(html.contains("fn main()"));
    }

    #[test]
    fn test_markdown_to_html_links() {
        let md = "[Click here](https://example.com)";
        let theme = PreviewTheme::default();
        let html = markdown_to_html(md, &theme);

        assert!(html.contains("<a"));
        assert!(html.contains("href=\"https://example.com\""));
        assert!(html.contains("Click here"));
    }

    #[test]
    fn test_markdown_to_html_tables() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |";
        let theme = PreviewTheme::default();
        let html = markdown_to_html(md, &theme);

        assert!(html.contains("<table>"));
        assert!(html.contains("<th>"));
        assert!(html.contains("<td>"));
    }

    #[test]
    fn test_preview_theme_default() {
        let theme = PreviewTheme::default();
        assert!(!theme.background.is_empty());
        assert!(!theme.text.is_empty());
        assert!(theme.background.starts_with('#'));
    }

    #[test]
    fn test_content_to_preview_html_markdown() {
        let md = "# Hello\n\nWorld";
        let theme = PreviewTheme::default();
        let result = content_to_preview_html(md, LanguageId::Markdown, &theme);

        assert!(result.is_some());
        let html = result.unwrap();
        assert!(html.contains("<h1>"));
        assert!(html.contains("Hello"));
    }

    #[test]
    fn test_content_to_preview_html_complete_document() {
        let html_content = "<!DOCTYPE html><html><head></head><body><h1>Test</h1></body></html>";
        let theme = PreviewTheme::default();
        let result = content_to_preview_html(html_content, LanguageId::Html, &theme);

        assert!(result.is_some());
        let html = result.unwrap();
        // Complete documents should be used as-is
        assert_eq!(html, html_content);
    }

    #[test]
    fn test_content_to_preview_html_fragment() {
        let html_fragment = "<h1>Hello</h1><p>World</p>";
        let theme = PreviewTheme::default();
        let result = content_to_preview_html(html_fragment, LanguageId::Html, &theme);

        assert!(result.is_some());
        let html = result.unwrap();
        // Fragments should be wrapped in a document structure
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<body>"));
        assert!(html.contains("<h1>Hello</h1>"));
    }

    #[test]
    fn test_content_to_preview_html_unsupported() {
        let rust_code = "fn main() {}";
        let theme = PreviewTheme::default();
        let result = content_to_preview_html(rust_code, LanguageId::Rust, &theme);

        assert!(result.is_none());
    }
}
