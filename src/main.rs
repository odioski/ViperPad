use std::{
    collections::HashMap,
    env, fs,
    io::{Cursor, Read},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path as FsPath, PathBuf},
    sync::{
        Arc, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};

use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::{get, put},
};
use axum_server::{Handle, tls_rustls::RustlsConfig};
use pulldown_cmark::{Options, Parser, html};
use serde::Serialize;

// Embedded static assets
const APP_HTML: &str = include_str!("app.html");
const EDITOR_JS: &str = include_str!("editor.bundle.js");
const SPLASH_PNG: &[u8] = include_bytes!("../assets/viperpad-splash.png");

// In-memory file store with atomic ID counter
#[derive(Clone)]
struct AppState {
    files: Arc<RwLock<HashMap<u64, FileEntry>>>,
    next_id: Arc<AtomicU64>,
}

// Stored file with optional base URL for remote HTML asset resolution
#[derive(Clone)]
struct FileEntry {
    name: String,
    kind: FileKind,
    body: String,
    rendered_html: Option<String>,
    original_bytes: Option<Vec<u8>>,
    base_url: Option<String>,
}

// File type determines preview/editor rendering strategy
#[derive(Clone, Copy)]
enum FileKind {
    Markdown,
    Html,
    Text,
    Document,
    Pdf,
    Code,
}

impl FileKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::Html => "html",
            Self::Text => "text",
            Self::Document => "document",
            Self::Pdf => "pdf",
            Self::Code => "code",
        }
    }
}

// JSON response for editor and raw file endpoints
#[derive(Serialize)]
struct RawFile {
    name: String,
    kind: &'static str,
    body: String,
}

// Start HTTPS server on localhost:4173 with graceful shutdown on Ctrl+C
#[tokio::main]
async fn main() {
    let state = AppState {
        files: Arc::new(RwLock::new(HashMap::new())),
        next_id: Arc::new(AtomicU64::new(1)),
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/view/{id}", get(index))
        .route("/assets/editor.js", get(editor_asset))
        .route("/assets/splash.png", get(splash_asset))
        .route("/api/files", put(create_file))
        .route("/api/files/{id}", get(get_file).put(update_file))
        .route("/api/files/{id}/binary", get(get_binary_file))
        .route("/api/files/{id}/raw", get(get_raw_file))
        .route("/api/remote", put(create_remote))
        .with_state(state);

    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 4173);
    let (cert_path, key_path) = tls_paths().unwrap_or_else(|error| panic!("{error}"));
    let tls_config = RustlsConfig::from_pem_file(&cert_path, &key_path)
        .await
        .unwrap_or_else(|error| {
            panic!(
                "could not load TLS cert/key from {} and {}: {error}",
                cert_path.display(),
                key_path.display()
            )
        });

    let display_host = env::var("VIPERPAD_HOST").unwrap_or_else(|_| "viperpad".to_owned());
    let url = format!("https://{}:{}", display_host, address.port());
    println!("ViperPad is running at {url}");

    let server_handle = Handle::new();
    tokio::spawn(handle_shutdown(server_handle.clone()));

    axum_server::bind_rustls(address, tls_config)
        .handle(server_handle)
        .serve(app.into_make_service())
        .await
        .expect("server failed");
}

