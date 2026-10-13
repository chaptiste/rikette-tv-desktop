//! Everything after a save (remux, trims, thumbnails, uploads, Discord exports), run with the
//! FFmpeg shipped next to the app.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;

use super::discord_plan::DiscordPlan;

/// The FFmpeg build OBS itself uses, next to its DLLs. Elsewhere (macOS) the one on the PATH.
fn binary(name: &str) -> PathBuf {
    let file = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
    let bundled = crate::paths::obs_bin_dir().join(&file);
    if bundled.exists() {
        return bundled;
    }
    PathBuf::from(file)
}

pub fn ffmpeg() -> PathBuf {
    binary("ffmpeg")
}

pub fn ffprobe() -> PathBuf {
    binary("ffprobe")
}

fn command(bin: PathBuf) -> Command {
    let mut command = Command::new(bin);
    command.stdin(Stdio::null()).kill_on_drop(true);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// Run to completion; stdout is returned, stderr ends up in the error.
async fn run(bin: PathBuf, args: &[String]) -> Result<Vec<u8>> {
    let name = bin.display().to_string();
    let output = command(bin)
        .args(args)
        .output()
        .await
        .with_context(|| format!("start {name}"))?;
    if output.status.success() {
        return Ok(output.stdout);
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    bail!("{name} exited {}: {}", output.status, tail(&stderr, 800))
}

fn tail(text: &str, max: usize) -> &str {
    let start = text.len().saturating_sub(max);
    let start = (start..text.len()).find(|i| text.is_char_boundary(*i)).unwrap_or(text.len());
    &text[start..]
}

fn args<const N: usize>(items: [&str; N]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
}

const MUX_FLAT: [&str; 8] = [
    "-avoid_negative_ts",
    "make_zero",
    "-muxpreload",
    "0",
    "-muxdelay",
    "0",
    "-movflags",
    "+faststart",
];

fn path(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

pub async fn remux_faststart(input: &Path, output: &Path) -> Result<()> {
    let mut a = args(["-y", "-fflags", "+genpts", "-i"]);
    a.push(path(input));
    a.extend(args(["-map", "0", "-c", "copy"]));
    a.extend(args(MUX_FLAT));
    a.push(path(output));
    run(ffmpeg(), &a).await.map(drop)
}

/// Library cards, the share page poster and og:image all fit in 1280 wide.
const THUMB_MAX_WIDTH: u32 = 1280;

pub async fn write_thumbnail(input: &Path, output: &Path) -> Result<()> {
    let mut a = args(["-y", "-ss", "1", "-i"]);
    a.push(path(input));
    a.extend(args(["-frames:v", "1", "-vf"]));
    a.push(format!("scale='min({THUMB_MAX_WIDTH},iw)':-2"));
    a.extend(args(["-q:v", "3"]));
    a.push(path(output));
    run(ffmpeg(), &a).await.map(drop)
}

const PEAK_RATE: f64 = 8000.0;
pub const PEAK_BUCKETS: usize = 1600;
pub const PEAK_CACHE_VERSION: u32 = 2;

#[derive(Deserialize)]
struct Probe {
    #[serde(default)]
    streams: Vec<ProbeStream>,
    format: Option<ProbeFormat>,
}

#[derive(Deserialize)]
struct ProbeStream {
    width: Option<u32>,
    height: Option<u32>,
    avg_frame_rate: Option<String>,
    codec_name: Option<String>,
}

#[derive(Deserialize)]
struct ProbeFormat {
    duration: Option<String>,
}

async fn probe(file: &Path, select: &str, entries: &str) -> Result<Probe> {
    let mut a = args(["-v", "error", "-select_streams"]);
    a.push(select.into());
    a.push("-show_entries".into());
    a.push(entries.into());
    a.extend(args(["-of", "json"]));
    a.push(path(file));
    let stdout = run(ffprobe(), &a).await?;
    serde_json::from_slice(&stdout).context("parse ffprobe output")
}

pub async fn count_audio_streams(file: &Path) -> Result<usize> {
    Ok(probe(file, "a", "stream=index").await?.streams.len())
}

pub async fn probe_video_codec(file: &Path) -> Result<Option<String>> {
    Ok(probe(file, "v:0", "stream=codec_name")
        .await?
        .streams
        .into_iter()
        .next()
        .and_then(|s| s.codec_name))
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MediaInfo {
    pub duration_ms: i64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
}

pub async fn probe_media(file: &Path) -> Result<MediaInfo> {
    let probe = probe(file, "v:0", "stream=width,height,avg_frame_rate:format=duration").await?;
    let duration: f64 = probe
        .format
        .and_then(|f| f.duration)
        .and_then(|d| d.parse().ok())
        .unwrap_or(0.0);
    let stream = probe.streams.into_iter().next();
    Ok(MediaInfo {
        duration_ms: if duration.is_finite() { (duration * 1000.0).round() as i64 } else { 0 },
        width: stream.as_ref().and_then(|s| s.width).unwrap_or(0),
        height: stream.as_ref().and_then(|s| s.height).unwrap_or(0),
        fps: parse_rate(stream.and_then(|s| s.avg_frame_rate).as_deref()),
    })
}

fn parse_rate(raw: Option<&str>) -> f64 {
    let mut parts = raw.unwrap_or("").split('/');
    let num: f64 = parts.next().and_then(|n| n.parse().ok()).unwrap_or(0.0);
    let den: f64 = parts.next().and_then(|n| n.parse().ok()).unwrap_or(0.0);
    if num == 0.0 || !num.is_finite() {
        return 0.0;
    }
    let rate = if den != 0.0 { num / den } else { num };
    if rate.is_finite() { rate } else { 0.0 }
}

pub async fn extract_audio_track(input: &Path, index: usize, output: &Path) -> Result<()> {
    let mut a = args(["-y", "-i"]);
    a.push(path(input));
    a.push("-map".into());
    a.push(format!("0:a:{index}"));
    a.extend(args(["-vn", "-af", "asetpts=PTS-STARTPTS", "-c:a", "aac", "-b:a", "256k"]));
    a.push(path(output));
    run(ffmpeg(), &a).await.map(drop)
}

/// Loudest sample per bucket, 0–1, for the editor's waveforms.
pub async fn compute_peaks(input: &Path, index: usize, duration_sec: f64) -> Result<Vec<f32>> {
    let mut a = args(["-nostdin", "-i"]);
    a.push(path(input));
    a.push("-map".into());
    a.push(format!("0:a:{index}"));
    a.extend(args([
        "-vn",
        "-af",
        "asetpts=PTS-STARTPTS,aresample=8000,aformat=channel_layouts=mono",
        "-f",
        "f32le",
        "pipe:1",
    ]));
    let raw = run(ffmpeg(), &a).await?;
    let mut peaks = vec![0f32; PEAK_BUCKETS];
    let domain = duration_sec.max(0.001);
    for (i, chunk) in raw.as_chunks::<4>().0.iter().enumerate() {
        let time = i as f64 / PEAK_RATE;
        if time >= domain {
            break;
        }
        let bucket = ((time / domain) * PEAK_BUCKETS as f64).floor() as usize;
        let bucket = bucket.min(PEAK_BUCKETS - 1);
        let amp = f32::from_le_bytes(*chunk).abs();
        if amp > peaks[bucket] {
            peaks[bucket] = amp.min(1.0);
        }
    }
    Ok(peaks)
}

pub fn volumes_need_encode(volumes: &[f64]) -> bool {
    volumes.iter().any(|v| (v - 1.0).abs() > 0.01)
}

pub fn clamp_volume(value: f64) -> f64 {
    if value.is_finite() { value.clamp(0.0, 2.0) } else { 1.0 }
}

pub async fn trim_reencode(input: &Path, output: &Path, start_sec: f64, end_sec: f64) -> Result<()> {
    let count = count_audio_streams(input).await?;
    encode_trim(input, output, start_sec, end_sec, &vec![1.0; count]).await
}

pub async fn trim_with_volumes(
    input: &Path,
    output: &Path,
    start_sec: f64,
    end_sec: f64,
    volumes: &[f64],
) -> Result<()> {
    let volumes: Vec<f64> = volumes.iter().copied().map(clamp_volume).collect();
    encode_trim(input, output, start_sec, end_sec, &volumes).await
}

async fn encode_trim(input: &Path, output: &Path, start_sec: f64, end_sec: f64, volumes: &[f64]) -> Result<()> {
    let start = start_sec.max(0.0);
    let end = end_sec.max(start + 0.25);
    let mut chains = vec!["[0:v:0]setpts=PTS-STARTPTS[v]".to_string()];
    let mut maps = args(["-map", "[v]"]);
    for (index, gain) in volumes.iter().enumerate() {
        let volume = if (gain - 1.0).abs() > 0.01 { format!("volume={gain},") } else { String::new() };
        chains.push(format!("[0:a:{index}]{volume}asetpts=PTS-STARTPTS[a{index}]"));
        maps.push("-map".into());
        maps.push(format!("[a{index}]"));
    }
    let mut a = args(["-y", "-ss"]);
    a.push(format!("{start:.3}"));
    a.push("-to".into());
    a.push(format!("{end:.3}"));
    a.push("-i".into());
    a.push(path(input));
    a.push("-filter_complex".into());
    a.push(chains.join(";"));
    a.extend(maps);
    a.extend(args(["-c:v", "libx264", "-preset", "veryfast", "-crf", "16", "-pix_fmt", "yuv420p"]));
    if volumes.is_empty() {
        a.push("-an".into());
    } else {
        a.extend(args(["-c:a", "aac", "-b:a", "320k"]));
    }
    a.extend(args(MUX_FLAT));
    a.push(path(output));
    run(ffmpeg(), &a).await.map(drop)
}

/// Upload ceiling shared with the API (`CLOUD_BITRATE_CEILING_BPS`).
pub const CLOUD_BITRATE_CEILING_BPS: u64 = 12_000_000;

pub struct CloudEncode {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub cqp: u32,
}

/// H.264 + AAC under the cloud bitrate ceiling, with every audio track mixed into one.
pub async fn transcode_for_cloud(input: &Path, output: &Path, opts: CloudEncode) -> Result<()> {
    let audio_count = count_audio_streams(input).await?;
    let vf = format!(
        "scale={}:{}:flags=bicubic,fps={},setpts=PTS-STARTPTS",
        opts.width, opts.height, opts.fps
    );
    let audio_filter = match audio_count {
        0 => String::new(),
        1 => "[0:a:0]asetpts=PTS-STARTPTS,aformat=channel_layouts=stereo[a]".into(),
        n => {
            let chains: Vec<String> = (0..n).map(|i| format!("[0:a:{i}]asetpts=PTS-STARTPTS[a{i}]")).collect();
            let inputs: String = (0..n).map(|i| format!("[a{i}]")).collect();
            format!(
                "{};{inputs}amix=inputs={n}:duration=first:dropout_transition=0:normalize=0[a]",
                chains.join(";")
            )
        }
    };
    let filter = if audio_filter.is_empty() {
        format!("[0:v:0]{vf}[v]")
    } else {
        format!("[0:v:0]{vf}[v];{audio_filter}")
    };
    let maxrate = format!("{}M", CLOUD_BITRATE_CEILING_BPS / 1_000_000);
    let bufsize = format!("{}M", CLOUD_BITRATE_CEILING_BPS * 2 / 1_000_000);
    let gop = (opts.fps * 2).max(2);
    let mut tail = vec!["-filter_complex".to_string(), filter, "-map".into(), "[v]".into()];
    if audio_count > 0 {
        tail.extend(args(["-map", "[a]", "-c:a", "aac", "-b:a", "160k", "-ac", "2"]));
    } else {
        tail.push("-an".into());
    }
    tail.extend(args(["-pix_fmt", "yuv420p", "-profile:v", "high", "-g"]));
    tail.push(gop.to_string());
    tail.push("-maxrate".into());
    tail.push(maxrate.clone());
    tail.push("-bufsize".into());
    tail.push(bufsize);
    tail.extend(args(MUX_FLAT));
    tail.push(path(output));

    let head = |codec: &[&str]| -> Vec<String> {
        let mut a = args(["-y", "-i"]);
        a.push(path(input));
        a.extend(codec.iter().map(|s| s.to_string()));
        a
    };
    let cq = opts.cqp.to_string();
    let mut nvenc = head(&["-c:v", "h264_nvenc", "-preset", "p4", "-tune", "hq", "-rc", "vbr", "-cq", &cq, "-b:v", &maxrate]);
    nvenc.extend(tail.iter().cloned());
    if let Err(error) = run(ffmpeg(), &nvenc).await {
        log::warn!("[rikette] nvenc upload encode failed, using x264: {error:#}");
        let mut x264 = head(&["-c:v", "libx264", "-preset", "veryfast", "-crf", &cq, "-b:v", &maxrate]);
        x264.extend(tail);
        run(ffmpeg(), &x264).await?;
    }
    Ok(())
}

pub struct SizedEncode<'a> {
    pub start_sec: f64,
    pub end_sec: f64,
    pub volumes: &'a [f64],
    pub plan: DiscordPlan,
    pub pass_log: &'a Path,
}

/// Two-pass x264 at a fixed bitrate so the file size is predictable. Audio tracks are mixed
/// into one stereo track because Discord only plays the first one.
pub async fn encode_for_size(
    input: &Path,
    output: &Path,
    opts: SizedEncode<'_>,
    mut on_progress: Option<&mut (dyn FnMut(f64) + Send)>,
) -> Result<()> {
    let plan = opts.plan;
    let start = opts.start_sec.max(0.0);
    let end = opts.end_sec.max(start + 0.25);
    let duration = end - start;
    let video = format!(
        "[0:v:0]scale={}:{}:flags=bicubic,fps={},setpts=PTS-STARTPTS[v]",
        plan.width, plan.height, plan.fps
    );
    let chains: Vec<String> = opts
        .volumes
        .iter()
        .enumerate()
        .map(|(index, gain)| {
            let volume = if (gain - 1.0).abs() > 0.01 {
                format!("volume={},", clamp_volume(*gain))
            } else {
                String::new()
            };
            format!("[0:a:{index}]{volume}asetpts=PTS-STARTPTS[a{index}]")
        })
        .collect();
    let audio_mix = match chains.len() {
        0 => String::new(),
        1 => format!("{};[a0]aformat=channel_layouts=stereo[a]", chains[0]),
        n => {
            let inputs: String = (0..n).map(|i| format!("[a{i}]")).collect();
            format!(
                "{};{inputs}amix=inputs={n}:duration=first:dropout_transition=0:normalize=0[a]",
                chains.join(";")
            )
        }
    };
    let mut head = args(["-y", "-ss"]);
    head.push(format!("{start:.3}"));
    head.push("-to".into());
    head.push(format!("{end:.3}"));
    head.push("-i".into());
    head.push(path(input));
    let mut video_args = args(["-map", "[v]", "-c:v", "libx264", "-preset", "medium", "-b:v"]);
    video_args.push(format!("{}k", plan.video_kbps));
    video_args.push("-maxrate".into());
    video_args.push(format!("{}k", (plan.video_kbps as f64 * 1.5).round() as i64));
    video_args.push("-bufsize".into());
    video_args.push(format!("{}k", plan.video_kbps * 2));
    video_args.extend(args(["-pix_fmt", "yuv420p", "-profile:v", "high", "-g"]));
    video_args.push((plan.fps * 2).to_string());
    video_args.push("-passlogfile".into());
    video_args.push(path(opts.pass_log));

    let mut pass1 = head.clone();
    pass1.push("-filter_complex".into());
    pass1.push(video.clone());
    pass1.extend(video_args.iter().cloned());
    pass1.extend(args(["-pass", "1", "-an", "-f", "null", "-"]));
    run_with_progress(&pass1, duration, |ratio| {
        if let Some(cb) = on_progress.as_mut() {
            cb(ratio * 0.5)
        }
    })
    .await?;

    let mut pass2 = head;
    pass2.push("-filter_complex".into());
    pass2.push(if audio_mix.is_empty() { video } else { format!("{video};{audio_mix}") });
    pass2.extend(video_args);
    pass2.extend(args(["-pass", "2"]));
    if audio_mix.is_empty() {
        pass2.push("-an".into());
    } else {
        pass2.extend(args(["-map", "[a]", "-c:a", "aac", "-b:a"]));
        pass2.push(format!("{}k", plan.audio_kbps));
        pass2.extend(args(["-ac", "2"]));
    }
    pass2.extend(args(MUX_FLAT));
    pass2.push(path(output));
    run_with_progress(&pass2, duration, |ratio| {
        if let Some(cb) = on_progress.as_mut() {
            cb(0.5 + ratio * 0.5)
        }
    })
    .await
}

async fn run_with_progress(args: &[String], duration_sec: f64, mut on_ratio: impl FnMut(f64)) -> Result<()> {
    let mut child = command(ffmpeg())
        .args(["-nostats", "-progress", "pipe:1"])
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("start ffmpeg")?;
    let stdout = child.stdout.take().ok_or_else(|| anyhow!("no ffmpeg stdout"))?;
    let mut stderr = child.stderr.take().ok_or_else(|| anyhow!("no ffmpeg stderr"))?;
    let stderr_task = tokio::spawn(async move {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text).await;
        text
    });
    let mut lines = BufReader::new(stdout).lines();
    while let Some(line) = lines.next_line().await? {
        if let Some(us) = line.strip_prefix("out_time_us=") {
            if let Ok(us) = us.trim().parse::<f64>() {
                let ratio = us / 1_000_000.0 / duration_sec.max(0.001);
                if ratio.is_finite() {
                    on_ratio(ratio.clamp(0.0, 1.0));
                }
            }
        }
    }
    let status = child.wait().await?;
    let stderr = stderr_task.await.unwrap_or_default();
    if !status.success() {
        bail!("ffmpeg exited {status}: {}", tail(&stderr, 800));
    }
    on_ratio(1.0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_rates_parse_like_ffprobe_prints_them() {
        assert_eq!(parse_rate(Some("60/1")), 60.0);
        assert!((parse_rate(Some("30000/1001")) - 29.97).abs() < 0.01);
        assert_eq!(parse_rate(Some("0/0")), 0.0);
        assert_eq!(parse_rate(None), 0.0);
    }

    #[test]
    fn error_tails_stay_on_char_boundaries() {
        assert_eq!(tail("ééé", 3), "é");
        assert_eq!(tail("abc", 10), "abc");
    }
}
