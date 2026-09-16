//! Preview URL routing and the bounded asynchronous resource worker.
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

use token::model::editor_area::PreviewId;
use token::preview_resources::{ResourceError, ResourceScope, ScopedFile};
use url::Url;

pub type Response = wry::http::Response<Cow<'static, [u8]>>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PreviewLocation {
    pub source: Option<PathBuf>,
    pub resolved_source: Option<PathBuf>,
    pub workspace: Option<PathBuf>,
}

impl PreviewLocation {
    pub fn from_document(
        document: &token::model::Document,
        workspace: Option<&token::model::Workspace>,
    ) -> Self {
        Self {
            source: document.file_path.clone(),
            resolved_source: document
                .file_identity()
                .map(|identity| identity.path().to_path_buf()),
            workspace: workspace.map(|workspace| workspace.root.clone()),
        }
    }
}

#[derive(Clone)]
pub struct PreviewContent {
    pub html: Arc<str>,
    pub location: PreviewLocation,
}

pub struct PreviewDocument {
    pub location: PreviewLocation,
    pub url: String,
    host: String,
    entry: PathBuf,
    html: Arc<str>,
    scope: Option<Arc<ResourceScope>>,
    active: AtomicBool,
}

#[derive(Debug)]
pub enum Navigation {
    Document,
    Local(ScopedFile),
    External(String),
}

impl PreviewDocument {
    pub fn new(
        id: PreviewId,
        generation: u64,
        content: PreviewContent,
    ) -> Result<Arc<Self>, ResourceError> {
        let (scope, entry) = if let Some(source) = &content.location.source {
            let source = std::path::absolute(source)?;
            // Keep the opened directory spelling when possible. Cached identity
            // handles parent components and workspace aliases without UI-thread I/O.
            let resolved = content.location.resolved_source.as_ref();
            let use_resolved = source
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
                || content.location.workspace.as_ref().is_some_and(|root| {
                    !source.starts_with(root) && resolved.is_some_and(|path| path.starts_with(root))
                });
            let source = if use_resolved {
                resolved.unwrap_or(&source)
            } else {
                &source
            };
            let parent = source.parent().ok_or(ResourceError::Malformed)?;
            let root = content
                .location
                .workspace
                .as_ref()
                .filter(|root| source.starts_with(root))
                .map_or(parent, PathBuf::as_path);
            let relative = source
                .strip_prefix(root)
                .map_err(|_| ResourceError::Forbidden)?
                .to_path_buf();
            (Some(ResourceScope::new(root.to_path_buf())), relative)
        } else {
            (None, PathBuf::from("index.html"))
        };
        let host = format!("preview-{}-{generation}", id.0);
        let mut url =
            Url::parse(&format!("token://{host}/")).map_err(|_| ResourceError::Malformed)?;
        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| ResourceError::Malformed)?;
            segments.clear();
            for component in entry.components() {
                let segment = component
                    .as_os_str()
                    .to_str()
                    .ok_or(ResourceError::Malformed)?;
                validate_segment(segment)?;
                segments.push(segment);
            }
        }
        Ok(Arc::new(Self {
            location: content.location,
            url: url.into(),
            host,
            entry,
            html: content.html,
            scope,
            active: AtomicBool::new(true),
        }))
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }
    pub fn revoke(&self) {
        self.active.store(false, Ordering::Release);
        if let Some(scope) = &self.scope {
            scope.revoke();
        }
    }

    pub fn navigation(&self, value: &str) -> Result<Navigation, ResourceError> {
        if !self.is_active() {
            return Err(ResourceError::Stale);
        }
        if value.chars().any(|c| c.is_control() || c == '\\') {
            return Err(ResourceError::Malformed);
        }
        let url = Url::parse(value).map_err(|_| ResourceError::Malformed)?;
        let host = url.host_str().ok_or(ResourceError::Forbidden)?;
        let alias = format!("token.{}", self.host);
        let internal = (url.scheme() == "token" && host == self.host)
            || (url.scheme() == "http" && host == alias);
        if !internal {
            // Other preview generations/origins are never sent to a browser.
            if host.starts_with("token.preview-") || url.scheme() == "token" {
                return Err(ResourceError::Forbidden);
            }
            return if token::util::is_web_url(value) {
                Ok(Navigation::External(value.to_owned()))
            } else {
                Err(ResourceError::Forbidden)
            };
        }
        // Compare the original authority too: Url normalizes explicit default ports.
        let authority = value
            .split_once("://")
            .ok_or(ResourceError::Malformed)?
            .1
            .split(['/', '?', '#'])
            .next()
            .ok_or(ResourceError::Malformed)?;
        if authority != self.host && authority != alias {
            return Err(ResourceError::Forbidden);
        }
        let path = decode_path(url.path())?;
        if path == self.entry {
            return Ok(Navigation::Document);
        }
        let scope = self.scope.as_ref().ok_or(ResourceError::Missing)?;
        Ok(Navigation::Local(scope.file(path)?))
    }

    pub fn respond(&self, request: wry::http::Request<Vec<u8>>) -> Response {
        let head = request.method() == wry::http::Method::HEAD;
        if !head && request.method() != wry::http::Method::GET {
            return response(405, "text/plain", b"Method not allowed".to_vec(), false);
        }
        let result = (|| match self.navigation(&request.uri().to_string())? {
            Navigation::Document => Ok((self.html.as_bytes().to_vec(), "text/html; charset=utf-8")),
            Navigation::Local(file) => Ok((file.read()?.bytes, mime_type(file.path()))),
            Navigation::External(_) => Err(ResourceError::Forbidden),
        })();
        if !self.is_active() {
            return response(
                410,
                "text/plain; charset=utf-8",
                ResourceError::Stale.to_string().into_bytes(),
                head,
            );
        }
        match result {
            Ok((bytes, mime)) => response(200, mime, bytes, head),
            Err(error) => response(
                error.status(),
                "text/plain; charset=utf-8",
                error.to_string().into_bytes(),
                head,
            ),
        }
    }
}

