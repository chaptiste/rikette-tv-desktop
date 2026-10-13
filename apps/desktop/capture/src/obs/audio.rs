//! Desktop audio and microphone sources: desktop on track 1, the mic on track 2.

use capture_protocol::{AudioDeviceInfo, AudioDevices, StartRequest};
use libobs as sys;
use serde_json::json;

use super::ffi::{self, ListItem};
use crate::win;

/// Output channels: 0 is the scene, 1 desktop audio, 2 the mic.
const DESKTOP_CHANNEL: u32 = 1;
const MIC_CHANNEL: u32 = 2;
/// Mixer bits: desktop goes to track 1, the mic to track 2.
pub const DESKTOP_MIXER: u32 = 1;
pub const MIC_MIXER: u32 = 2;

pub struct Audio {
    pub desktop: *mut sys::obs_source_t,
    pub mic: Option<*mut sys::obs_source_t>,
    /// Last headset volume applied to the desktop source.
    followed: Option<f32>,
}

impl Audio {
    pub fn attach(request: &StartRequest) -> Result<Self, String> {
        let desktop = create("wasapi_output_capture", "Rikette Desktop Audio", &request.desktop_audio_id, DESKTOP_MIXER)?;
        unsafe { sys::obs_set_output_source(DESKTOP_CHANNEL, desktop) };
        let mic = if request.mic_enabled {
            let mic = create("wasapi_input_capture", "Rikette Mic", &request.mic_device_id, MIC_MIXER)?;
            unsafe {
                sys::obs_source_set_volume(mic, volume_mul(request.mic_volume));
                sys::obs_set_output_source(MIC_CHANNEL, mic);
            }
            Some(mic)
        } else {
            None
        };
        let mut audio = Audio { desktop, mic, followed: None };
        audio.follow_output_volume();
        Ok(audio)
    }

    pub fn set_mic_volume(&self, percent: u32) {
        if let Some(mic) = self.mic {
            unsafe { sys::obs_source_set_volume(mic, volume_mul(percent)) };
        }
    }

    /// Desktop audio is recorded at the headset volume, so a clip sounds like what you heard.
    pub fn follow_output_volume(&mut self) {
        let Some(volume) = win::audio::output_volume() else { return };
        if self.followed.is_some_and(|v| (v - volume).abs() < 0.005) {
            return;
        }
        if self.followed.is_none_or(|v| (v - volume).abs() >= 0.02) {
            log::info!("[capture] headset volume={:.0}%", volume * 100.0);
        }
        self.followed = Some(volume);
        unsafe { sys::obs_source_set_volume(self.desktop, volume.clamp(0.0, 1.0)) };
    }

    pub fn release(self) {
        unsafe {
            sys::obs_set_output_source(DESKTOP_CHANNEL, std::ptr::null_mut());
            sys::obs_set_output_source(MIC_CHANNEL, std::ptr::null_mut());
            sys::obs_source_release(self.desktop);
            if let Some(mic) = self.mic {
                sys::obs_source_release(mic);
            }
        }
    }
}

fn create(kind: &str, name: &str, preferred: &str, mixer: u32) -> Result<*mut sys::obs_source_t, String> {
    let source = ffi::source_create(kind, name, &json!({})).ok_or_else(|| format!("Could not create {kind}"))?;
    let chosen = pick_device(&ffi::list_items(source, "device_id"), preferred);
    let device = chosen.as_ref().map_or_else(|| preferred_or_default(preferred), |item| item.value.clone());
    ffi::source_update(source, &json!({ "device_id": device }));
    unsafe {
        sys::obs_source_set_muted(source, false);
        sys::obs_source_set_audio_mixers(source, mixer);
    }
    log::info!(
        "[capture] audio {name} device={} id={device}",
        chosen.map_or_else(|| preferred.to_string(), |item| item.name)
    );
    Ok(source)
}

fn preferred_or_default(preferred: &str) -> String {
    if preferred.is_empty() { "default".into() } else { preferred.into() }
}

fn pick_device(items: &[ListItem], preferred: &str) -> Option<ListItem> {
    if !preferred.is_empty() && preferred != "default" {
        if let Some(hit) = items.iter().find(|item| item.value == preferred) {
            return Some(hit.clone());
        }
    }
    items
        .iter()
        .find(|item| item.name.to_lowercase().contains("default") || item.value == "default")
        .or_else(|| items.first())
        .cloned()
}

fn volume_mul(percent: u32) -> f32 {
    (percent as f32 / 100.0).clamp(0.0, 2.0)
}

/// Devices from Windows and from the live OBS sources, de-duplicated by id.
pub fn list_devices(live: Option<&Audio>) -> AudioDevices {
    let from_win = win::audio::devices();
    let from_live = |source: Option<*mut sys::obs_source_t>| -> Vec<AudioDeviceInfo> {
        source
            .map(|s| {
                ffi::list_items(s, "device_id")
                    .into_iter()
                    .map(|item| AudioDeviceInfo { id: item.value, label: item.name })
                    .collect()
            })
            .unwrap_or_default()
    };
    AudioDevices {
        outputs: merge([from_win.outputs, from_live(live.map(|a| a.desktop))]),
        inputs: merge([from_win.inputs, from_live(live.and_then(|a| a.mic))]),
    }
}

fn merge<const N: usize>(groups: [Vec<AudioDeviceInfo>; N]) -> Vec<AudioDeviceInfo> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for device in groups.into_iter().flatten() {
        let id = device.id.trim().to_string();
        let label = device.label.trim().to_string();
        if id.is_empty() || id == "default" || id.eq_ignore_ascii_case("disabled") || label.eq_ignore_ascii_case("disabled") {
            continue;
        }
        if seen.insert(id.clone()) {
            out.push(AudioDeviceInfo { label: if label.is_empty() { id.clone() } else { label }, id });
        }
    }
    out
}