fn tls_paths() -> Result<(PathBuf, PathBuf), String> {
    if let (Ok(cert), Ok(key)) = (env::var("VIPERPAD_TLS_CERT"), env::var("VIPERPAD_TLS_KEY")) {
        return Ok((PathBuf::from(cert), PathBuf::from(key)));
    }

    let cert_dir = env::var("VIPERPAD_TLS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("certs"));
    let entries = fs::read_dir(&cert_dir).map_err(|error| {
        format!(
            "could not read TLS cert directory {}: {error}",
            cert_dir.display()
        )
    })?;

    for entry in entries.flatten() {
        let cert_path = entry.path();
        let Some(file_name) = cert_path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !file_name.ends_with(".pem") || file_name.ends_with("-key.pem") {
            continue;
        }

        let key_name = file_name.trim_end_matches(".pem").to_owned() + "-key.pem";
        let key_path = cert_dir.join(key_name);
        if key_path.is_file() {
            return Ok((cert_path, key_path));
        }
    }

    Err(format!(
        "no mkcert cert/key pair found in {}; expected files like viperpad+3.pem and viperpad+3-key.pem",
        cert_dir.display()
    ))
}

// Wait for Ctrl+C signal
async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

async fn handle_shutdown(handle: Handle) {
    shutdown_signal().await;
    handle.graceful_shutdown(None);
}

// Serve app UI and viewer routes
async fn index() -> Html<&'static str> {
    Html(APP_HTML)
}

// Serve CodeMirror bundle without long-lived caching so embedded UI updates are visible
async fn editor_asset() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        EDITOR_JS,
    )
}

// Serve splash image with long cache headers
async fn splash_asset() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        SPLASH_PNG,
    )
}

// Create new file in memory; return assigned ID
async fn create_file(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, ApiError> {
    let id = state.next_id.fetch_add(1, Ordering::Relaxed);
    let entry = entry_from_request(&headers, body)?;
    state
        .files
        .write()
        .expect("file store poisoned")
        .insert(id, entry);
    Ok((StatusCode::CREATED, id.to_string()))
}

// Update existing file; fail if not found
async fn update_file(
    State(state): State<AppState>,
    Path(id): Path<u64>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, ApiError> {
    let entry = entry_from_request(&headers, body)?;
    let mut files = state.files.write().expect("file store poisoned");
    if !files.contains_key(&id) {
        return Err(ApiError::not_found());
    }
    files.insert(id, entry);
    Ok(StatusCode::NO_CONTENT)
}

// Download and store remote file; validate size (max 10 MB) and extract text when needed
async fn create_remote(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<impl IntoResponse, ApiError> {
    const MAX_SIZE: usize = 10 * 1024 * 1024;
    let requested_url = std::str::from_utf8(&body)
        .map_err(|_| ApiError(StatusCode::BAD_REQUEST, "URL is not valid UTF-8"))?
        .trim();
    let url = reqwest::Url::parse(requested_url)
        .map_err(|_| ApiError(StatusCode::BAD_REQUEST, "Enter a valid HTTP or HTTPS URL"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Only HTTP and HTTPS URLs are supported",
        ));
    }

    let mut response = reqwest::get(url).await.map_err(|_| {
        ApiError(
            StatusCode::BAD_GATEWAY,
            "Remote file could not be downloaded",
        )
    })?;
    if !response.status().is_success() {
        return Err(ApiError(
            StatusCode::BAD_GATEWAY,
            "Remote server returned an error",
        ));
    }
    if response
        .content_length()
        .is_some_and(|size| size > MAX_SIZE as u64)
    {
        return Err(ApiError(
            StatusCode::PAYLOAD_TOO_LARGE,
            "Remote file exceeds 10 MB",
        ));
    }

    let final_url = response.url().clone();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ApiError(StatusCode::BAD_GATEWAY, "Remote download was interrupted"))?
    {
        if bytes.len() + chunk.len() > MAX_SIZE {
            return Err(ApiError(
                StatusCode::PAYLOAD_TOO_LARGE,
                "Remote file exceeds 10 MB",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }

    let name = final_url
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .filter(|value| !value.is_empty())
        .unwrap_or("remote.txt")
        .to_owned();
    let kind = infer_kind(&name, &content_type);
    let entry = entry_from_bytes(name, kind, bytes.into(), Some(final_url.to_string()))?;
    let id = state.next_id.fetch_add(1, Ordering::Relaxed);
    state
        .files
        .write()
        .expect("file store poisoned")
        .insert(id, entry);
    Ok((StatusCode::CREATED, id.to_string()))
}

// Fetch raw file for editor; return JSON with name, kind, body
async fn get_raw_file(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<Json<RawFile>, ApiError> {
    let entry = state
        .files
        .read()
        .expect("file store poisoned")
        .get(&id)
        .cloned()
        .ok_or_else(ApiError::not_found)?;
    Ok(Json(RawFile {
        name: entry.name,
        kind: entry.kind.as_str(),
        body: entry.body,
    }))
}

// Render file for preview; render markdown/HTML to HTML and text/code as escaped source
async fn get_file(State(state): State<AppState>, Path(id): Path<u64>) -> Response {
    let Some(entry) = state
        .files
        .read()
        .expect("file store poisoned")
        .get(&id)
        .cloned()
    else {
        return (
            StatusCode::NOT_FOUND,
            "This viewer URL is no longer available.",
        )
            .into_response();
    };

    let rendered = match entry.kind {
        FileKind::Markdown => render_markdown(&entry.body),
        FileKind::Html => entry.body,
        FileKind::Document => entry
            .rendered_html
            .unwrap_or_else(|| render_document(&entry.body)),
        FileKind::Pdf => render_pdf(id, &entry.body),
        FileKind::Text => format!("<pre>{}</pre>", escape_html(&entry.body)),
        FileKind::Code => format!("<pre>{}</pre>", escape_html(&entry.body)),
    };

    let document = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\">{}<title>{}</title><style>{}</style></head><body>{}</body></html>",
        entry
            .base_url
            .as_deref()
            .map(|url| format!("<base href=\"{}\">", escape_html(url)))
            .unwrap_or_default(),
        escape_html(&entry.name),
        DOCUMENT_CSS,
        rendered
    );

    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        document,
    )
        .into_response()
}

async fn get_binary_file(State(state): State<AppState>, Path(id): Path<u64>) -> Response {
    let Some(entry) = state
        .files
        .read()
        .expect("file store poisoned")
        .get(&id)
        .cloned()
    else {
        return (StatusCode::NOT_FOUND, "Viewer URL not found").into_response();
    };

    let Some(bytes) = entry.original_bytes else {
        return (StatusCode::NOT_FOUND, "No binary preview is available").into_response();
    };
    let content_type = match entry.kind {
        FileKind::Pdf => "application/pdf",
        _ => "application/octet-stream",
    };

    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "no-store"),
        ],
        bytes,
    )
        .into_response()
}