fn decode_path(path: &str) -> Result<PathBuf, ResourceError> {
    let mut output = PathBuf::new();
    for encoded in path
        .strip_prefix('/')
        .ok_or(ResourceError::Malformed)?
        .split('/')
    {
        if encoded.is_empty() {
            return Err(ResourceError::Malformed);
        }
        let bytes = encoded.as_bytes();
        for (index, byte) in bytes.iter().enumerate() {
            if *byte == b'%'
                && (index + 2 >= bytes.len()
                    || !bytes[index + 1].is_ascii_hexdigit()
                    || !bytes[index + 2].is_ascii_hexdigit())
            {
                return Err(ResourceError::Malformed);
            }
        }
        let decoded = percent_encoding::percent_decode_str(encoded)
            .decode_utf8()
            .map_err(|_| ResourceError::Malformed)?;
        validate_segment(&decoded)?;
        output.push(decoded.as_ref());
    }
    Ok(output)
}

fn validate_segment(segment: &str) -> Result<(), ResourceError> {
    if segment.is_empty()
        || segment == "."
        || segment == ".."
        || segment
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
    {
        return Err(ResourceError::Forbidden);
    }
    #[cfg(windows)]
    validate_windows_segment(segment)?;
    Ok(())
}

#[cfg(any(windows, test))]
fn validate_windows_segment(segment: &str) -> Result<(), ResourceError> {
    let stem = segment
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end()
        .to_ascii_uppercase();
    let device_number = stem
        .strip_prefix("COM")
        .or_else(|| stem.strip_prefix("LPT"));
    if segment.ends_with(['.', ' '])
        || matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        )
        || matches!(
            device_number,
            Some("1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³")
        )
    {
        return Err(ResourceError::Forbidden);
    }
    Ok(())
}

fn mime_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "eot" => "application/vnd.ms-fontobject",
        "wasm" => "application/wasm",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "ogv" => "video/ogg",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" | "oga" => "audio/ogg",
        "pdf" => "application/pdf",
        "xml" => "application/xml",
        "txt" => "text/plain; charset=utf-8",
        "md" => "text/markdown; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn response(status: u16, mime: &'static str, bytes: Vec<u8>, head: bool) -> Response {
    wry::http::Response::builder()
        .status(status)
        .header("Content-Type", mime)
        .header("X-Content-Type-Options", "nosniff")
        .header("Cache-Control", "no-store")
        .header("Content-Length", bytes.len())
        .body(Cow::Owned(if head { Vec::new() } else { bytes }))
        .expect("static preview response headers are valid")
}
pub fn error_response(error: ResourceError) -> Response {
    response(
        error.status(),
        "text/plain; charset=utf-8",
        error.to_string().into_bytes(),
        false,
    )
}

