//! Messages between the Rikette app and `rikette-capture`, the process that owns libobs.
//!
//! Each message is one line of JSON: the app writes [`ToHost`] to the capture process's
//! stdin and reads [`FromHost`] from its stdout. libobs runs out of process so that a
//! crash in OBS or a game hook takes down only the capture process, which the app restarts.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CaptureMode {
    AutoGame,
    Display,
    Window,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    High,
    Medium,
    Low,
}

impl Quality {
    /// Constant QP (or CRF) for each preset; lower is better.
    pub fn cqp(self) -> u32 {
        match self {
            Quality::High => 15,
            Quality::Medium => 18,
            Quality::Low => 23,
        }
    }
}

/// One speed/quality preset in NVENC terms (p1 fast … p7 best), mapped per encoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NvencPreset {
    P1,
    P4,
    P7,
}

impl NvencPreset {
    pub fn as_str(self) -> &'static str {
        match self {
            NvencPreset::P1 => "p1",
            NvencPreset::P4 => "p4",
            NvencPreset::P7 => "p7",
        }
    }
}

/// Encoders the app knows by name. The OBS ids behind each one differ between OBS versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncoderId {
    JimNvenc,
    FfmpegNvenc,
    H264Amf,
    ObsQsv11,
    VtH264,
    X264,
}

impl EncoderId {
    pub const ALL: [EncoderId; 6] = [
        EncoderId::JimNvenc,
        EncoderId::FfmpegNvenc,
        EncoderId::H264Amf,
        EncoderId::ObsQsv11,
        EncoderId::VtH264,
        EncoderId::X264,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            EncoderId::JimNvenc => "jim_nvenc",
            EncoderId::FfmpegNvenc => "ffmpeg_nvenc",
            EncoderId::H264Amf => "h264_amf",
            EncoderId::ObsQsv11 => "obs_qsv11",
            EncoderId::VtH264 => "vt_h264",
            EncoderId::X264 => "x264",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            EncoderId::JimNvenc => "NVIDIA NVENC",
            EncoderId::FfmpegNvenc => "NVIDIA NVENC (FFmpeg)",
            EncoderId::H264Amf => "AMD AMF",
            EncoderId::ObsQsv11 => "Intel Quick Sync",
            EncoderId::VtH264 => "VideoToolbox",
            EncoderId::X264 => "x264 (CPU)",
        }
    }
}

/// `auto` or one encoder, as stored in settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoderPref {
    Auto,
    Only(EncoderId),
}

impl Serialize for EncoderPref {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            EncoderPref::Auto => serializer.serialize_str("auto"),
            EncoderPref::Only(id) => id.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for EncoderPref {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        if raw == "auto" {
            return Ok(EncoderPref::Auto);
        }
        EncoderId::ALL
            .into_iter()
            .find(|id| id.as_str() == raw)
            .map(EncoderPref::Only)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown encoder {raw}")))
    }
}

/// Physical pixels captured, and the encoded size after the output resolution cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Canvas {
    pub base_width: u32,
    pub base_height: u32,
    pub output_width: u32,
    pub output_height: u32,
}

/// The monitor to capture, resolved by the app (which knows the window system's view of it).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayInfo {
    pub label: String,
    /// Logical size.
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
}

/// Everything the capture process needs to build a pipeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRequest {
    pub buffer_sec: u32,
    pub fps: u32,
    pub capture_mode: CaptureMode,
    pub fallback_to_display: bool,
    pub monitor_index: u32,
    pub mic_enabled: bool,
    pub mic_volume: u32,
    pub desktop_audio_id: String,
    pub mic_device_id: String,
    pub quality: Quality,
    pub encoder_pref: EncoderPref,
    pub nvenc_preset: NvencPreset,
    pub nvenc_multipass: bool,
    pub canvas: Canvas,
    pub display: DisplayInfo,
    /// OBS's own config folder (profiles, logs).
    pub data_path: String,
    /// Where the replay buffer writes saved files before the app moves them.
    pub temp_dir: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncoderInfo {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetKind {
    Game,
    Display,
}

/// What is being recorded right now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureTarget {
    pub kind: TargetKind,
    pub exe: String,
    pub title: String,
}

impl CaptureTarget {
    pub fn display() -> Self {
        CaptureTarget {
            kind: TargetKind::Display,
            exe: String::new(),
            title: "Desktop".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioDevices {
    pub outputs: Vec<AudioDeviceInfo>,
    pub inputs: Vec<AudioDeviceInfo>,
}

/// What the app reads synchronously for its status, sent with every message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostState {
    pub encoder: Option<EncoderInfo>,
    pub encoders: Vec<EncoderInfo>,
    pub target: CaptureTarget,
}

impl Default for HostState {
    fn default() -> Self {
        HostState {
            encoder: None,
            encoders: Vec::new(),
            target: CaptureTarget::display(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "camelCase")]
pub enum Call {
    Start { request: Box<StartRequest> },
    Stop,
    Shutdown,
    Pause,
    Resume,
    /// Replies with the saved file's path.
    Save,
    /// Replies with [`AudioDevices`].
    ListAudioDevices,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ToHost {
    Call {
        id: u64,
        #[serde(flatten)]
        call: Call,
    },
    MicVolume {
        percent: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FromHost {
    Reply {
        id: u64,
        result: Result<serde_json::Value, String>,
        state: HostState,
    },
    /// Something shown in the status changed (capture target, encoder).
    Changed { state: HostState },
    /// The running pipeline died; the app restarts it.
    Failed { message: String, state: HostState },
    Log { level: LogLevel, text: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn call_round_trips_as_one_flat_object() {
        let message = ToHost::Call {
            id: 7,
            call: Call::Save,
        };
        let line = serde_json::to_string(&message).unwrap();
        assert_eq!(line, r#"{"kind":"call","id":7,"method":"save"}"#);
        assert_eq!(serde_json::from_str::<ToHost>(&line).unwrap(), message);
    }

    #[test]
    fn encoder_pref_matches_the_settings_file() {
        let auto: EncoderPref = serde_json::from_str(r#""auto""#).unwrap();
        assert_eq!(auto, EncoderPref::Auto);
        let amf: EncoderPref = serde_json::from_str(r#""h264_amf""#).unwrap();
        assert_eq!(amf, EncoderPref::Only(EncoderId::H264Amf));
        assert_eq!(serde_json::to_string(&amf).unwrap(), r#""h264_amf""#);
        assert!(serde_json::from_str::<EncoderPref>(r#""nope""#).is_err());
    }
}
