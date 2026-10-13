//! The libobs pipeline: video, a scene, desktop and mic audio, and a replay buffer output.
//! Runs on the capture process's main thread; libobs calls back on its own threads only
//! through the signal handlers below.

mod audio;
mod ffi;
mod scene;

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use capture_protocol::{Call, EncoderInfo, HostState, StartRequest};
use libobs as sys;
use serde_json::{json, Value};

use self::audio::{Audio, DESKTOP_MIXER, MIC_MIXER};
use self::scene::Scene;
use crate::encoders::{self, Picked};
use crate::{win, Backend, Event};

const SAVE_TIMEOUT: Duration = Duration::from_secs(30);
/// OBS caps the replay buffer itself; the app never asks for more.
const MAX_REPLAY_SEC: u32 = 60;
const AUDIO_BITRATE: i64 = 320;
const VOLUME_EVERY: Duration = Duration::from_millis(400);

/// Fired from OBS's output thread.
static SAVED: Mutex<Option<mpsc::Sender<()>>> = Mutex::new(None);
static EVENTS: OnceLock<Mutex<mpsc::Sender<Event>>> = OnceLock::new();
/// Set while the app itself stops the output, so its stop signal is not a failure.
static STOPPING: AtomicBool = AtomicBool::new(false);

struct Pipeline {
    scene: Scene,
    audio: Audio,
    output: *mut sys::obs_output_t,
    video_encoder: *mut sys::obs_encoder_t,
    audio_encoders: Vec<*mut sys::obs_encoder_t>,
}

pub struct ObsBackend {
    started: bool,
    modules_loaded: bool,
    pipeline: Option<Pipeline>,
    picked: Option<Picked>,
    encoders: Vec<EncoderInfo>,
    last_volume: Option<Instant>,
}

impl ObsBackend {
    pub fn new(events: mpsc::Sender<Event>) -> Self {
        let _ = EVENTS.set(Mutex::new(events));
        win::audio::init_com();
        ffi::install_log_handler();
        ObsBackend { started: false, modules_loaded: false, pipeline: None, picked: None, encoders: Vec::new(), last_volume: None }
    }

    fn startup(&mut self, request: &StartRequest) -> Result<(), String> {
        if self.started {
            return Ok(());
        }
        std::fs::create_dir_all(&request.data_path).map_err(|e| e.to_string())?;
        let locale = ffi::cstr("en-US");
        let config = ffi::cstr(&request.data_path);
        if !unsafe { sys::obs_startup(locale.as_ptr(), config.as_ptr(), std::ptr::null_mut()) } {
            return Err("OBS failed to start".into());
        }
        let audio = sys::obs_audio_info { samples_per_sec: 48_000, speakers: sys::speaker_layout_SPEAKERS_STEREO };
        if !unsafe { sys::obs_reset_audio(&audio) } {
            return Err("OBS could not open audio".into());
        }
        self.started = true;
        log::info!("[capture] OBS {} started", ffi::string(unsafe { sys::obs_get_version_string() }));
        Ok(())
    }

