//! Cloud sharing: mint a link, transcode for the web, upload, and retry from a queue.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use reqwest::{Method, StatusCode};
use serde::Deserialize;
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::library::{self, CloudStatus, LocalClip};
use crate::media::ffmpeg::{self, CloudEncode};
use crate::settings::{self, AppSettings};
use crate::{auth, events, paths};

/// Same as `MAX_UPLOAD_BYTES` and `estimatedUploadBytes` in `packages/shared/src/plans.ts`.
const MAX_UPLOAD_BYTES: u64 = 500 * 1024 * 1024;

fn estimated_upload_bytes(duration_ms: i64) -> u64 {
    let seconds = (duration_ms as f64 / 1000.0).max(1.0);
    let bytes = ((ffmpeg::CLOUD_BITRATE_CEILING_BPS as f64 / 8.0) * seconds * 1.2).ceil() as u64;
    bytes.min(MAX_UPLOAD_BYTES)
}

/// The upload cap per quality preset (`qualityUploadCaps`).
fn upload_caps(settings: &AppSettings) -> (u32, u32) {
    use capture_protocol::Quality;
    match settings.quality {
        Quality::High => (1440, 60),
        Quality::Medium => (1080, 60),
        Quality::Low => (1080, 30),
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Minted {
    id: String,
    public_url: String,
    video_upload_url: String,
    thumb_upload_url: String,
}

/// The API refused for quota reasons; the clip stays local.
#[derive(Debug)]
pub struct QuotaError {
    pub code: String,
}

impl std::fmt::Display for QuotaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.code)
    }
}

impl std::error::Error for QuotaError {}

static TICKING: AtomicBool = AtomicBool::new(false);
static INFLIGHT: Mutex<Option<HashSet<String>>> = Mutex::new(None);
static HOLD: Mutex<Option<HashSet<String>>> = Mutex::new(None);
static DEFERRED: Mutex<Option<HashMap<String, Minted>>> = Mutex::new(None);

fn set_has(set: &Mutex<Option<HashSet<String>>>, id: &str) -> bool {
    set.lock().unwrap().as_ref().is_some_and(|s| s.contains(id))
}

fn set_add(set: &Mutex<Option<HashSet<String>>>, id: &str) -> bool {
    set.lock().unwrap().get_or_insert_with(HashSet::new).insert(id.to_string())
}

fn set_remove(set: &Mutex<Option<HashSet<String>>>, id: &str) {
    if let Some(s) = set.lock().unwrap().as_mut() {
        s.remove(id);
    }
}

/// Removes the id from the in-flight set when dropped.
struct Inflight(String);

impl Inflight {
    fn take(id: &str) -> Option<Self> {
        set_add(&INFLIGHT, id).then(|| Inflight(id.to_string()))
    }
}

impl Drop for Inflight {
    fn drop(&mut self) {
        set_remove(&INFLIGHT, &self.0);
    }
}

pub fn hold(id: &str) {
    set_add(&HOLD, id);
}

pub fn release(id: &str) {
    set_remove(&HOLD, id);
    tauri::async_runtime::spawn(drain());
}

