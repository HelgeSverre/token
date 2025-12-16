//! CSV parsing using the csv crate
//!
//! RFC 4180 compliant parsing with support for quoted fields,
//! escaped quotes, and custom delimiters.

use super::model::{CsvData, Delimiter};
use std::io::Cursor;

/// Error type for CSV parsing
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub line: Option<usize>,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.line {
            Some(line) => write!(f, "CSV parse error at line {}: {}", line, self.message),
            None => write!(f, "CSV parse error: {}", self.message),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse CSV content into CsvData
///
/// Uses the csv crate for RFC 4180 compliant parsing.
pub fn parse_csv(content: &str, delimiter: Delimiter) -> Result<CsvData, ParseError> {
    let cursor = Cursor::new(content.as_bytes());

    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter.char() as u8)
        .has_headers(false)
        .flexible(true)
        .from_reader(cursor);

    let mut rows: Vec<Vec<String>> = Vec::new();

    for (line_num, result) in reader.records().enumerate() {
        match result {
            Ok(record) => {
                let row: Vec<String> = record.iter().map(|s| s.to_string()).collect();
                rows.push(row);
            }
            Err(e) => {
                return Err(ParseError {
                    message: e.to_string(),
                    line: Some(line_num + 1),
                });
            }
        }
    }

    Ok(CsvData::from_rows(rows))
}

/// Detect delimiter by analyzing first few lines
pub fn detect_delimiter(content: &str) -> Delimiter {
    let first_lines: String = content.lines().take(5).collect::<Vec<_>>().join("\n");

    let comma_count = first_lines.matches(',').count();
    let tab_count = first_lines.matches('\t').count();
    let pipe_count = first_lines.matches('|').count();
    let semi_count = first_lines.matches(';').count();

    let max = comma_count.max(tab_count).max(pipe_count).max(semi_count);

    if max == 0 {
        return Delimiter::Comma;
    }

    if tab_count == max {
        Delimiter::Tab
    } else if pipe_count == max {
        Delimiter::Pipe
    } else if semi_count == max {
        Delimiter::Semicolon
    } else {
        Delimiter::Comma
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_csv() {
        let content = "a,b,c\n1,2,3\n";
        let data = parse_csv(content, Delimiter::Comma).unwrap();

        assert_eq!(data.row_count(), 2);
        assert_eq!(data.column_count(), 3);
        assert_eq!(data.get(0, 0), "a");
        assert_eq!(data.get(1, 2), "3");
    }

    #[test]
    fn test_parse_quoted_fields() {
        let content = r#""hello, world","test"
"with ""quotes""","normal"
"#;
        let data = parse_csv(content, Delimiter::Comma).unwrap();

        assert_eq!(data.get(0, 0), "hello, world");
        assert_eq!(data.get(1, 0), "with \"quotes\"");
    }

    #[test]
    fn test_parse_tsv() {
        let content = "a\tb\tc\n1\t2\t3\n";
        let data = parse_csv(content, Delimiter::Tab).unwrap();

        assert_eq!(data.row_count(), 2);
        assert_eq!(data.get(0, 1), "b");
    }

    #[test]
    fn test_parse_ragged_rows() {
        let content = "a,b,c\n1,2\n";
        let data = parse_csv(content, Delimiter::Comma).unwrap();

        assert_eq!(data.column_count(), 3);
        assert_eq!(data.get(1, 2), "");
    }

    #[test]
    fn test_detect_delimiter_comma() {
        let content = "a,b,c\n1,2,3\n";
        assert_eq!(detect_delimiter(content), Delimiter::Comma);
    }

    #[test]
    fn test_detect_delimiter_tab() {
        let content = "a\tb\tc\n1\t2\t3\n";
        assert_eq!(detect_delimiter(content), Delimiter::Tab);
    }

    #[test]
    fn test_detect_delimiter_pipe() {
        let content = "a|b|c\n1|2|3\n";
        assert_eq!(detect_delimiter(content), Delimiter::Pipe);
    }

    #[test]
    fn test_detect_delimiter_semicolon() {
        let content = "a;b;c\n1;2;3\n";
        assert_eq!(detect_delimiter(content), Delimiter::Semicolon);
    }

    #[test]
    fn test_parse_empty() {
        let data = parse_csv("", Delimiter::Comma).unwrap();
        assert!(data.is_empty());
    }

    #[test]
    fn test_parse_single_column() {
        let content = "a\nb\nc\n";
        let data = parse_csv(content, Delimiter::Comma).unwrap();

        assert_eq!(data.row_count(), 3);
        assert_eq!(data.column_count(), 1);
    }
}
