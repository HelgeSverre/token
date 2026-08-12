//! Content-Length framing for JSON-RPC over a language server's stdio,
//! per the LSP base protocol (`Content-Length: <n>\r\n\r\n<body>`).
//!
//! Framing is hand-written rather than pulled from a crate — it's ~80
//! lines and every alternative brings an async runtime this editor
//! doesn't have (see docs/feature/lsp-integration.md).

use std::io::{self, BufRead, Write};

use serde_json::Value;

/// Writes one JSON-RPC message with a Content-Length header, then flushes.
///
/// Flushing here (rather than leaving it to the caller) matters: the
/// worker thread writes requests interleaved with reads on the same
/// child process, and an unflushed write can wedge a server waiting on
/// stdin.
pub fn write_message<W: Write>(writer: &mut W, message: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(message)?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()
}

/// Reads one Content-Length-framed JSON-RPC message.
///
/// Takes `BufRead` (callers wrap the child's stdout in a `BufReader`)
/// so headers can be read line by line while leaving the body position
/// exactly at the start of the next frame — required since one `read`
/// off a pipe can return a partial header, a partial body, or several
/// whole messages back to back.
pub fn read_message<R: BufRead>(reader: &mut R) -> io::Result<Value> {
    let mut content_length: Option<usize> = None;
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "stream closed while reading headers",
            ));
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break; // blank line terminates the header block
        }
        let (name, value) = trimmed.split_once(':').ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("malformed header line: {trimmed:?}"),
            )
        })?;
        if name.trim().eq_ignore_ascii_case("content-length") {
            let value = value.trim();
            content_length = Some(value.parse().map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid Content-Length value: {value:?}"),
                )
            })?);
        }
        // Other headers (Content-Type, ...) are read and ignored.
    }

    let content_length = content_length.ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "missing Content-Length header")
    })?;

    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body)?;
    serde_json::from_slice(&body).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::{BufReader, Cursor, Read};

    fn frame(body: &str) -> Vec<u8> {
        let mut out = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
        out.extend_from_slice(body.as_bytes());
        out
    }

    #[test]
    fn round_trips_a_message() {
        let msg = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize"});
        let mut buf = Vec::new();
        write_message(&mut buf, &msg).unwrap();

        let mut reader = BufReader::new(Cursor::new(buf));
        let read_back = read_message(&mut reader).unwrap();
        assert_eq!(read_back, msg);
    }

    #[test]
    fn reads_multiple_messages_from_one_stream() {
        let a = json!({"id": 1});
        let b = json!({"id": 2});
        let mut bytes = frame(&a.to_string());
        bytes.extend(frame(&b.to_string()));

        let mut reader = BufReader::new(Cursor::new(bytes));
        assert_eq!(read_message(&mut reader).unwrap(), a);
        assert_eq!(read_message(&mut reader).unwrap(), b);
    }

    #[test]
    fn ignores_extra_headers_case_insensitively() {
        let body = json!({"ok": true}).to_string();
        let raw = format!(
            "content-type: application/vscode-jsonrpc; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let mut reader = BufReader::new(Cursor::new(raw.into_bytes()));
        assert_eq!(read_message(&mut reader).unwrap(), json!({"ok": true}));
    }

    #[test]
    fn errors_on_missing_content_length() {
        let raw = b"Content-Type: application/vscode-jsonrpc\r\n\r\n{}".to_vec();
        let mut reader = BufReader::new(Cursor::new(raw));
        let err = read_message(&mut reader).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn errors_on_malformed_header_line() {
        let raw = b"not-a-header-line\r\n\r\n".to_vec();
        let mut reader = BufReader::new(Cursor::new(raw));
        let err = read_message(&mut reader).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn errors_on_non_numeric_content_length() {
        let raw = b"Content-Length: not-a-number\r\n\r\n".to_vec();
        let mut reader = BufReader::new(Cursor::new(raw));
        let err = read_message(&mut reader).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn errors_on_truncated_stream() {
        // Header claims 100 bytes, stream only ever delivers 2.
        let raw = b"Content-Length: 100\r\n\r\n{}".to_vec();
        let mut reader = BufReader::new(Cursor::new(raw));
        let err = read_message(&mut reader).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn errors_on_invalid_json_body() {
        let raw = b"Content-Length: 9\r\n\r\nnot json!".to_vec();
        let mut reader = BufReader::new(Cursor::new(raw));
        let err = read_message(&mut reader).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn handles_a_huge_payload() {
        // Well past any single pipe-buffer read (~64 KB on Linux).
        let big_string = "x".repeat(2_000_000);
        let msg = json!({"payload": big_string});
        let mut buf = Vec::new();
        write_message(&mut buf, &msg).unwrap();

        let mut reader = BufReader::new(Cursor::new(buf));
        assert_eq!(read_message(&mut reader).unwrap(), msg);
    }

    /// A `Read` that trickles bytes out a handful at a time, standing in
    /// for a pipe where one `read` call rarely returns a whole frame.
    struct StutteringReader {
        data: Vec<u8>,
        pos: usize,
        chunk: usize,
    }

    impl Read for StutteringReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let remaining = self.data.len() - self.pos;
            let n = remaining.min(self.chunk).min(buf.len());
            buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
            self.pos += n;
            Ok(n)
        }
    }

    #[test]
    fn reassembles_a_message_delivered_in_small_partial_reads() {
        let msg =
            json!({"jsonrpc": "2.0", "method": "textDocument/didChange", "params": {"n": 42}});
        let mut buf = Vec::new();
        write_message(&mut buf, &msg).unwrap();

        let stuttering = StutteringReader {
            data: buf,
            pos: 0,
            chunk: 3, // deliberately smaller than any header or body chunk
        };
        let mut reader = BufReader::new(stuttering);
        assert_eq!(read_message(&mut reader).unwrap(), msg);
    }

    #[test]
    fn reassembles_two_messages_across_partial_reads() {
        let a = json!({"id": 1, "note": "first"});
        let b = json!({"id": 2, "note": "second"});
        let mut bytes = frame(&a.to_string());
        bytes.extend(frame(&b.to_string()));

        let stuttering = StutteringReader {
            data: bytes,
            pos: 0,
            chunk: 5,
        };
        let mut reader = BufReader::new(stuttering);
        assert_eq!(read_message(&mut reader).unwrap(), a);
        assert_eq!(read_message(&mut reader).unwrap(), b);
    }
}
