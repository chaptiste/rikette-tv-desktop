//! Where the app keeps its files. The folders match the Electron build, so an update keeps
//! settings, the clip library, thumbnails and logs.

use std::path::PathBuf;

use crate::brand::{APP_NAME, CLIP_FOLDER_NAME};

/// `%APPDATA%\Rikette` on Windows, `~/Library/Application Support/Rikette` on macOS.
pub fn data_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(APP_NAME)
}

pub fn logs_dir() -> PathBuf {
    data_dir().join("logs")
}

pub fn settings_file() -> PathBuf {
    data_dir().join("settings.json")
}

pub fn library_db() -> PathBuf {
    data_dir().join("library.db")
}

pub fn thumbs_dir() -> PathBuf {
    data_dir().join("thumbs")
}

pub fn thumb_file(id: &str) -> PathBuf {
    thumbs_dir().join(format!("{id}.jpg"))
}

pub fn audio_dir(id: &str) -> PathBuf {
    data_dir().join("audio").join(id)
}

pub fn audio_file(id: &str, index: usize) -> PathBuf {
    audio_dir(id).join(format!("{index}.v2.m4a"))
}

pub fn uploads_dir() -> PathBuf {
    data_dir().join("uploads")
}

/// OBS's config folder: profiles, scene collections and its own logs.
pub fn obs_data_dir() -> PathBuf {
    data_dir().join("osn-data")
}

/// The replay buffer writes here; saved clips are then moved into the output folder.
pub fn capture_temp_dir() -> PathBuf {
    std::env::temp_dir().join(CLIP_FOLDER_NAME)
}

/// The trimmed OBS runtime (see `scripts/obs-runtime.mjs`): libobs, its plugins, FFmpeg and
/// `rikette-capture`, all side by side so they share one set of FFmpeg DLLs.
pub fn obs_bin_dir() -> PathBuf {
    use tauri::Manager;
    let resources = crate::events::app()
        .path()
        .resource_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    resources.join("obs").join("bin").join("64bit")
}

pub fn default_output_folder() -> PathBuf {
    dirs::video_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join(CLIP_FOLDER_NAME)
}
