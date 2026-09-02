//! llama.cpp `/infill` transport — autocomplete.md Phase 2.
//!
//! One blocking HTTP/1.1 POST over `std::net`, no TLS, no client crate:
//! the target is a local llama-server. Other transports (Ollama,
//! OpenAI-compatible, Mistral) are Phase 3 and would earn a dependency.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde::Deserialize;

use super::inline::InlineRequest;

/// llama.cpp's `/infill` reply; every other field is ignored.
#[derive(Deserialize)]
struct InfillResponse {
    #[serde(default)]
    content: String,
}

/// Ask the server for the text between `prefix` and `suffix`. Returns the
/// raw model output; callers post-process it.
pub fn infill(request: &InlineRequest) -> Result<String, String> {
    let (host, port, base_path) = parse_url(&request.endpoint.url)?;
    let body = serde_json::json!({
        "input_prefix": request.prefix,
        "input_suffix": request.suffix,
        "n_predict": request.endpoint.max_tokens,
        "temperature": 0.1,
        "cache_prompt": true,
        "stream": false,
        "t_max_predict_ms": request.endpoint.timeout_ms,
    })
    .to_string();
    let path = format!("{}/infill", base_path.trim_end_matches('/'));
    let reply = post_json(
        &host,
        port,
        &path,
        &body,
        Duration::from_millis(request.endpoint.timeout_ms),
    )?;
    let parsed: InfillResponse =
        serde_json::from_str(&reply).map_err(|error| format!("invalid /infill reply: {error}"))?;
    Ok(parsed.content)
}

/// `http://host[:port][/path]` → (host, port, path). Only plain HTTP.
fn parse_url(url: &str) -> Result<(String, u16, String), String> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| format!("unsupported URL `{url}` (only http:// is supported)"))?;
    let (authority, path) = match rest.find('/') {
        Some(index) => (&rest[..index], &rest[index..]),
        None => (rest, ""),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (
            host,
            port.parse::<u16>()
                .map_err(|_| format!("invalid port in `{url}`"))?,
        ),
        None => (authority, 80),
    };
    if host.is_empty() {
        return Err(format!("missing host in `{url}`"));
    }
    Ok((host.to_owned(), port, path.to_owned()))
}

/// Minimal HTTP/1.1 POST: returns the response body, or the status line
/// as the error for anything but 200.
fn post_json(
    host: &str,
    port: u16,
    path: &str,
    body: &str,
    timeout: Duration,
) -> Result<String, String> {
    let address = (host, port);
    let addr = std::net::ToSocketAddrs::to_socket_addrs(&address)
        .map_err(|error| format!("cannot resolve {host}: {error}"))?
        .next()
        .ok_or_else(|| format!("cannot resolve {host}"))?;
    let mut stream = TcpStream::connect_timeout(&addr, timeout)
        .map_err(|error| format!("cannot connect to {host}:{port}: {error}"))?;
    let io_error = |error: std::io::Error| error.to_string();
    stream.set_read_timeout(Some(timeout)).map_err(io_error)?;
    stream.set_write_timeout(Some(timeout)).map_err(io_error)?;
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).map_err(io_error)?;

    let mut reader = BufReader::new(stream);
    let mut status = String::new();
    reader.read_line(&mut status).map_err(io_error)?;
    let ok = status.split_whitespace().nth(1) == Some("200");
    let mut content_length: Option<usize> = None;
    let mut chunked = false;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).map_err(io_error)?;
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            match name.trim().to_ascii_lowercase().as_str() {
                "content-length" => content_length = value.trim().parse().ok(),
                "transfer-encoding" if value.trim().eq_ignore_ascii_case("chunked") => {
                    chunked = true;
                }
                _ => {}
            }
        }
    }
    let body = if chunked {
        read_chunked(&mut reader).map_err(io_error)?
    } else if let Some(length) = content_length {
        let mut bytes = vec![0; length];
        reader.read_exact(&mut bytes).map_err(io_error)?;
        String::from_utf8_lossy(&bytes).into_owned()
    } else {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).map_err(io_error)?;
        String::from_utf8_lossy(&bytes).into_owned()
    };
    if ok {
        Ok(body)
    } else {
        Err(format!(
            "server answered {}",
            status
                .trim()
                .strip_prefix("HTTP/1.1 ")
                .unwrap_or(status.trim())
        ))
    }
}

