//! `settings.json`, with the same shape and rules as `appSettingsSchema` in
//! `packages/shared/src/settings.ts`.

use std::fs;
use std::sync::Mutex;

use anyhow::{bail, Context, Result};
use capture_protocol::{CaptureMode, EncoderPref, NvencPreset, Quality};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{brand, paths};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputRes {
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "1440")]
    P1440,
    #[serde(rename = "1080")]
    P1080,
    #[serde(rename = "720")]
    P720,
}

impl OutputRes {
    /// Height cap, or None for the monitor's own size.
    pub fn max_height(self) -> Option<u32> {
        match self {
            OutputRes::Native => None,
            OutputRes::P1440 => Some(1440),
            OutputRes::P1080 => Some(1080),
            OutputRes::P720 => Some(720),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub buffer_sec: u32,
    pub capture_mode: CaptureMode,
    pub monitor_index: u32,
    /// Window mode's window, as a `capture_protocol::WindowInfo` id; empty until picked.
    #[serde(default)]
    pub capture_window: String,
    pub mic_enabled: bool,
    pub desktop_audio_id: String,
    pub mic_device_id: String,
    pub mic_volume: u32,
    pub hotkey: String,
    pub quality: Quality,
    pub output_res: OutputRes,
    pub encoder_pref: EncoderPref,
    pub nvenc_preset: NvencPreset,
    pub nvenc_multipass: bool,
    pub fps: u32,
    pub output_folder: String,
    pub autostart: bool,
    pub fallback_to_display: bool,
    pub encoder_id: String,
    pub encoder_label: String,
    pub api_base: String,
}

pub const DEFAULT_HOTKEY: &str = "Alt+1";

impl AppSettings {
    pub fn defaults() -> Self {
        AppSettings {
            buffer_sec: 60,
            capture_mode: CaptureMode::AutoGame,
            monitor_index: 0,
            capture_window: String::new(),
            mic_enabled: false,
            desktop_audio_id: "default".into(),
            mic_device_id: "default".into(),
            mic_volume: 100,
            hotkey: DEFAULT_HOTKEY.into(),
            quality: Quality::High,
            output_res: OutputRes::Native,
            encoder_pref: EncoderPref::Auto,
            nvenc_preset: NvencPreset::P4,
            nvenc_multipass: false,
            fps: 60,
            output_folder: paths::default_output_folder().to_string_lossy().into_owned(),
            autostart: false,
            fallback_to_display: true,
            encoder_id: String::new(),
            encoder_label: String::new(),
            api_base: api_origin().into(),
        }
    }

    fn validate(self) -> Result<Self> {
        if ![15, 30, 60].contains(&self.buffer_sec) {
            bail!("bufferSec must be 15, 30 or 60");
        }
        if ![30, 60].contains(&self.fps) {
            bail!("fps must be 30 or 60");
        }
        if self.mic_volume > 200 {
            bail!("micVolume must be 0–200");
        }
        for (name, value) in [
            ("desktopAudioId", &self.desktop_audio_id),
            ("micDeviceId", &self.mic_device_id),
            ("hotkey", &self.hotkey),
            ("outputFolder", &self.output_folder),
        ] {
            if value.is_empty() {
                bail!("{name} is empty");
            }
        }
        if !(self.api_base.starts_with("https://") || self.api_base.starts_with("http://")) {
            bail!("apiBase is not a URL");
        }
        Ok(self)
    }

    pub fn api_base(&self) -> &str {
        self.api_base.trim_end_matches('/')
    }
}

/// Not user-editable: release builds use production, debug builds the local worker.
fn api_origin() -> &'static str {
    if cfg!(debug_assertions) {
        brand::DEV_ORIGIN
    } else {
        brand::APP_ORIGIN
    }
}

static CACHE: Mutex<Option<AppSettings>> = Mutex::new(None);

pub fn load() -> AppSettings {
    if let Some(settings) = CACHE.lock().unwrap().as_ref() {
        return settings.clone();
    }
    // `persist` fills the cache.
    match read_file() {
        Ok((settings, changed)) => {
            if changed {
                persist(&settings);
            } else {
                *CACHE.lock().unwrap() = Some(settings.clone());
            }
            settings
        }
        Err(error) => {
            log::warn!("[rikette] settings reset to defaults: {error:#}");
            let defaults = AppSettings::defaults();
            persist(&defaults);
            defaults
        }
    }
}

/// Settings on disk merged over the defaults, and whether that changed what is stored.
fn read_file() -> Result<(AppSettings, bool)> {
    let text = fs::read_to_string(paths::settings_file()).context("read settings.json")?;
    let raw: Value = serde_json::from_str(&text).context("parse settings.json")?;
    let Value::Object(stored) = raw else {
        bail!("settings.json is not an object");
    };
    let mut merged = to_object(&AppSettings::defaults());
    for (key, value) in &stored {
        merged.insert(key.clone(), value.clone());
    }
    merged.insert("apiBase".into(), Value::from(api_origin()));
    merged.insert("bufferSec".into(), Value::from(clamp_buffer(stored.get("bufferSec"))));
    let settings = from_object(merged)?;
    let changed = stored.get("apiBase") != Some(&Value::from(settings.api_base.as_str()))
        || stored.get("bufferSec") != Some(&Value::from(settings.buffer_sec));
    Ok((settings, changed))
}

fn clamp_buffer(raw: Option<&Value>) -> u32 {
    let n = raw.and_then(Value::as_f64).unwrap_or(f64::NAN);
    if n == 15.0 || n == 30.0 || n == 60.0 {
        return n as u32;
    }
    if n > 60.0 {
        60
    } else if n >= 30.0 {
        30
    } else {
        15
    }
}

/// Apply a partial update from the UI. Unknown keys are ignored, bad values rejected.
pub fn update(patch: &Value) -> Result<AppSettings> {
    let Value::Object(patch) = patch else {
        bail!("Invalid settings patch");
    };
    let mut merged = to_object(&load());
    for (key, value) in patch {
        if merged.contains_key(key) {
            merged.insert(key.clone(), value.clone());
        }
    }
    merged.insert("apiBase".into(), Value::from(api_origin()));
    let next = from_object(merged)?;
    persist(&next);
    Ok(next)
}

/// Write fields the app owns (the encoder it ended up using), outside a UI update.
pub fn set(change: impl FnOnce(&mut AppSettings)) -> AppSettings {
    let mut next = load();
    change(&mut next);
    persist(&next);
    next
}

fn to_object(settings: &AppSettings) -> serde_json::Map<String, Value> {
    match serde_json::to_value(settings) {
        Ok(Value::Object(map)) => map,
        _ => unreachable!("settings serialize to an object"),
    }
}

fn from_object(map: serde_json::Map<String, Value>) -> Result<AppSettings> {
    serde_json::from_value::<AppSettings>(Value::Object(map))
        .context("invalid settings")?
        .validate()
}

fn persist(settings: &AppSettings) {
    *CACHE.lock().unwrap() = Some(settings.clone());
    let result = fs::create_dir_all(paths::data_dir()).and_then(|_| {
        let text = serde_json::to_string_pretty(settings).expect("settings serialize");
        fs::write(paths::settings_file(), format!("{text}\n"))
    });
    if let Err(error) = result {
        log::warn!("[rikette] could not save settings: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_lengths_snap_to_the_allowed_values() {
        assert_eq!(clamp_buffer(Some(&Value::from(30))), 30);
        assert_eq!(clamp_buffer(Some(&Value::from(45))), 30);
        assert_eq!(clamp_buffer(Some(&Value::from(120))), 60);
        assert_eq!(clamp_buffer(Some(&Value::from(5))), 15);
        assert_eq!(clamp_buffer(None), 15);
    }

    #[test]
    fn the_electron_settings_file_still_parses() {
        let stored = serde_json::json!({
            "bufferSec": 30, "captureMode": "display", "monitorIndex": 1, "micEnabled": true,
            "desktopAudioId": "default", "micDeviceId": "{0.0.1}", "micVolume": 150,
            "hotkey": "CommandOrControl+Shift+S", "quality": "medium", "outputRes": "1080",
            "encoderPref": "h264_amf", "nvencPreset": "p7", "nvencMultipass": true, "fps": 30,
            "outputFolder": "C:\\Clips", "autostart": true, "fallbackToDisplay": false,
            "encoderId": "h264_texture_amf", "encoderLabel": "AMD AMF",
            "apiBase": "https://rikette.tv"
        });
        let Value::Object(map) = stored else { unreachable!() };
        let settings = from_object(map).unwrap();
        assert_eq!(settings.output_res, OutputRes::P1080);
        assert_eq!(settings.encoder_pref, EncoderPref::Only(capture_protocol::EncoderId::H264Amf));
        assert_eq!(to_object(&settings)["outputRes"], "1080");
    }

    #[test]
    fn bad_values_are_rejected() {
        let mut map = to_object(&AppSettings::defaults());
        map.insert("fps".into(), Value::from(144));
        assert!(from_object(map).is_err());
        let mut map = to_object(&AppSettings::defaults());
        map.insert("hotkey".into(), Value::from(""));
        assert!(from_object(map).is_err());
    }
}
