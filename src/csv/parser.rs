//! CSV parsing using the csv crate
//!
//! RFC 4180 compliant parsing with support for quoted fields,
//! escaped quotes, and custom delimiters.

use super::model::{CsvData, Delimiter};
use std::io::{self, Read};

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
    parse_reader(content.as_bytes(), delimiter)
}

/// Parse a document without flattening its rope into another full-size string.
pub fn parse_csv_rope(content: &ropey::Rope, delimiter: Delimiter) -> Result<CsvData, ParseError> {
    let mut data = parse_reader(
        RopeReader {
            chunks: content.chunks(),
            remaining: &[],
        },
        delimiter,
    )?;
    data.source = Some(content.clone());
    Ok(data)
}

struct RopeReader<'a> {
    chunks: ropey::iter::Chunks<'a>,
    remaining: &'a [u8],
}

impl Read for RopeReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        while self.remaining.is_empty() {
            match self.chunks.next() {
                Some(chunk) => self.remaining = chunk.as_bytes(),
                None => return Ok(0),
            }
        }
        self.remaining.read(buffer)
    }
}

fn parse_reader(input: impl Read, delimiter: Delimiter) -> Result<CsvData, ParseError> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter.char() as u8)
        .has_headers(false)
        .flexible(true)
        .from_reader(input);

    let mut data = CsvData::new();
    let mut record = csv::StringRecord::new();
    loop {
        match reader.read_record(&mut record) {
            Ok(true) => data.push_record(&record, reader.position().byte() as usize),
            Ok(false) => return Ok(data),
            Err(error) => {
                return Err(ParseError {
                    message: error.to_string(),
                    line: Some(data.row_count() + 1),
                })
            }
        }
    }
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

/// Escape a value for CSV output
///
/// Adds quotes if the value contains the delimiter, quotes, or newlines.
/// Existing quotes are doubled per RFC 4180.
pub fn escape_csv_value(value: &str, delimiter: Delimiter) -> String {
    let delim = delimiter.char();
    let needs_quotes = value.contains(delim)
        || value.contains('"')
        || value.contains('\n')
        || value.contains('\r');

    if needs_quotes {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
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
    fn test_parse_rope_spanning_chunks_and_ragged_records() {
        let long_value = "æøå🦀".repeat(2048);
        let text = format!(
            "\"{long_value},quoted\r\nsecond line\",\"a \"\"quote\"\"\",\r\nshort\r\nlast,end,3,4"
        );
        let rope = ropey::Rope::from_str(&text);
        assert!(rope.chunks().count() > 2);
        let data = parse_csv_rope(&rope, Delimiter::Comma).unwrap();
        assert_eq!(data.row_count(), 3);
        assert_eq!(data.column_count(), 4);
        assert_eq!(
            data.get(0, 0),
            format!("{long_value},quoted\r\nsecond line")
        );
        assert_eq!(data.get(0, 1), "a \"quote\"");
        assert_eq!(data.get(0, 2), "");
        assert_eq!(data.get(1, 0), "short");
        assert_eq!(data.get(1, 1), "");
        assert_eq!(data.get(2, 3), "4");
        assert!(parse_csv_rope(&ropey::Rope::new(), Delimiter::Comma)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn test_parse_reader_one_byte_reads() {
        struct OneByte<'a>(&'a [u8]);
        impl Read for OneByte<'_> {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                let length = buffer.len().min(1);
                self.0.read(&mut buffer[..length])
            }
        }
        let data = parse_reader(
            OneByte("\"æ\tø\"\t\"a\"\"b\"\r\n\"x\ny\"\tå".as_bytes()),
            Delimiter::Tab,
        )
        .unwrap();
        assert_eq!(data.row_count(), 2);
        assert_eq!(data.column_count(), 2);
        assert_eq!(data.get(0, 0), "æ\tø");
        assert_eq!(data.get(0, 1), "a\"b");
        assert_eq!(data.get(1, 0), "x\ny");
        assert_eq!(data.get(1, 1), "å");
    }

    #[test]
    fn test_parse_reader_propagates_io_error() {
        let error = std::io::Error::other("read failed");
        struct BrokenReader(io::Error);
        impl Read for BrokenReader {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(self.0.kind(), self.0.to_string()))
            }
        }
        let result = parse_reader(
            b"a,b\n".as_slice().chain(BrokenReader(error)),
            Delimiter::Comma,
        )
        .unwrap_err();
        assert_eq!(result.line, Some(2));
        assert!(result.message.contains("read failed"));
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

    #[test]
    fn test_escape_csv_value_simple() {
        assert_eq!(escape_csv_value("hello", Delimiter::Comma), "hello");
        assert_eq!(escape_csv_value("123", Delimiter::Comma), "123");
    }

    #[test]
    fn test_escape_csv_value_with_comma() {
        assert_eq!(
            escape_csv_value("hello, world", Delimiter::Comma),
            "\"hello, world\""
        );
    }

    #[test]
    fn test_escape_csv_value_with_quotes() {
        assert_eq!(
            escape_csv_value("say \"hello\"", Delimiter::Comma),
            "\"say \"\"hello\"\"\""
        );
    }

    #[test]
    fn test_escape_csv_value_with_newline() {
        assert_eq!(
            escape_csv_value("line1\nline2", Delimiter::Comma),
            "\"line1\nline2\""
        );
    }

    #[test]
    fn test_escape_csv_value_tab_delimiter() {
        assert_eq!(escape_csv_value("a,b", Delimiter::Tab), "a,b");
        assert_eq!(escape_csv_value("a\tb", Delimiter::Tab), "\"a\tb\"");
    }
}