fn read_chunked(reader: &mut impl BufRead) -> std::io::Result<String> {
    let mut out = Vec::new();
    loop {
        let mut size_line = String::new();
        reader.read_line(&mut size_line)?;
        let size = usize::from_str_radix(size_line.trim().split(';').next().unwrap_or("0"), 16)
            .unwrap_or(0);
        if size == 0 {
            break;
        }
        let mut chunk = vec![0; size];
        reader.read_exact(&mut chunk)?;
        out.extend_from_slice(&chunk);
        let mut crlf = String::new();
        reader.read_line(&mut crlf)?;
    }
    Ok(String::from_utf8_lossy(&out).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::inline::{InlineEndpoint, RequestSnapshot};
    use crate::model::editor_area::DocumentId;
    use std::net::TcpListener;

    /// A one-shot fake llama-server: records the request body and answers
    /// with `response` (a full HTTP response, so tests can pick the status).
    fn fake_server(response: &'static str) -> (String, std::sync::mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request = String::new();
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                request.push_str(&line);
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap();
                }
                if line == "\r\n" {
                    break;
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            request.push_str(&String::from_utf8_lossy(&body));
            tx.send(request).unwrap();
            stream.write_all(response.as_bytes()).unwrap();
        });
        (url, rx)
    }

    fn request(url: &str) -> InlineRequest {
        InlineRequest {
            snapshot: RequestSnapshot {
                document_id: DocumentId(1),
                revision: 1,
                line: 0,
                column: 8,
                request_id: 1,
            },
            prefix: "let a = ".into(),
            suffix: "\n".into(),
            language: Some("rust".into()),
            file_path: None,
            endpoint: InlineEndpoint {
                url: url.to_owned(),
                max_tokens: 32,
                timeout_ms: 2000,
            },
            explicit: false,
        }
    }

    #[test]
    fn parses_urls_with_and_without_port_and_path() {
        assert_eq!(
            parse_url("http://127.0.0.1:8012").unwrap(),
            ("127.0.0.1".into(), 8012, String::new())
        );
        assert_eq!(
            parse_url("http://localhost/llama").unwrap(),
            ("localhost".into(), 80, "/llama".into())
        );
        assert!(parse_url("https://x").is_err());
        assert!(parse_url("http://:80").is_err());
    }

    #[test]
    fn posts_prefix_and_suffix_and_returns_the_content() {
        let (url, rx) = fake_server(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 20\r\n\r\n{\"content\":\"1 + 2;\"}",
        );
        let content = infill(&request(&url)).unwrap();
        assert_eq!(content, "1 + 2;");
        let seen = rx.recv().unwrap();
        assert!(seen.starts_with("POST /infill HTTP/1.1\r\n"), "{seen}");
        let body: serde_json::Value =
            serde_json::from_str(seen.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(body["input_prefix"], "let a = ");
        assert_eq!(body["input_suffix"], "\n");
        assert_eq!(body["n_predict"], 32);
    }

    #[test]
    fn reads_chunked_replies() {
        let (url, _rx) = fake_server(
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nb\r\n{\"content\":\r\n9\r\n\"chunk\"}\r\n\r\n0\r\n\r\n",
        );
        assert_eq!(infill(&request(&url)).unwrap(), "chunk");
    }

    #[test]
    fn non_200_and_unreachable_servers_are_errors() {
        let (url, _rx) =
            fake_server("HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n");
        let error = infill(&request(&url)).unwrap_err();
        assert!(error.contains("503"), "{error}");
        let closed = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", closed.local_addr().unwrap());
        drop(closed);
        let mut req = request(&url);
        req.endpoint.timeout_ms = 300;
        assert!(infill(&req).unwrap_err().contains("cannot connect"));
    }
}
