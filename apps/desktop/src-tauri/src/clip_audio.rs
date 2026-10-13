//! The editor's audio lanes: one preview file and a cached waveform per audio track.

use std::path::Path;
use std::time::UNIX_EPOCH;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::media::ffmpeg::{self, PEAK_CACHE_VERSION};
use crate::{library, media_protocol, paths};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipAudioTrack {
    id: &'static str,
    label: String,
    index: usize,
    peaks: Vec<f32>,
    duration_sec: f64,
    preview_url: String,
}

#[derive(Serialize, Deserialize)]
struct PeakCache {
    v: u32,
    mtime: f64,
    peaks: Vec<f32>,
}

fn mtime_ms(meta: &std::fs::Metadata) -> f64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
}

pub async fn get_clip_audio(id: &str) -> Result<Vec<ClipAudioTrack>> {
    let Some(row) = library::get_clip(id)? else { return Ok(Vec::new()) };
    let source = Path::new(&row.path);
    let Ok(meta) = tokio::fs::metadata(source).await else { return Ok(Vec::new()) };
    let count = ffmpeg::count_audio_streams(source).await?;
    if count == 0 {
        return Ok(Vec::new());
    }
    let dir = paths::audio_dir(id);
    tokio::fs::create_dir_all(&dir).await?;
    let source_mtime = mtime_ms(&meta);
    let duration_sec = (row.duration_ms as f64 / 1000.0).max(0.0);

    let dir = &dir;
    let tracks = (0..count).map(|index| async move {
        let preview = paths::audio_file(id, index);
        if !fresh(&preview, source_mtime).await {
            ffmpeg::extract_audio_track(source, index, &preview).await?;
        }
        let peaks = match load_peaks(dir, index, &preview, source_mtime, duration_sec).await {
            Ok(peaks) => peaks,
            Err(_) => ffmpeg::compute_peaks(source, index, duration_sec).await?,
        };
        let kind = track_kind(count, index);
        anyhow::Ok(ClipAudioTrack {
            id: kind,
            label: track_label(kind, index),
            index,
            peaks,
            duration_sec,
            preview_url: media_protocol::audio_url(id, index, source_mtime as i64),
        })
    });
    futures_util::future::try_join_all(tracks).await
}

pub async fn clear_cache(id: &str) {
    let _ = tokio::fs::remove_dir_all(paths::audio_dir(id)).await;
}

fn track_kind(count: usize, index: usize) -> &'static str {
    match (count, index) {
        (n, 0) if n >= 1 => "desktop",
        (n, 1) if n >= 2 => "mic",
        _ => "audio",
    }
}

fn track_label(kind: &str, index: usize) -> String {
    match kind {
        "desktop" => "Desktop".into(),
        "mic" => "Microphone".into(),
        _ => format!("Audio {}", index + 1),
    }
}

async fn load_peaks(dir: &Path, index: usize, preview: &Path, mtime: f64, duration_sec: f64) -> Result<Vec<f32>> {
    let cache = dir.join(format!("{index}.peaks.json"));
    if let Ok(text) = tokio::fs::read_to_string(&cache).await {
        if let Ok(parsed) = serde_json::from_str::<PeakCache>(&text) {
            if parsed.v == PEAK_CACHE_VERSION && parsed.mtime == mtime && !parsed.peaks.is_empty() {
                return Ok(parsed.peaks);
            }
        }
    }
    let peaks = ffmpeg::compute_peaks(preview, 0, duration_sec).await?;
    let body = serde_json::to_string(&PeakCache { v: PEAK_CACHE_VERSION, mtime, peaks: peaks.clone() })?;
    let _ = tokio::fs::write(&cache, body).await;
    Ok(peaks)
}

async fn fresh(path: &Path, source_mtime: f64) -> bool {
    match tokio::fs::metadata(path).await {
        Ok(meta) => meta.len() > 0 && mtime_ms(&meta) >= source_mtime,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::track_kind;

    #[test]
    fn tracks_are_named_like_the_recorder_writes_them() {
        assert_eq!(track_kind(1, 0), "desktop");
        assert_eq!(track_kind(2, 0), "desktop");
        assert_eq!(track_kind(2, 1), "mic");
        assert_eq!(track_kind(3, 2), "audio");
    }
}