// Parse file entry from upload request headers and body
fn entry_from_request(headers: &HeaderMap, body: Bytes) -> Result<FileEntry, ApiError> {
    const MAX_SIZE: usize = 10 * 1024 * 1024;
    if body.len() > MAX_SIZE {
        return Err(ApiError(
            StatusCode::PAYLOAD_TOO_LARGE,
            "File exceeds 10 MB",
        ));
    }

    let name = header_text(headers, "x-file-name").unwrap_or_else(|| "Untitled".into());
    let requested_kind = header_text(headers, "x-file-kind");
    let kind = requested_kind
        .as_deref()
        .and_then(parse_file_kind)
        .unwrap_or_else(|| infer_kind(&name, ""));

    entry_from_bytes(name, kind, body, None)
}

fn parse_file_kind(value: &str) -> Option<FileKind> {
    match value {
        "markdown" => Some(FileKind::Markdown),
        "html" => Some(FileKind::Html),
        "text" => Some(FileKind::Text),
        "document" => Some(FileKind::Document),
        "pdf" => Some(FileKind::Pdf),
        "code" => Some(FileKind::Code),
        _ => None,
    }
}

fn entry_from_bytes(
    name: String,
    kind: FileKind,
    bytes: Bytes,
    base_url: Option<String>,
) -> Result<FileEntry, ApiError> {
    let extension = file_extension(&name);
    let (body, rendered_html) = match kind {
        FileKind::Document => (
            document_text(&extension, &bytes)?,
            render_document_html(&extension, &bytes),
        ),
        FileKind::Pdf => (pdf_text(&bytes)?, None),
        _ => String::from_utf8(bytes.to_vec())
            .map(|body| (body, None))
            .map_err(|_| ApiError(StatusCode::BAD_REQUEST, "File is not valid UTF-8 text"))?,
    };
    let original_bytes = matches!(kind, FileKind::Pdf).then(|| bytes.to_vec());

    Ok(FileEntry {
        name,
        kind,
        body,
        rendered_html,
        original_bytes,
        base_url,
    })
}