pub async fn wait_for_idle(id: &str) {
    for _ in 0..150 {
        if !set_has(&INFLIGHT, id) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

pub fn start_queue() {
    tauri::async_runtime::spawn(async {
        loop {
            drain().await;
            tokio::time::sleep(Duration::from_secs(4)).await;
        }
    });
}

fn public_url(cloud_id: &str) -> String {
    format!("{}/c/{cloud_id}", settings::load().api_base())
}

fn copy_to_clipboard(text: &str) {
    if let Err(error) = events::app().clipboard().write_text(text) {
        log::warn!("[rikette] clipboard: {error}");
    }
}

/// Mint (or reuse) the public link, put it on the clipboard and upload in the background.
/// None when signed out or offline (the clip is queued).
pub async fn share_clip(id: &str, defer_upload: bool) -> Result<Option<String>> {
    if auth::access_token().is_none() {
        return Ok(None);
    }
    let Some(clip) = library::get_clip(id)? else { return Ok(None) };

    if let Some(cloud_id) = clip.cloud_id.clone() {
        if clip.cloud_status == CloudStatus::Error && !defer_upload {
            let urls = overwrite_urls(&clip).await?;
            library::set_cloud_status(&clip.id, CloudStatus::Uploading)?;
            events::clips_changed();
            start_upload(&clip.id, Minted { id: cloud_id.clone(), public_url: public_url(&cloud_id), ..urls }, false);
        } else if matches!(clip.cloud_status, CloudStatus::Local | CloudStatus::Error) {
            library::set_cloud_status(&clip.id, CloudStatus::Uploading)?;
            events::clips_changed();
        }
        let url = public_url(&cloud_id);
        copy_to_clipboard(&url);
        return Ok(Some(url));
    }

    let minted = match mint(&clip).await {
        Ok(minted) => minted,
        Err(error) if error.downcast_ref::<reqwest::Error>().is_some_and(|e| e.is_connect() || e.is_timeout()) => {
            // Offline: the queue mints it later.
            library::set_cloud_status(&clip.id, CloudStatus::Uploading)?;
            events::clips_changed();
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    copy_to_clipboard(&minted.public_url);
    library::update_clip(&clip.id, |c| {
        c.cloud_id = Some(minted.id.clone());
        c.cloud_status = CloudStatus::Uploading;
    })?;
    events::clips_changed();
    let url = minted.public_url.clone();
    if defer_upload {
        DEFERRED.lock().unwrap().get_or_insert_with(HashMap::new).insert(clip.id.clone(), minted);
    } else if !set_has(&HOLD, &clip.id) {
        start_upload(&clip.id, minted, false);
    }
    Ok(Some(url))
}

/// After a trim: upload the new file over the same link.
pub async fn replace_cloud_object(id: &str) -> Result<()> {
    if auth::access_token().is_none() {
        return Ok(());
    }
    if library::get_clip(id)?.and_then(|c| c.cloud_id).is_none() {
        return Ok(());
    }
    wait_for_idle(id).await;
    let Some(latest) = library::get_clip(id)? else { return Ok(()) };
    let Some(cloud_id) = latest.cloud_id.clone() else { return Ok(()) };
    let Some(_guard) = Inflight::take(id) else { return Ok(()) };
    library::set_cloud_status(id, CloudStatus::Uploading)?;
    events::clips_changed();
    let result = async {
        let urls = overwrite_urls(&latest).await?;
        upload_minted(id, &Minted { id: cloud_id.clone(), public_url: public_url(&cloud_id), ..urls }, true).await
    }
    .await;
    if result.is_err() {
        let _ = library::set_cloud_status(id, CloudStatus::Error);
        events::clips_changed();
    }
    result
}

pub async fn drain() {
    if auth::access_token().is_none() || TICKING.swap(true, Ordering::SeqCst) {
        return;
    }
    let pending: Vec<LocalClip> = library::list_clips()
        .unwrap_or_default()
        .into_iter()
        .filter(|row| matches!(row.cloud_status, CloudStatus::Uploading | CloudStatus::Error))
        .collect();
    for row in pending {
        if auth::access_token().is_none() {
            break;
        }
        if set_has(&HOLD, &row.id) {
            continue;
        }
        let Some(_guard) = Inflight::take(&row.id) else { continue };
        if let Err(error) = retry(&row).await {
            log::warn!("[rikette] upload retry failed {}: {error:#}", row.id);
            let _ = library::set_cloud_status(&row.id, CloudStatus::Error);
            events::clips_changed();
            if error.downcast_ref::<reqwest::Error>().is_some_and(|e| e.is_connect()) {
                break;
            }
        }
    }
    TICKING.store(false, Ordering::SeqCst);
}

async fn retry(clip: &LocalClip) -> Result<()> {
    let deferred = DEFERRED.lock().unwrap().as_mut().and_then(|m| m.remove(&clip.id));
    if let Some(minted) = deferred {
        return upload_minted(&clip.id, &minted, false).await;
    }
    let Some(cloud_id) = clip.cloud_id.clone() else {
        let minted = mint(clip).await?;
        library::update_clip(&clip.id, |c| {
            c.cloud_id = Some(minted.id.clone());
            c.cloud_status = CloudStatus::Uploading;
        })?;
        events::clips_changed();
        return upload_minted(&clip.id, &minted, false).await;
    };
    let urls = overwrite_urls(clip).await?;
    upload_minted(&clip.id, &Minted { id: cloud_id.clone(), public_url: public_url(&cloud_id), ..urls }, false).await
}

fn clip_body(clip: &LocalClip) -> serde_json::Value {
    let (width, height) = upload_dims(clip.width, clip.height);
    let title = clip.title.as_deref().map(str::trim).filter(|t| !t.is_empty());
    serde_json::json!({
        "durationMs": clip.duration_ms.max(1),
        "width": width,
        "height": height,
        "bytes": estimated_upload_bytes(clip.duration_ms),
        "mime": "video/mp4",
        "title": title,
    })
}

async fn mint(clip: &LocalClip) -> Result<Minted> {
    let res = auth::api(Method::POST, "/api/clips").json(&clip_body(clip)).send().await?;
    match res.status() {
        StatusCode::UNAUTHORIZED => bail!("Signed out"),
        StatusCode::PAYMENT_REQUIRED => {
            let body: serde_json::Value = res.json().await.unwrap_or_default();
            let code = body["code"].as_str().unwrap_or("quota").to_string();
            return Err(QuotaError { code }.into());
        }
        status if !status.is_success() => bail!("mint failed ({})", status.as_u16()),
        _ => {}
    }
    res.json().await.context("parse minted clip")
}

async fn overwrite_urls(clip: &LocalClip) -> Result<Minted> {
    let cloud_id = clip.cloud_id.as_deref().ok_or_else(|| anyhow!("Clip is not in the cloud"))?;
    let res = auth::api(Method::POST, &format!("/api/clips/{cloud_id}/overwrite"))
        .json(&clip_body(clip))
        .send()
        .await?;
    if !res.status().is_success() {
        bail!("overwrite urls failed ({})", res.status().as_u16());
    }
    res.json().await.context("parse overwrite urls")
}

fn start_upload(id: &str, minted: Minted, replaced: bool) {
    if set_has(&HOLD, id) {
        return;
    }
    let Some(guard) = Inflight::take(id) else { return };
    let id = id.to_string();
    tauri::async_runtime::spawn(async move {
        let _guard = guard;
        if let Err(error) = upload_minted(&id, &minted, replaced).await {
            log::error!("[rikette] upload failed: {error:#}");
        }
    });
}

async fn upload_minted(id: &str, minted: &Minted, replaced: bool) -> Result<()> {
    let Some(clip) = library::get_clip(id)? else { return Ok(()) };
    tokio::fs::create_dir_all(paths::uploads_dir()).await?;
    let staged = paths::uploads_dir().join(format!("{}.mp4", clip.id));
    let result = encode_and_put(&clip, minted, replaced, &staged).await;
    let _ = tokio::fs::remove_file(&staged).await;
    if result.is_err() {
        let _ = library::set_cloud_status(&clip.id, CloudStatus::Error);
        events::clips_changed();
    }
    result
}

async fn encode_and_put(clip: &LocalClip, minted: &Minted, replaced: bool, staged: &Path) -> Result<()> {
    let settings = settings::load();
    let (width, height) = upload_dims(clip.width, clip.height);
    let cqp = settings.quality.cqp();
    ffmpeg::transcode_for_cloud(
        Path::new(&clip.path),
        staged,
        CloudEncode { width, height, fps: settings.fps.min(upload_caps(&settings).1), cqp },
    )
    .await?;
    let codec = ffmpeg::probe_video_codec(staged).await?;
    if codec.as_deref() != Some("h264") {
        bail!("upload encode produced {}, want h264", codec.as_deref().unwrap_or("unknown"));
    }
    let src = tokio::fs::metadata(&clip.path).await?.len();
    let out = tokio::fs::metadata(staged).await?.len();
    log::info!("[rikette] cloud encode {} {src} -> {out} {width}x{height} cq={cqp}", clip.id);
    put_file(&minted.video_upload_url, staged, "video/mp4").await?;
    if let Some(thumb) = clip.thumb_path.as_deref() {
        // The thumbnail is optional for /complete.
        if tokio::fs::metadata(thumb).await.is_ok() {
            if let Err(error) = put_file(&minted.thumb_upload_url, Path::new(thumb), "image/jpeg").await {
                log::warn!("[rikette] thumb upload failed: {error:#}");
            }
        }
    }
    let body = if replaced { serde_json::json!({ "replaced": true }) } else { serde_json::json!({}) };
    let done = auth::api(Method::POST, &format!("/api/clips/{}/complete", minted.id))
        .json(&body)
        .send()
        .await?;
    if !done.status().is_success() {
        bail!("complete failed ({})", done.status().as_u16());
    }
    library::update_clip(&clip.id, |c| {
        c.cloud_id = Some(minted.id.clone());
        c.cloud_status = if replaced { CloudStatus::Overwritten } else { CloudStatus::Live };
    })?;
    events::clips_changed();
    Ok(())
}

async fn put_file(url: &str, file: &Path, content_type: &str) -> Result<()> {
    let handle = tokio::fs::File::open(file).await?;
    let length = handle.metadata().await?.len();
    let body = reqwest::Body::wrap_stream(tokio_util_stream(handle));
    let mut request = auth::http()
        .put(url)
        .header(reqwest::header::CONTENT_TYPE, content_type)
        .header(reqwest::header::CONTENT_LENGTH, length)
        .body(body);
    if !is_presigned(url) {
        if let Some(token) = auth::access_token() {
            request = request.bearer_auth(token);
        }
    }
    let res = request.send().await?;
    if !res.status().is_success() {
        bail!("PUT {content_type} {}", res.status().as_u16());
    }
    Ok(())
}

/// Reads the file in chunks so a large upload never sits in memory whole.
fn tokio_util_stream(
    file: tokio::fs::File,
) -> impl futures_util::Stream<Item = std::io::Result<Vec<u8>>> + Send + 'static {
    use tokio::io::AsyncReadExt;
    futures_util::stream::unfold(file, |mut file| async move {
        let mut buf = vec![0u8; 256 * 1024];
        match file.read(&mut buf).await {
            Ok(0) => None,
            Ok(n) => {
                buf.truncate(n);
                Some((Ok(buf), file))
            }
            Err(error) => Some((Err(error), file)),
        }
    })
}

fn is_presigned(url: &str) -> bool {
    url.contains("r2.cloudflarestorage.com") || url.contains("X-Amz-Algorithm")
}

fn upload_dims(src_w: i64, src_h: i64) -> (u32, u32) {
    let cap = upload_caps(&settings::load()).0 as i64;
    let height = if src_h > 0 { src_h.min(cap) } else { cap };
    let width = if src_w > 0 && src_h > 0 {
        ((src_w as f64 * height as f64) / src_h as f64).round().max(2.0) as i64
    } else {
        (cap as f64 * 16.0 / 9.0).round() as i64
    };
    ((width - width % 2) as u32, (height - height % 2) as u32)
}

pub async fn delete_cloud_clip(cloud_id: &str) -> Result<()> {
    let res = auth::api(Method::DELETE, &format!("/api/clips/{cloud_id}")).send().await?;
    if res.status() == StatusCode::NOT_FOUND {
        return Ok(());
    }
    if !res.status().is_success() {
        bail!("cloud delete failed ({})", res.status().as_u16());
    }
    Ok(())
}

pub async fn patch_cloud_title(cloud_id: &str, title: &str) -> Result<()> {
    if auth::access_token().is_none() {
        return Ok(());
    }
    let res = auth::api(Method::PATCH, &format!("/api/clips/{cloud_id}"))
        .json(&serde_json::json!({ "title": title }))
        .send()
        .await?;
    if res.status() == StatusCode::NOT_FOUND {
        return Ok(());
    }
    if !res.status().is_success() {
        bail!("cloud rename failed ({})", res.status().as_u16());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_estimates_match_the_shared_package() {
        assert_eq!(estimated_upload_bytes(10_000), 18_000_000);
        assert_eq!(estimated_upload_bytes(0), 1_800_000);
        assert_eq!(estimated_upload_bytes(10 * 60 * 60 * 1000), MAX_UPLOAD_BYTES);
    }
}