    fn reset_video(&self, request: &StartRequest) -> Result<(), String> {
        static GRAPHICS: OnceLock<std::ffi::CString> = OnceLock::new();
        let module = GRAPHICS.get_or_init(|| ffi::cstr("libobs-d3d11"));
        let canvas = request.canvas;
        let mut ovi: sys::obs_video_info = unsafe { std::mem::zeroed() };
        ovi.graphics_module = module.as_ptr();
        ovi.fps_num = request.fps;
        ovi.fps_den = 1;
        ovi.base_width = canvas.base_width;
        ovi.base_height = canvas.base_height;
        ovi.output_width = canvas.output_width;
        ovi.output_height = canvas.output_height;
        ovi.output_format = sys::video_format_VIDEO_FORMAT_NV12;
        ovi.adapter = 0;
        ovi.gpu_conversion = true;
        ovi.colorspace = sys::video_colorspace_VIDEO_CS_709;
        ovi.range = sys::video_range_type_VIDEO_RANGE_PARTIAL;
        ovi.scale_type = sys::obs_scale_type_OBS_SCALE_BICUBIC;
        // The previous output stops asynchronously; video cannot reset while it is active.
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let code = unsafe { sys::obs_reset_video(&mut ovi) };
            if code == sys::OBS_VIDEO_SUCCESS as i32 {
                return Ok(());
            }
            if code != sys::OBS_VIDEO_CURRENTLY_ACTIVE || Instant::now() > deadline {
                return Err(format!("OBS could not start video ({code})"));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn load_modules(&mut self) {
        if self.modules_loaded {
            return;
        }
        // Found relative to the working directory (bin/64bit), as in OBS's own layout.
        unsafe {
            sys::obs_load_all_modules();
            sys::obs_log_loaded_modules();
            sys::obs_post_load_modules();
        }
        self.modules_loaded = true;
    }

    fn start(&mut self, request: &StartRequest) -> Result<(), String> {
        self.startup(request)?;
        self.stop_pipeline();
        self.reset_video(request)?;
        self.load_modules();

        let available = video_encoder_ids();
        log::info!("[capture] encoder types={}", available.join(","));
        self.encoders = encoders::list(&available).iter().map(Picked::info).collect();
        let want = encoders::pick(&available, request.encoder_pref);

        let scene = Scene::build(request)?;
        let audio = match Audio::attach(request) {
            Ok(audio) => audio,
            Err(error) => {
                scene.release();
                return Err(error);
            }
        };
        let tracks = if request.mic_enabled { 2 } else { 1 };
        let mut last_error = String::new();
        for candidate in encoders::fallbacks(&available, &want) {
            match open_output(request, &candidate, tracks) {
                Ok((output, video_encoder, audio_encoders)) => {
                    if candidate != want {
                        log::warn!("[capture] {} did not open, recording with {}", want.obs_id, candidate.obs_id);
                    }
                    log::info!(
                        "[capture] replay buffer started encoder={} {}x{}@{} buffer={}s tracks={tracks}",
                        candidate.id.label(),
                        request.canvas.output_width,
                        request.canvas.output_height,
                        request.fps,
                        request.buffer_sec
                    );
                    self.picked = Some(candidate);
                    self.pipeline = Some(Pipeline { scene, audio, output, video_encoder, audio_encoders });
                    return Ok(());
                }
                Err(error) => {
                    log::warn!("[capture] {} failed: {error}", candidate.obs_id);
                    last_error = error;
                }
            }
        }
        audio.release();
        scene.release();
        Err(last_error)
    }

    fn stop_pipeline(&mut self) {
        let Some(pipeline) = self.pipeline.take() else { return };
        // A save waiting on this output will never get its signal.
        SAVED.lock().unwrap().take();
        unsafe {
            stop_output(pipeline.output);
            sys::obs_output_release(pipeline.output);
            sys::obs_encoder_release(pipeline.video_encoder);
            for encoder in pipeline.audio_encoders {
                sys::obs_encoder_release(encoder);
            }
        }
        pipeline.audio.release();
        pipeline.scene.release();
    }

    fn save(&mut self) -> Result<Value, String> {
        let pipeline = self.pipeline.as_ref().ok_or("Replay buffer is not running")?;
        let handler = unsafe { sys::obs_output_get_proc_handler(pipeline.output) };
        let (tx, rx) = mpsc::channel();
        *SAVED.lock().unwrap() = Some(tx);
        ffi::proc_call(handler, "save").ok_or("Replay buffer did not accept the save")?;
        match rx.recv_timeout(SAVE_TIMEOUT) {
            Ok(()) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => return Err("Replay buffer save timed out".into()),
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err("Capture pipeline stopped".into()),
        }
        let path = ffi::proc_call(handler, "get_last_replay")
            .and_then(|data| data.string("path"))
            .filter(|path| !path.is_empty())
            .ok_or("Replay buffer saved but reported no file")?;
        Ok(Value::from(path))
    }

    fn shutdown(&mut self) {
        self.stop_pipeline();
        if self.started {
            unsafe { sys::obs_shutdown() };
            self.started = false;
            self.modules_loaded = false;
        }
    }
}

impl Backend for ObsBackend {
    fn state(&self) -> HostState {
        HostState {
            encoder: self.picked.as_ref().map(Picked::info),
            encoders: self.encoders.clone(),
            target: self.pipeline.as_ref().map(|p| p.scene.target()).unwrap_or_else(capture_protocol::CaptureTarget::display),
        }
    }

    fn call(&mut self, call: Call) -> Result<Value, String> {
        match call {
            Call::Start { request } => self.start(&request).map(|_| Value::Null),
            Call::Stop => {
                self.stop_pipeline();
                Ok(Value::Null)
            }
            Call::Shutdown => {
                self.shutdown();
                Ok(Value::Null)
            }
            Call::Pause => {
                if let Some(pipeline) = &self.pipeline {
                    unsafe { stop_output(pipeline.output) };
                }
                Ok(Value::Null)
            }
            Call::Resume => {
                let pipeline = self.pipeline.as_ref().ok_or("Replay buffer is not running")?;
                if !unsafe { sys::obs_output_start(pipeline.output) } {
                    return Err(last_error(pipeline.output));
                }
                Ok(Value::Null)
            }
            Call::Save => self.save(),
            Call::ListAudioDevices => {
                Ok(serde_json::to_value(audio::list_devices(self.pipeline.as_ref().map(|p| &p.audio))).unwrap_or_default())
            }
        }
    }

    fn set_mic_volume(&mut self, percent: u32) {
        if let Some(pipeline) = &self.pipeline {
            pipeline.audio.set_mic_volume(percent);
        }
    }

    fn tick(&mut self) {
        let Some(pipeline) = self.pipeline.as_mut() else { return };
        if pipeline.scene.tick() {
            send_event(Event::Changed);
        }
        if self.last_volume.is_none_or(|at| at.elapsed() >= VOLUME_EVERY) {
            self.last_volume = Some(Instant::now());
            pipeline.audio.follow_output_volume();
        }
    }
}

fn send_event(event: Event) {
    if let Some(events) = EVENTS.get() {
        let _ = events.lock().unwrap().send(event);
    }
}

fn video_encoder_ids() -> Vec<String> {
    let mut ids = Vec::new();
    let mut index = 0;
    loop {
        let mut id: *const std::ffi::c_char = std::ptr::null();
        if !unsafe { sys::obs_enum_encoder_types(index, &mut id) } {
            break;
        }
        index += 1;
        if unsafe { sys::obs_get_encoder_type(id) } == sys::obs_encoder_type_OBS_ENCODER_VIDEO {
            ids.push(ffi::string(id));
        }
    }
    ids
}

type Opened = (*mut sys::obs_output_t, *mut sys::obs_encoder_t, Vec<*mut sys::obs_encoder_t>);

/// Create the replay buffer with this encoder and start it; everything is released on failure.
fn open_output(request: &StartRequest, picked: &Picked, tracks: usize) -> Result<Opened, String> {
    let settings = ffi::Data::from_json(&json!({
        "max_time_sec": request.buffer_sec.min(MAX_REPLAY_SEC),
        // No size cap, so the duration decides what is kept.
        "max_size_mb": 0,
        "directory": request.temp_dir,
        "format": "%CCYY-%MM-%DD %hh-%mm-%ss",
        "extension": "mkv",
        "allow_spaces": true,
    }));
    let (id, name) = (ffi::cstr("replay_buffer"), ffi::cstr("Rikette Replay"));
    let output = unsafe { sys::obs_output_create(id.as_ptr(), name.as_ptr(), settings.0, std::ptr::null_mut()) };
    if output.is_null() {
        return Err("Could not create the replay buffer".into());
    }

    let encoder_settings = ffi::Data::from_json(&encoders::settings(request, picked.id));
    let (encoder_id, encoder_name) = (ffi::cstr(&picked.obs_id), ffi::cstr("Rikette Video"));
    let video_encoder = unsafe {
        sys::obs_video_encoder_create(encoder_id.as_ptr(), encoder_name.as_ptr(), encoder_settings.0, std::ptr::null_mut())
    };
    if video_encoder.is_null() {
        unsafe { sys::obs_output_release(output) };
        return Err(format!("{} is not available", picked.obs_id));
    }
    let mut audio_encoders = Vec::new();
    unsafe {
        sys::obs_encoder_set_video(video_encoder, sys::obs_get_video());
        sys::obs_output_set_video_encoder(output, video_encoder);
        let aac = ffi::cstr("ffmpeg_aac");
        let audio_settings = ffi::Data::from_json(&json!({ "bitrate": AUDIO_BITRATE }));
        for track in 0..tracks {
            let name = ffi::cstr(&format!("Rikette Audio {}", track + 1));
            let encoder = sys::obs_audio_encoder_create(aac.as_ptr(), name.as_ptr(), audio_settings.0, track, std::ptr::null_mut());
            sys::obs_encoder_set_audio(encoder, sys::obs_get_audio());
            sys::obs_output_set_audio_encoder(output, encoder, track);
            audio_encoders.push(encoder);
        }
        let mixers = if tracks > 1 { DESKTOP_MIXER | MIC_MIXER } else { DESKTOP_MIXER };
        sys::obs_output_set_mixers(output, mixers as usize);
        connect_signals(output);
        if sys::obs_output_start(output) {
            return Ok((output, video_encoder, audio_encoders));
        }
        let error = last_error(output);
        sys::obs_output_release(output);
        sys::obs_encoder_release(video_encoder);
        for encoder in audio_encoders {
            sys::obs_encoder_release(encoder);
        }
        Err(error)
    }
}

fn last_error(output: *mut sys::obs_output_t) -> String {
    let error = ffi::string(unsafe { sys::obs_output_get_last_error(output) });
    if error.is_empty() { "The replay buffer could not start".into() } else { error }
}

/// Stop and wait until OBS reports the output inactive.
unsafe fn stop_output(output: *mut sys::obs_output_t) {
    if !sys::obs_output_active(output) {
        return;
    }
    STOPPING.store(true, Ordering::SeqCst);
    sys::obs_output_stop(output);
    let deadline = Instant::now() + Duration::from_secs(5);
    while sys::obs_output_active(output) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    if sys::obs_output_active(output) {
        sys::obs_output_force_stop(output);
    }
    STOPPING.store(false, Ordering::SeqCst);
}

unsafe fn connect_signals(output: *mut sys::obs_output_t) {
    let handler = sys::obs_output_get_signal_handler(output);
    let saved = ffi::cstr("saved");
    let stop = ffi::cstr("stop");
    sys::signal_handler_connect(handler, saved.as_ptr(), Some(on_saved), std::ptr::null_mut());
    sys::signal_handler_connect(handler, stop.as_ptr(), Some(on_stop), std::ptr::null_mut());
}

unsafe extern "C" fn on_saved(_data: *mut c_void, _params: *mut sys::calldata_t) {
    if let Some(tx) = SAVED.lock().unwrap().take() {
        let _ = tx.send(());
    }
}

unsafe extern "C" fn on_stop(_data: *mut c_void, params: *mut sys::calldata_t) {
    let code = ffi::calldata_int(params, "code");
    if code == sys::OBS_OUTPUT_SUCCESS as i64 || STOPPING.load(Ordering::SeqCst) {
        return;
    }
    let output = ffi::calldata_ptr::<sys::obs_output_t>(params, "output");
    let message = if output.is_null() { String::new() } else { ffi::string(sys::obs_output_get_last_error(output)) };
    let message = if message.is_empty() { format!("Replay buffer error {code}") } else { message };
    SAVED.lock().unwrap().take();
    send_event(Event::Failed(message));
}