// Infer file kind from extension and content type
fn infer_kind(name: &str, content_type: &str) -> FileKind {
    let extension = file_extension(name);
    match extension.as_str() {
        "md" | "markdown" => FileKind::Markdown,
        "html" | "htm" => FileKind::Html,
        "pdf" => FileKind::Pdf,
        "doc" | "docx" | "odt" | "rtf" => FileKind::Document,
        "txt" | "text" | "log" | "csv" | "tsv" => FileKind::Text,
        _ if content_type.contains("application/pdf") => FileKind::Pdf,
        _ if content_type.contains("officedocument") => FileKind::Document,
        _ if content_type.contains("msword") => FileKind::Document,
        _ if content_type.contains("rtf") => FileKind::Document,
        _ if content_type.contains("text/html") => FileKind::Html,
        _ if content_type.contains("markdown") => FileKind::Markdown,
        _ if content_type.starts_with("text/plain") => FileKind::Text,
        _ => FileKind::Code,
    }
}

fn file_extension(name: &str) -> String {
    FsPath::new(name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn document_text(extension: &str, bytes: &[u8]) -> Result<String, ApiError> {
    let text = match extension {
        "docx" => zipped_xml_text(bytes, "word/document.xml")?,
        "odt" => zipped_xml_text(bytes, "content.xml")?,
        "rtf" => rtf_to_text(bytes),
        "doc" => legacy_binary_text(bytes),
        _ => lossy_utf8_text(bytes),
    };
    let text = normalize_document_text(&text);
    if text.is_empty() {
        return Err(ApiError(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "Document did not contain extractable text",
        ));
    }
    Ok(text)
}

fn pdf_text(bytes: &[u8]) -> Result<String, ApiError> {
    let text = pdf_extract::extract_text_from_mem(bytes)
        .map_err(|_| ApiError(StatusCode::BAD_REQUEST, "PDF text could not be extracted"))?;
    let text = normalize_document_text(&text);
    if text.is_empty() {
        return Err(ApiError(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "PDF did not contain extractable text",
        ));
    }
    Ok(text)
}

fn zipped_xml_text(bytes: &[u8], member: &str) -> Result<String, ApiError> {
    let cursor = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|_| {
        ApiError(
            StatusCode::BAD_REQUEST,
            "Document archive could not be read",
        )
    })?;
    let mut file = archive
        .by_name(member)
        .map_err(|_| ApiError(StatusCode::BAD_REQUEST, "Document text could not be found"))?;
    let mut xml = String::new();
    file.read_to_string(&mut xml)
        .map_err(|_| ApiError(StatusCode::BAD_REQUEST, "Document XML is not valid UTF-8"))?;
    xml_to_text(&xml)
}

fn xml_to_text(xml: &str) -> Result<String, ApiError> {
    let document = roxmltree::Document::parse(xml)
        .map_err(|_| ApiError(StatusCode::BAD_REQUEST, "Document XML could not be parsed"))?;
    let mut output = String::new();
    for node in document.descendants() {
        if node.is_text() {
            output.push_str(node.text().unwrap_or_default());
            continue;
        }
        if !node.is_element() {
            continue;
        }
        match node.tag_name().name() {
            "tab" => output.push('\t'),
            "br" | "cr" => output.push('\n'),
            "p" => {
                if !output.ends_with('\n') {
                    output.push('\n');
                }
            }
            _ => {}
        }
    }
    Ok(output)
}

fn rtf_to_text(bytes: &[u8]) -> String {
    let source = lossy_utf8_text(bytes);
    let mut output = String::new();
    let mut chars = source.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '{' | '}' => {}
            '\\' => {
                let mut word = String::new();
                while let Some(next) = chars.peek().copied() {
                    if next.is_ascii_alphabetic() {
                        word.push(next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                while let Some(next) = chars.peek().copied() {
                    if next == '-' || next.is_ascii_digit() {
                        chars.next();
                    } else {
                        break;
                    }
                }
                if chars.peek().is_some_and(|next| *next == ' ') {
                    chars.next();
                }
                if matches!(word.as_str(), "par" | "line") {
                    output.push('\n');
                } else if word.is_empty() {
                    if let Some(symbol) = chars.next() {
                        output.push(symbol);
                    }
                }
            }
            '\r' => {}
            _ => output.push(ch),
        }
    }
    output
}

fn legacy_binary_text(bytes: &[u8]) -> String {
    let mut output = String::new();
    let mut run = Vec::new();
    for byte in bytes {
        if byte.is_ascii_graphic()
            || *byte == b' '
            || *byte == b'\n'
            || *byte == b'\r'
            || *byte == b'\t'
        {
            run.push(*byte);
        } else {
            if run.len() >= 4 {
                output.push_str(&String::from_utf8_lossy(&run));
                output.push('\n');
            }
            run.clear();
        }
    }
    if run.len() >= 4 {
        output.push_str(&String::from_utf8_lossy(&run));
    }
    output
}

fn lossy_utf8_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn normalize_document_text(input: &str) -> String {
    input
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}

fn render_document_html(extension: &str, bytes: &[u8]) -> Option<String> {
    libreoffice_convert_html(extension, bytes).or_else(|| {
        if extension == "docx" {
            docx_html(bytes)
        } else {
            None
        }
    })
}

fn libreoffice_convert_html(extension: &str, bytes: &[u8]) -> Option<String> {
    let options = libreoffice_convert_rust::ConvertOptions::new()
        .with_input_format(extension)
        .with_async_times(5)
        .with_async_interval(250);
    let future = libreoffice_convert_rust::convert_with_options(bytes, "html", None, options);
    let converted = if let Ok(handle) = tokio::runtime::Handle::try_current() {
        tokio::task::block_in_place(|| handle.block_on(future)).ok()?
    } else {
        tokio::runtime::Runtime::new().ok()?.block_on(future).ok()?
    };
    String::from_utf8(converted)
        .ok()
        .map(|html| extract_body_html(&html))
}

fn docx_html(bytes: &[u8]) -> Option<String> {
    let document = rdocx::Document::from_bytes(bytes).ok()?;
    Some(extract_body_html(&document.to_html()))
}

fn extract_body_html(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let Some(body_start) = lower.find("<body") else {
        return html.to_owned();
    };
    let after_body_tag = lower[body_start..]
        .find('>')
        .map(|index| body_start + index + 1)
        .unwrap_or(body_start);
    let body_end = lower[after_body_tag..]
        .find("</body>")
        .map(|index| after_body_tag + index)
        .unwrap_or(html.len());
    html[after_body_tag..body_end].trim().to_owned()
}

// Extract header value as string
fn header_text(headers: &HeaderMap, name: &'static str) -> Option<String> {
    headers
        .get(name)?
        .to_str()
        .ok()
        .map(|value| percent_decode_header(value).unwrap_or_else(|| value.to_owned()))
}

fn percent_decode_header(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    let mut changed = false;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = hex_value(*bytes.get(index + 1)?)?;
            let low = hex_value(*bytes.get(index + 2)?)?;
            decoded.push(high << 4 | low);
            index += 3;
            changed = true;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    if changed {
        String::from_utf8(decoded).ok()
    } else {
        Some(value.to_owned())
    }
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

// Render markdown to HTML with tables, footnotes, strikethrough, tasklists
fn render_markdown(source: &str) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;
    let parser = Parser::new_ext(source, options);
    let mut output = String::new();
    html::push_html(&mut output, parser);
    output
}

fn render_document(source: &str) -> String {
    let body = source
        .split("\n\n")
        .map(str::trim)
        .filter(|paragraph| !paragraph.is_empty())
        .map(|paragraph| format!("<p>{}</p>", escape_html(paragraph).replace('\n', "<br>")))
        .collect::<String>();
    if body.is_empty() {
        "<p></p>".to_owned()
    } else {
        body
    }
}

fn render_pdf(id: u64, extracted_text: &str) -> String {
    format!(
        "<div class=\"pdf-preview\"><object data=\"/api/files/{id}/binary\" type=\"application/pdf\"><div class=\"pdf-fallback\">{}</div></object></div>",
        render_document(extracted_text)
    )
}

// Escape HTML special characters to prevent XSS
fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// API error response with status code and message
#[derive(Debug)]
struct ApiError(StatusCode, &'static str);

impl ApiError {
    fn not_found() -> Self {
        Self(StatusCode::NOT_FOUND, "Viewer URL not found")
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, self.1).into_response()
    }
}

const DOCUMENT_CSS: &str = r#"
:root { color-scheme: light dark; font-family: system-ui, sans-serif; }
body { box-sizing: border-box; max-width: 900px; margin: 0 auto; padding: 40px 28px 80px; line-height: 1.65; }
img, video { max-width: 100%; }
.pdf-preview { position: fixed; inset: 0; background: Canvas; }
.pdf-preview object { width: 100%; height: 100%; border: 0; }
.pdf-fallback { box-sizing: border-box; max-width: 900px; margin: 0 auto; padding: 40px 28px 80px; }
pre { overflow: auto; padding: 16px; border-radius: 8px; background: color-mix(in srgb, CanvasText 8%, Canvas); white-space: pre-wrap; word-wrap: break-word; }
code { font-family: ui-monospace, SFMono-Regular, Consolas, monospace; }
blockquote { margin-left: 0; padding-left: 18px; border-left: 4px solid #7c6df2; color: color-mix(in srgb, CanvasText 72%, Canvas); }
table { border-collapse: collapse; width: 100%; }
th, td { border: 1px solid color-mix(in srgb, CanvasText 20%, Canvas); padding: 7px 10px; text-align: left; }
a { color: #6957e8; }
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_common_markdown() {
        let rendered = render_markdown("# Heading\n\n- [x] done\n\n~~old~~");
        assert!(rendered.contains("<h1>Heading</h1>"));
        assert!(rendered.contains("type=\"checkbox\""));
        assert!(rendered.contains("<del>old</del>"));
    }

    #[test]
    fn escapes_plain_text() {
        assert_eq!(escape_html("<script>\"&"), "&lt;script&gt;&quot;&amp;");
    }

    #[test]
    fn renders_document_paragraphs() {
        let rendered = render_document("First paragraph\ncontinued\n\n<script>");
        assert!(rendered.contains("<p>First paragraph<br>continued</p>"));
        assert!(rendered.contains("<p>&lt;script&gt;</p>"));
    }

    #[test]
    fn extracts_basic_rtf_text() {
        let text = document_text("rtf", br"{\rtf1 Hello\par World}").unwrap();
        assert_eq!(text, "Hello\nWorld");
    }

    #[test]
    fn decodes_percent_encoded_header_text() {
        assert_eq!(
            percent_decode_header("Meeting%20notes%20%E2%80%94%20draft.docx").as_deref(),
            Some("Meeting notes \u{2014} draft.docx")
        );
    }
}
