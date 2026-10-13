//! Owns the capture status, pause, save single-flight and restarts (including bounded
//! auto-restart when the capture process dies), and drives `remote`.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use capture_protocol::{AudioDevices, CaptureTarget, EncoderInfo, StartRequest};
use serde::Serialize;
use tokio::sync::mpsc;

use super::canvas;
use super::remote::{self, HostEvent};
use crate::settings::{self, AppSettings};
use crate::{events, paths, tray};

/// Settings changes within this window collapse into one pipeline restart.
const RESTART_DEBOUNCE: Duration = Duration::from_millis(280);
/// A pipeline that dies on its own is restarted this many times in a row before giving up.
const MAX_AUTO_RESTARTS: u32 = 3;
const AUTO_RESTART_DELAY: Duration = Duration::from_secs(2);
/// A failure this long after the previous one starts a fresh run of auto restarts.
const AUTO_RESTART_WINDOW: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureStatus {
    pub ready: bool,
    pub paused: bool,
    pub saving: bool,
    pub starting: bool,
    pub encoder_id: String,
    pub encoder_label: String,
    pub available_encoders: Vec<EncoderInfo>,
    pub error: Option<String>,
    pub buffer_sec: u32,
    /// When the replay buffer last started filling (epoch ms), or null when it is not buffering.
    pub buffered_since: Option<i64>,
    pub capture_target: CaptureTarget,
    pub output_width: u32,
    pub output_height: u32,
    pub fps: u32,
}

#[derive(Default)]
struct State {
    running: bool,
    starting: bool,
    paused: bool,
    saving: bool,
    buffered_since: Option<i64>,
    last_error: Option<String>,
    auto_restarts: u32,
    last_failure: Option<Instant>,
    /// Settings waiting for the debounced restart.
    restart_queued: Option<AppSettings>,
    restart_scheduled: bool,
    restart_running: bool,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with<T>(f: impl FnOnce(&mut State) -> T) -> T {
    let mut state = STATE.lock().unwrap();
    f(state.get_or_insert_with(State::default))
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Listens for the capture process's own events. Call once at startup.
pub fn init() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    remote::set_event_sink(tx);
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                HostEvent::Changed => emit_status(),
                HostEvent::Failed(message) => on_failed(message),
            }
        }
    });
}

fn on_failed(message: String) {
    log::error!("[rikette] capture failed: {message}");
    let retry = with(|s| {
        s.running = false;
        s.last_error = Some(message);
        if s.last_failure.is_none_or(|at| at.elapsed() > AUTO_RESTART_WINDOW) {
            s.auto_restarts = 0;
        }
        s.last_failure = Some(Instant::now());
        if s.auto_restarts >= MAX_AUTO_RESTARTS {
            return false;
        }
        s.auto_restarts += 1;
        true
    });
    emit_status();
    if retry {
        tauri::async_runtime::spawn(async {
            tokio::time::sleep(AUTO_RESTART_DELAY).await;
            if with(|s| !s.running && !s.paused) {
                schedule_restart(settings::load());
            }
        });
    }
}

pub fn status() -> CaptureStatus {
    let settings = settings::load();
    let canvas = canvas::for_settings(&settings);
    let host = remote::state();
    with(|s| {
        let restarting = s.restart_queued.is_some() || s.restart_scheduled || s.restart_running;
        CaptureStatus {
            ready: s.running && !s.paused && s.last_error.is_none() && !restarting,
            paused: s.paused,
            saving: s.saving || restarting,
            starting: s.starting || restarting,
            encoder_id: host.encoder.as_ref().map(|e| e.id.clone()).unwrap_or_default(),
            encoder_label: host.encoder.as_ref().map(|e| e.label.clone()).unwrap_or_default(),
            available_encoders: host.encoders.clone(),
            error: s.last_error.clone(),
            buffer_sec: settings.buffer_sec,
            buffered_since: if s.running && !s.paused { s.buffered_since } else { None },
            capture_target: host.target.clone(),
            output_width: canvas.output_width,
            output_height: canvas.output_height,
            fps: settings.fps,
        }
    })
}

pub fn emit_status() {
    events::emit("capture:status", status());
    tray::refresh();
}

pub async fn start(settings: AppSettings) {
    if let Some(reason) = remote::unavailable_reason() {
        log::warn!("[rikette] capture unavailable: {reason}");
        with(|s| s.last_error = Some(reason));
        emit_status();
        return;
    }
    with(|s| s.starting = true);
    emit_status();
    if let Err(error) = run_pipeline(settings).await {
        log::error!("[rikette] capture start failed: {error:#}");
    }
    with(|s| s.starting = false);
    emit_status();
}

pub async fn shutdown() {
    with(|s| {
        s.restart_queued = None;
        s.running = false;
    });
    remote::shutdown().await;
    emit_status();
}

pub fn restart() {
    with(|s| s.auto_restarts = 0);
    schedule_restart(settings::load());
}

pub fn is_paused() -> bool {
    with(|s| s.paused)
}