pub struct ResourceJob {
    pub document: Arc<PreviewDocument>,
    pub request: wry::http::Request<Vec<u8>>,
    pub responder: wry::RequestAsyncResponder,
}

pub fn start_worker() -> mpsc::SyncSender<ResourceJob> {
    let (tx, rx) = mpsc::sync_channel::<ResourceJob>(32);
    if let Err(error) = std::thread::Builder::new()
        .name("preview-resources".into())
        .spawn(move || {
            while let Ok(job) = rx.recv() {
                job.responder.respond(job.document.respond(job.request));
            }
        })
    {
        tracing::error!(%error, "Could not start preview resource worker");
    }
    tx
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn document(root: &Path, workspace: bool) -> Arc<PreviewDocument> {
        PreviewDocument::new(
            PreviewId(7),
            3,
            PreviewContent {
                html: "generated page".into(),
                location: PreviewLocation {
                    source: Some(root.join("docs/ui/COMBOBOX.md")),
                    resolved_source: None,
                    workspace: workspace.then(|| root.into()),
                },
            },
        )
        .unwrap()
    }

    fn get(document: &PreviewDocument, reference: &str) -> Response {
        let url = Url::parse(&document.url).unwrap().join(reference).unwrap();
        document.respond(
            wry::http::Request::builder()
                .uri(url.as_str())
                .body(Vec::new())
                .unwrap(),
        )
    }

    #[test]
    fn browser_resolution_retains_document_directory_and_css_base() {
        let root = tempfile::tempdir().unwrap();
        let fixtures = [
            (
                "docs/ui/mockups/renders/COMBOBOX.png",
                "mockups/renders/COMBOBOX.png",
            ),
            ("docs/ui/sibling.PNG", "sibling.PNG"),
            ("docs/parent.svg", "../parent.svg?version=1#icon"),
            ("assets/root.png", "/assets/root.png"),
            ("docs/ui/space å%#?.png", "space%20%C3%A5%25%23%3F.png"),
            ("docs/ui/image..png", "image..png"),
            ("docs/ui/index.html", "index.html"),
            ("docs/ui/css/style.css", "css/style.css"),
            ("docs/ui/fonts/font.WOFF2", "fonts/font.WOFF2"),
        ];
        for (path, _) in fixtures {
            let path = root.path().join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"resource").unwrap();
        }
        let doc = document(root.path(), true);
        assert!(doc.url.ends_with("/docs/ui/COMBOBOX.md"));
        assert_eq!(get(&doc, "#heading").body().as_ref(), b"generated page");
        for (_, reference) in fixtures {
            let response = get(&doc, reference);
            assert_eq!(response.status(), 200, "{reference}");
            assert_eq!(response.body().as_ref(), b"resource", "{reference}");
            assert_eq!(response.headers()["Cache-Control"], "no-store");
            assert_eq!(response.headers()["X-Content-Type-Options"], "nosniff");
            assert!(!response
                .headers()
                .contains_key("Access-Control-Allow-Origin"));
        }
        assert_eq!(
            get(&doc, "sibling.PNG").headers()["Content-Type"],
            "image/png"
        );
        let css = Url::parse(&doc.url).unwrap().join("css/style.css").unwrap();
        for reference in ["../sibling.PNG", "../fonts/font.WOFF2"] {
            let target = css.join(reference).unwrap();
            assert_eq!(get(&doc, target.as_str()).status(), 200);
        }
    }

    #[test]
    fn origins_and_navigation_are_identical_for_native_and_windows_urls() {
        let root = tempfile::tempdir().unwrap();
        let doc = document(root.path(), true);
        for origin in ["token://preview-7-3", "http://token.preview-7-3"] {
            for suffix in ["", "#heading", "?reload=yes#heading"] {
                assert!(matches!(
                    doc.navigation(&format!("{origin}/docs/ui/COMBOBOX.md{suffix}")),
                    Ok(Navigation::Document)
                ));
            }
            assert!(matches!(
                doc.navigation(&format!("{origin}/docs/other.md#heading")),
                Ok(Navigation::Local(_))
            ));
            for path in [
                "/%2fetc",
                "/%5Cfile",
                "/%00",
                "/%0a",
                "/C%3A/file",
                "/%FF",
                "/bad%",
                "/bad%2",
                "/bad%XZ",
                "/\\server\\file",
            ] {
                assert!(
                    doc.navigation(&format!("{origin}{path}")).is_err(),
                    "{origin}{path}"
                );
            }
        }
        for url in [
            "token://preview-7-2/a",
            "token://preview-8-3/a",
            "http://token.preview-7-2/a",
            "http://token.preview-7-3:80/a",
            "token://preview-7-3:9/a",
            "token://user@preview-7-3/a",
            "https://token.preview-7-3/a",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "data:text/plain,a",
        ] {
            assert!(doc.navigation(url).is_err(), "{url}");
        }
        for url in ["https://example.org/docs", "http://example.org"] {
            assert!(matches!(doc.navigation(url), Ok(Navigation::External(_))));
        }
    }

    #[test]
    fn windows_device_names_are_rejected_including_extensions_and_legacy_digits() {
        for name in [
            "CON",
            "con.txt",
            "CON .txt",
            "NUL.png",
            "COM1",
            "lpt9.svg",
            "COM¹.txt",
            "LPT²",
            "CONIN$",
            "CONOUT$",
            "trailing.",
            "trailing ",
        ] {
            assert_eq!(
                validate_windows_segment(name),
                Err(ResourceError::Forbidden),
                "{name}"
            );
        }
        for name in [
            "image..png",
            "console.txt",
            "COM10.txt",
            "lpt0.png",
            "normal.PNG",
        ] {
            assert!(validate_windows_segment(name).is_ok(), "{name}");
        }
        assert_eq!(decode_path("/%252f.png").unwrap(), PathBuf::from("%2f.png"));
    }

    #[test]
    fn standalone_and_untitled_documents_have_narrow_authority() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("docs/ui")).unwrap();
        std::fs::write(root.path().join("docs/ui/asset"), b"local").unwrap();
        std::fs::write(root.path().join("outside"), b"outside").unwrap();
        let doc = document(root.path(), false);
        assert!(doc.url.ends_with("/COMBOBOX.md"));
        assert_eq!(get(&doc, "/asset").body().as_ref(), b"local");
        assert_eq!(get(&doc, "../../outside").status(), 404);
        let untitled = PreviewDocument::new(
            PreviewId(1),
            1,
            PreviewContent {
                html: "untitled".into(),
                location: PreviewLocation::default(),
            },
        )
        .unwrap();
        assert_eq!(get(&untitled, "asset").status(), 404);
        assert_eq!(get(&untitled, "#heading").body().as_ref(), b"untitled");
    }

    #[test]
    fn responses_report_missing_method_and_revocation_and_head_has_no_body() {
        let root = tempfile::tempdir().unwrap();
        let doc = document(root.path(), true);
        assert_eq!(get(&doc, "missing.png").status(), 404);
        let response = doc.respond(
            wry::http::Request::builder()
                .uri(&doc.url)
                .method("POST")
                .body(vec![])
                .unwrap(),
        );
        assert_eq!(response.status(), 405);
        for target in [&doc.url, &format!("{}/missing", doc.url)] {
            let response = doc.respond(
                wry::http::Request::builder()
                    .uri(target)
                    .method("HEAD")
                    .body(vec![])
                    .unwrap(),
            );
            assert!(response.body().is_empty());
        }
        doc.revoke();
        assert_eq!(get(&doc, "#heading").status(), 410);
        assert!(matches!(
            doc.navigation(&doc.url),
            Err(ResourceError::Stale)
        ));
    }

    proptest! {
        #[test]
        fn escaped_segments_round_trip_exactly_once(parts in prop::collection::vec("[a-zA-Z0-9éø %?#_.-]{1,40}", 1..8)) {
            prop_assume!(parts.iter().all(|part| validate_segment(part).is_ok()));
            let encoded = parts.iter().map(|part| percent_encoding::utf8_percent_encode(part, percent_encoding::NON_ALPHANUMERIC).to_string()).collect::<Vec<_>>().join("/");
            let decoded = decode_path(&format!("/{encoded}")).unwrap();
            let expected: PathBuf = parts.iter().collect();
            prop_assert_eq!(decoded, expected);
        }

        #[test]
        fn accepted_paths_contain_only_normal_components(input in ".{0,300}") {
            if let Ok(path) = decode_path(&input) {
                prop_assert!(!path.is_absolute());
                prop_assert!(path.components().all(|component| matches!(component, std::path::Component::Normal(_))));
                for segment in path.iter() { prop_assert!(validate_segment(segment.to_str().unwrap()).is_ok()); }
            }
        }
    }
}
