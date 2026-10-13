//! Picks bitrates, resolution and frame rate so an export fits Discord's upload limit.
//! Pure so it can be unit tested without FFmpeg.

pub const DISCORD_LIMIT_BYTES: u64 = 20 * 1024 * 1024;

/// Share of the limit spent on audio + video; the rest absorbs MP4 overhead and rate drift.
const BUDGET_SHARE: f64 = 0.92;
/// Below this the picture is mush; ask the user to trim instead.
const MIN_VIDEO_KBPS: i64 = 200;
/// Above this a short clip gains nothing visible, so it just comes out smaller.
const MAX_VIDEO_KBPS: i64 = 12_000;
/// Bits per pixel per frame needed to keep a game clip watchable with two-pass x264.
const MIN_BPP: f64 = 0.055;
const HEIGHTS: [u32; 7] = [1440, 1080, 900, 720, 540, 480, 360];

pub const TOO_LONG: &str = "This clip is too long to fit in 20 MB. Trim it in the editor and try again.";

#[derive(Debug, Clone, Copy)]
pub struct DiscordSource {
    pub duration_sec: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiscordPlan {
    pub video_kbps: i64,
    pub audio_kbps: i64,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

/// Err(TOO_LONG) when even the smallest picture would be unwatchable.
pub fn plan_discord_export(source: DiscordSource, limit_bytes: u64) -> Result<DiscordPlan, &'static str> {
    let duration = source.duration_sec.max(0.5);
    let total_kbps = ((limit_bytes as f64 * BUDGET_SHARE * 8.0) / 1000.0 / duration).floor() as i64;
    let audio_kbps = if total_kbps >= 1500 {
        128
    } else if total_kbps >= 600 {
        96
    } else {
        64
    };
    let video_kbps = MAX_VIDEO_KBPS.min(total_kbps - audio_kbps);
    if video_kbps < MIN_VIDEO_KBPS {
        return Err(TOO_LONG);
    }

    let source_fps = if source.fps > 0.0 {
        (source.fps.round() as u32).min(60)
    } else {
        60
    };
    let source_height = if source.height > 0 { source.height } else { 1080 };
    let aspect = if source.width > 0 && source.height > 0 {
        source.width as f64 / source.height as f64
    } else {
        16.0 / 9.0
    };
    let candidates: Vec<u32> = HEIGHTS.into_iter().filter(|h| *h <= source_height).collect();
    let fits = |fps: u32| -> Option<u32> {
        candidates.iter().copied().find(|h| {
            let h = *h as f64;
            (video_kbps as f64 * 1000.0) / (h * h * aspect * fps as f64) >= MIN_BPP
        })
    };

    // Keep high frame rate only while it still allows 720p (or the source size if smaller);
    // past that a sharper 30 fps picture reads better than a blurry 60 fps one.
    let smooth_height = if source_fps > 30 { fits(source_fps) } else { None };
    let smooth = smooth_height.is_some_and(|h| h >= 720.min(source_height));
    let fps = if smooth { source_fps } else { 30.min(source_fps) };
    let height = (if smooth { smooth_height } else { fits(fps) })
        .or_else(|| candidates.last().copied())
        .unwrap_or_else(|| source_height.min(360));
    Ok(DiscordPlan {
        video_kbps,
        audio_kbps,
        width: even(height as f64 * aspect),
        height: even(height as f64),
        fps,
    })
}

fn even(value: f64) -> u32 {
    (((value / 2.0).round() * 2.0) as u32).max(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(duration_sec: f64, height: u32, fps: f64) -> DiscordSource {
        DiscordSource {
            duration_sec,
            width: (height as f64 * 16.0 / 9.0).round() as u32,
            height,
            fps,
        }
    }

    fn plan(duration_sec: f64, height: u32, fps: f64) -> DiscordPlan {
        plan_discord_export(source(duration_sec, height, fps), DISCORD_LIMIT_BYTES).unwrap()
    }

    fn expected_bytes(plan: &DiscordPlan, duration_sec: f64) -> f64 {
        ((plan.video_kbps + plan.audio_kbps) as f64 * 1000.0 * duration_sec) / 8.0
    }

    #[test]
    fn every_buffer_length_fits_under_20_mb() {
        for seconds in [5.0, 15.0, 30.0, 60.0, 120.0] {
            let plan = plan(seconds, 1440, 60.0);
            assert!(expected_bytes(&plan, seconds) < DISCORD_LIMIT_BYTES as f64, "{seconds}s overshoots");
        }
    }

    #[test]
    fn short_clips_keep_1080p60_long_ones_step_down() {
        let short = plan(15.0, 1440, 60.0);
        assert_eq!(short.height, 1080);
        assert_eq!(short.fps, 60);
        let long = plan(60.0, 1440, 60.0);
        assert!(long.height < 1080);
        assert_eq!(long.fps, 30);
    }

    #[test]
    fn a_full_minute_never_drops_below_720p() {
        for seconds in [40.0, 50.0, 58.0, 60.0] {
            let plan = plan(seconds, 1080, 60.0);
            assert!(plan.height >= 720, "{seconds}s: {}p{}", plan.height, plan.fps);
        }
    }

    #[test]
    fn never_upscales_or_exceeds_the_source_frame_rate() {
        let plan = plan(5.0, 720, 30.0);
        assert_eq!(plan.height, 720);
        assert_eq!(plan.fps, 30);
        assert_eq!(plan.width % 2, 0);
    }

    #[test]
    fn tiny_clips_do_not_waste_bitrate() {
        assert!(plan(2.0, 1440, 60.0).video_kbps <= 12_000);
    }

    #[test]
    fn clips_too_long_to_be_watchable_ask_for_a_trim() {
        let result = plan_discord_export(source(30.0 * 60.0, 1440, 60.0), DISCORD_LIMIT_BYTES);
        assert_eq!(result, Err(TOO_LONG));
    }
}
