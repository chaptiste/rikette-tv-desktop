//! `efmedia`: serves local clips, thumbnails and audio previews to the page, with byte ranges
//! so `<video>` can seek. WebView2 exposes custom schemes as `http://<scheme>.localhost/`.

use std::io::SeekFrom;
use std::path::{Path, PathBuf};

use tauri::http::{header, Request, Response, StatusCode};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::{library, paths};

pub const SCHEME: &str = "efmedia";

/// Most bytes sent per response; the player asks for the next range as it plays.
const CHUNK: u64 = 4 * 1024 * 1024;
/// Thumbnail URLs carry the file's mtime, so a cached copy is never stale.
const THUMB_CACHE: &str = "public, max-age=31536000, immutable";

static SUCCESS_SOUND: &[u8] = include_bytes!("../../resources/success.mp3");

fn base() -> String {
    if cfg!(windows) {
        format!("http://{SCHEME}.localhost")
    } else {
        format!("{SCHEME}://localhost")
    }
}

pub fn video_url(id: &str, bust: i64) -> String {
    format!("{}/video/{}?t={bust}", base(), encode(id))
}

pub fn thumb_url(id: &str, bust: i64) -> String {
    format!("{}/thumb/{}?t={bust}", base(), encode(id))
}

pub fn audio_url(id: &str, index: usize, bust: i64) -> String {
    format!("{}/audio/{}/{index}?t={bust}", base(), encode(id))
}

pub fn sound_url() -> String {
    format!("{}/sfx/success", base())
}

fn encode(id: &str) -> String {
    percent_encoding::utf8_percent_encode(id, percent_encoding::NON_ALPHANUMERIC).to_string()
}

pub async fn handle(request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    match respond(&request).await {
        Ok(response) => response,
        Err(error) => {
            log::warn!("[rikette] media protocol {}: {error:#}", request.uri());
            status(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn respond(request: &Request<Vec<u8>>) -> anyhow::Result<Response<Vec<u8>>> {
    let parts: Vec<String> = request
        .uri()
        .path()
        .split('/')
        .filter(|part| !part.is_empty())
        .map(|part| percent_encoding::percent_decode_str(part).decode_utf8_lossy().into_owned())
        .collect();
    let kind = parts.first().map(String::as_str).unwrap_or("");
    let id = parts.get(1).map(String::as_str).unwrap_or("");

    if kind == "sfx" {
        return Ok(Response::builder()
            .header(header::CONTENT_TYPE, "audio/mpeg")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(SUCCESS_SOUND.to_vec())?);
    }
    if id.is_empty() {
        return Ok(status(StatusCode::NOT_FOUND));
    }
    let Some(clip) = library::get_clip(id)? else {
        return Ok(status(StatusCode::NOT_FOUND));
    };
    let (file, cache): (PathBuf, &str) = match kind {
        "video" => (PathBuf::from(&clip.path), "no-store"),
        "thumb" => match clip.thumb_path {
            Some(path) => (PathBuf::from(path), THUMB_CACHE),
            None => return Ok(status(StatusCode::NOT_FOUND)),
        },
        "audio" => match parts.get(2).and_then(|n| n.parse::<usize>().ok()) {
            Some(index) => (paths::audio_file(id, index), "no-store"),
            None => return Ok(status(StatusCode::NOT_FOUND)),
        },
        _ => return Ok(status(StatusCode::NOT_FOUND)),
    };
    file_response(&file, request, cache).await
}

async fn file_response(file: &Path, request: &Request<Vec<u8>>, cache: &str) -> anyhow::Result<Response<Vec<u8>>> {
    let Ok(meta) = tokio::fs::metadata(file).await else {
        return Ok(status(StatusCode::NOT_FOUND));
    };
    let size = meta.len();
    let mime = mime_of(file);
    let builder = Response::builder()
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_TYPE, mime)
        .header(header::CACHE_CONTROL, cache)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*");

    let range = request
        .headers()
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .map(|raw| parse_range(raw, size));
    let (start, end) = match range {
        None if size <= CHUNK => {
            let body = if request.method() == "HEAD" { Vec::new() } else { tokio::fs::read(file).await? };
            return Ok(builder.header(header::CONTENT_LENGTH, size).body(body)?);
        }
        None => (0, CHUNK - 1),
        Some(None) => {
            return Ok(builder
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header(header::CONTENT_RANGE, format!("bytes */{size}"))
                .body(Vec::new())?);
        }
        Some(Some(range)) => range,
    };
    let end = end.min(start + CHUNK - 1).min(size - 1);
    let length = end - start + 1;
    let mut body = Vec::new();
    if request.method() != "HEAD" {
        let mut handle = tokio::fs::File::open(file).await?;
        handle.seek(SeekFrom::Start(start)).await?;
        body.reserve(length as usize);
        handle.take(length).read_to_end(&mut body).await?;
    }
    Ok(builder
        .status(StatusCode::PARTIAL_CONTENT)
        .header(header::CONTENT_LENGTH, length)
        .header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{size}"))
        .body(body)?)
}

/// `bytes=a-b`, `bytes=a-` or `bytes=-n`; None when it cannot be served.
fn parse_range(header: &str, size: u64) -> Option<(u64, u64)> {
    let spec = header.trim().strip_prefix("bytes=")?;
    let (start, end) = spec.split_once('-')?;
    let (start, end) = match (start.trim(), end.trim()) {
        ("", "") => return None,
        ("", suffix) => (size.saturating_sub(suffix.parse().ok()?), size.checked_sub(1)?),
        (start, "") => (start.parse().ok()?, size.checked_sub(1)?),
        (start, end) => (start.parse().ok()?, end.parse().ok()?),
    };
    if start >= size || start > end {
        return None;
    }
    Some((start, end.min(size - 1)))
}

fn mime_of(file: &Path) -> &'static str {
    match file
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("mp4" | "m4v") => "video/mp4",
        Some("m4a") => "audio/mp4",
        Some("aac") => "audio/aac",
        Some("mov") => "video/quicktime",
        Some("mkv") => "video/x-matroska",
        Some("webm") => "video/webm",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("png") => "image/png",
        _ => "application/octet-stream",
    }
}

fn status(code: StatusCode) -> Response<Vec<u8>> {
    Response::builder()
        .status(code)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Vec::new())
        .expect("static response")
}

#[cfg(test)]
mod tests {
    use super::parse_range;

    #[test]
    fn ranges_parse_like_the_electron_handler() {
        assert_eq!(parse_range("bytes=0-", 100), Some((0, 99)));
        assert_eq!(parse_range("bytes=10-19", 100), Some((10, 19)));
        assert_eq!(parse_range("bytes=90-200", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=-10", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=100-", 100), None);
        assert_eq!(parse_range("bytes=5-1", 100), None);
        assert_eq!(parse_range("bytes=-", 100), None);
        assert_eq!(parse_range("items=0-1", 100), None);
    }
}
