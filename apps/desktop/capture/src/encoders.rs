//! Which OBS encoder backs each encoder the app offers, and its settings.

use capture_protocol::{EncoderId, EncoderInfo, EncoderPref, NvencPreset, StartRequest};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picked {
    pub id: EncoderId,
    pub obs_id: String,
}

impl Picked {
    pub fn x264() -> Self {
        Picked { id: EncoderId::X264, obs_id: "obs_x264".into() }
    }

    pub fn info(&self) -> EncoderInfo {
        EncoderInfo { id: self.obs_id.clone(), label: self.id.label().into() }
    }
}

/// NVENC ids across OBS versions, newest first.
const NVENC_OBS_IDS: &[&str] = &["obs_nvenc_h264_tex", "jim_nvenc", "obs_nvenc_h264", "ffmpeg_nvenc", "h264_nvenc"];

fn matches(id: EncoderId, obs_id: &str) -> bool {
    match id {
        EncoderId::JimNvenc => matches!(obs_id, "obs_nvenc_h264_tex" | "jim_nvenc" | "obs_nvenc_h264"),
        EncoderId::FfmpegNvenc => matches!(obs_id, "ffmpeg_nvenc" | "h264_nvenc"),
        EncoderId::H264Amf => matches!(obs_id, "h264_amf" | "h264_texture_amf" | "ffmpeg_h264_amf"),
        EncoderId::ObsQsv11 => obs_id.starts_with("obs_qsv11") && !obs_id.contains("hevc") && !obs_id.contains("av1"),
        EncoderId::VtH264 => obs_id == "vt_h264",
        EncoderId::X264 => obs_id == "obs_x264",
    }
}

/// The H.264 encoders OBS can open here, one per app encoder, best first.
pub fn list(available: &[String]) -> Vec<Picked> {
    let usable: Vec<&str> = available
        .iter()
        .map(String::as_str)
        .filter(|id| !id.contains("hevc") && !id.contains("av1") && !id.contains("avc1"))
        .collect();
    EncoderId::ALL
        .into_iter()
        .filter_map(|id| {
            let obs_id = NVENC_OBS_IDS
                .iter()
                .copied()
                .find(|n| usable.contains(n) && matches(id, n))
                // Prefer the newer QSV implementation, and texture input over copied frames.
                .or_else(|| usable.iter().copied().filter(|o| matches(id, o)).max_by_key(|o| (!o.contains("soft"), o.ends_with("_v2"))))?;
            Some(Picked { id, obs_id: obs_id.to_string() })
        })
        .collect()
}

pub fn pick(available: &[String], pref: EncoderPref) -> Picked {
    let listed = list(available);
    if let EncoderPref::Only(want) = pref {
        if let Some(hit) = listed.iter().find(|p| p.id == want) {
            return hit.clone();
        }
    }
    listed.into_iter().next().unwrap_or_else(Picked::x264)
}

/// What to try, in order, when the preferred encoder fails to open: the other NVENC ids for
/// NVENC, then x264.
pub fn fallbacks(available: &[String], picked: &Picked) -> Vec<Picked> {
    let mut out = vec![picked.clone()];
    if matches!(picked.id, EncoderId::JimNvenc | EncoderId::FfmpegNvenc) {
        for id in NVENC_OBS_IDS {
            if available.iter().any(|a| a == id) && !out.iter().any(|p| p.obs_id == *id) {
                out.push(Picked { id: picked.id, obs_id: (*id).into() });
            }
        }
    }
    if picked.id != EncoderId::X264 {
        out.push(Picked::x264());
    }
    out
}

fn x264_preset(preset: NvencPreset) -> &'static str {
    match preset {
        NvencPreset::P1 => "ultrafast",
        NvencPreset::P7 => "medium",
        NvencPreset::P4 => "veryfast",
    }
}

fn amf_preset(preset: NvencPreset) -> &'static str {
    match preset {
        NvencPreset::P1 => "speed",
        NvencPreset::P7 => "quality",
        NvencPreset::P4 => "balanced",
    }
}

fn qsv_usage(preset: NvencPreset) -> u32 {
    match preset {
        NvencPreset::P1 => 7,
        NvencPreset::P7 => 1,
        NvencPreset::P4 => 4,
    }
}

pub fn settings(request: &StartRequest, id: EncoderId) -> Value {
    let cqp = request.quality.cqp();
    let preset = request.nvenc_preset;
    match id {
        EncoderId::JimNvenc | EncoderId::FfmpegNvenc => json!({
            "rate_control": "CQP",
            "cqp": cqp,
            "keyint_sec": 2,
            "preset": preset.as_str(),
            "tune": if preset == NvencPreset::P1 { "ll" } else { "hq" },
            "multipass": if request.nvenc_multipass { "qres" } else { "disabled" },
            "profile": "high",
            "psycho_aq": preset != NvencPreset::P1,
            "bf": 2,
        }),
        EncoderId::H264Amf => json!({
            "rate_control": "CQP",
            "qp_i": cqp,
            "qp_p": cqp,
            "preset": amf_preset(preset),
            "keyint_sec": 2,
        }),
        EncoderId::ObsQsv11 => json!({
            "rate_control": "CQP",
            "qpi": cqp,
            "qpp": cqp,
            "qpb": cqp,
            "target_usage": qsv_usage(preset),
            "keyint_sec": 2,
        }),
        EncoderId::VtH264 => json!({ "rate_control": "CRF", "crf": cqp, "profile": "high", "keyint_sec": 2 }),
        EncoderId::X264 => json!({
            "rate_control": "CRF",
            "crf": cqp,
            "preset": x264_preset(preset),
            "profile": "high",
            "keyint_sec": 2,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn nvenc_wins_and_x264_stays_last() {
        let available = ids(&["obs_x264", "h264_texture_amf", "obs_nvenc_h264_tex", "obs_nvenc_hevc_tex"]);
        let listed = list(&available);
        assert_eq!(listed[0], Picked { id: EncoderId::JimNvenc, obs_id: "obs_nvenc_h264_tex".into() });
        assert_eq!(listed.last().unwrap().id, EncoderId::X264);
        assert!(listed.iter().all(|p| !p.obs_id.contains("hevc")));
    }

    #[test]
    fn a_preference_that_is_missing_falls_back_to_the_best() {
        let available = ids(&["obs_x264", "obs_qsv11_v2", "obs_qsv11_soft_v2"]);
        let picked = pick(&available, EncoderPref::Only(EncoderId::H264Amf));
        assert_eq!(picked, Picked { id: EncoderId::ObsQsv11, obs_id: "obs_qsv11_v2".into() });
    }

    #[test]
    fn hardware_falls_back_to_x264() {
        let available = ids(&["obs_x264", "h264_texture_amf"]);
        let order = fallbacks(&available, &pick(&available, EncoderPref::Auto));
        assert_eq!(order.iter().map(|p| p.obs_id.as_str()).collect::<Vec<_>>(), ["h264_texture_amf", "obs_x264"]);
    }
}