pub async fn pause() {
    if !with(|s| s.running && !s.paused) {
        return;
    }
    match remote::pause().await {
        Ok(()) => {
            with(|s| s.paused = true);
            log::info!("[rikette] buffering paused");
        }
        Err(error) => with(|s| s.last_error = Some(format!("{error:#}"))),
    }
    emit_status();
}

pub async fn resume() {
    if !with(|s| s.running && s.paused) {
        return;
    }
    with(|s| s.starting = true);
    emit_status();
    match remote::resume().await {
        Ok(()) => {
            with(|s| {
                s.paused = false;
                s.buffered_since = Some(now_ms());
                s.last_error = None;
            });
            log::info!("[rikette] buffering resumed");
        }
        Err(error) => with(|s| s.last_error = Some(format!("{error:#}"))),
    }
    with(|s| s.starting = false);
    emit_status();
}

/// Clears `saving` when the save ends, however it ends.
struct Saving;

impl Drop for Saving {
    fn drop(&mut self) {
        with(|s| s.saving = false);
        emit_status();
    }
}

pub async fn save_replay() -> Result<PathBuf> {
    let (running, paused, saving) = with(|s| (s.running, s.paused, s.saving));
    if !running || paused {
        bail!(if paused { "Buffering is paused" } else { "Replay buffer is not running" });
    }
    if saving {
        bail!("A clip is already saving");
    }
    with(|s| s.saving = true);
    emit_status();
    let _saving = Saving;
    remote::save().await
}

pub async fn list_audio_devices() -> AudioDevices {
    match remote::list_audio_devices().await {
        Ok(devices) => devices,
        Err(error) => {
            log::warn!("[rikette] audio device list failed: {error:#}");
            AudioDevices::default()
        }
    }
}

pub fn set_mic_volume(percent: u32) {
    remote::set_mic_volume(percent);
}

pub fn schedule_restart(settings: AppSettings) {
    let schedule = with(|s| {
        s.restart_queued = Some(settings);
        if s.restart_scheduled || s.restart_running {
            return false;
        }
        s.restart_scheduled = true;
        true
    });
    emit_status();
    if !schedule {
        return;
    }
    tauri::async_runtime::spawn(async {
        tokio::time::sleep(RESTART_DEBOUNCE).await;
        with(|s| {
            s.restart_scheduled = false;
            s.restart_running = true;
        });
        drain_restarts().await;
        with(|s| s.restart_running = false);
        emit_status();
    });
}

pub fn capture_changed(prev: &AppSettings, next: &AppSettings) -> bool {
    prev.buffer_sec != next.buffer_sec
        || prev.capture_mode != next.capture_mode
        || prev.monitor_index != next.monitor_index
        || prev.mic_enabled != next.mic_enabled
        || prev.fps != next.fps
        || prev.quality != next.quality
        || prev.output_res != next.output_res
        || prev.encoder_pref != next.encoder_pref
        || prev.nvenc_preset != next.nvenc_preset
        || prev.nvenc_multipass != next.nvenc_multipass
        || prev.fallback_to_display != next.fallback_to_display
        || prev.desktop_audio_id != next.desktop_audio_id
        || prev.mic_device_id != next.mic_device_id
}

async fn drain_restarts() {
    while let Some(next) = with(|s| s.restart_queued.take()) {
        if remote::unavailable_reason().is_some() {
            return;
        }
        if let Err(error) = run_pipeline(next).await {
            log::error!("[rikette] capture restart failed: {error:#}");
        }
    }
}

fn start_request(settings: &AppSettings) -> StartRequest {
    StartRequest {
        buffer_sec: settings.buffer_sec,
        fps: settings.fps,
        capture_mode: settings.capture_mode,
        fallback_to_display: settings.fallback_to_display,
        monitor_index: settings.monitor_index,
        mic_enabled: settings.mic_enabled,
        mic_volume: settings.mic_volume,
        desktop_audio_id: settings.desktop_audio_id.clone(),
        mic_device_id: settings.mic_device_id.clone(),
        quality: settings.quality,
        encoder_pref: settings.encoder_pref,
        nvenc_preset: settings.nvenc_preset,
        nvenc_multipass: settings.nvenc_multipass,
        canvas: canvas::for_settings(settings),
        display: canvas::display_info(settings.monitor_index),
        data_path: paths::obs_data_dir().to_string_lossy().into_owned(),
        temp_dir: paths::capture_temp_dir().to_string_lossy().into_owned(),
        version: crate::version(),
    }
}

async fn run_pipeline(settings: AppSettings) -> Result<()> {
    with(|s| {
        s.running = false;
        s.paused = false;
    });
    let _ = std::fs::create_dir_all(&settings.output_folder);
    let _ = std::fs::create_dir_all(paths::capture_temp_dir());
    let result = remote::start(start_request(&settings)).await;
    match &result {
        Ok(()) => {
            with(|s| {
                s.running = true;
                s.buffered_since = Some(now_ms());
                s.last_error = None;
            });
            if let Some(encoder) = remote::state().encoder {
                settings::set(|s| {
                    s.encoder_id = encoder.id;
                    s.encoder_label = encoder.label;
                });
            }
        }
        Err(error) => with(|s| s.last_error = Some(format!("{error:#}"))),
    }
    emit_status();
    result
}
