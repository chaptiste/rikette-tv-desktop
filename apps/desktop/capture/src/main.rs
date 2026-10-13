//! `rikette-capture`: the process that owns libobs. The app writes one JSON command per line
//! on stdin and reads replies, events and logs on stdout (see `capture-protocol`). It exits
//! when stdin closes. Running libobs here keeps a crash in OBS, or in a game hook, from
//! taking the app down with it.

// Pure logic, so its tests run on any platform.
#[cfg_attr(not(windows), allow(dead_code))]
mod encoders;
mod log_relay;
#[cfg(windows)]
mod obs;
#[cfg(windows)]
mod win;

use std::io::{BufRead, Write};
use std::sync::mpsc;
use std::sync::Mutex;

use capture_protocol::{Call, FromHost, HostState, ToHost};

/// stdout carries the protocol; every write goes through here so lines never interleave.
static OUT: Mutex<()> = Mutex::new(());

pub fn send(message: &FromHost) {
    let Ok(line) = serde_json::to_string(message) else { return };
    let _guard = OUT.lock().unwrap();
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

/// What a pipeline reports on its own, outside a reply.
pub enum Event {
    Changed,
    Failed(String),
}

/// The capture runtime. Calls arrive one at a time on the main thread.
pub trait Backend {
    fn state(&self) -> HostState;
    fn call(&mut self, call: Call) -> Result<serde_json::Value, String>;
    fn set_mic_volume(&mut self, percent: u32);
    /// Work that runs every tick between calls (auto capture, volume following).
    fn tick(&mut self) {}
}

fn main() {
    log_relay::install();
    let (events_tx, events_rx) = mpsc::channel::<Event>();
    #[cfg(windows)]
    let mut backend = obs::ObsBackend::new(events_tx);
    #[cfg(not(windows))]
    let mut backend = Unavailable(events_tx);

    let (lines_tx, lines_rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if lines_tx.send(line).is_err() {
                break;
            }
        }
        // stdin closed: the app is gone or asked us to stop.
    });

    loop {
        match lines_rx.recv_timeout(std::time::Duration::from_millis(250)) {
            Ok(line) => handle_line(&mut backend, &line),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        backend.tick();
        while let Ok(event) = events_rx.try_recv() {
            let state = backend.state();
            send(&match event {
                Event::Changed => FromHost::Changed { state },
                Event::Failed(message) => FromHost::Failed { message, state },
            });
        }
    }
    let _ = backend.call(Call::Shutdown);
}

fn handle_line(backend: &mut dyn Backend, line: &str) {
    let message = match serde_json::from_str::<ToHost>(line) {
        Ok(message) => message,
        Err(error) => {
            log::warn!("[capture] bad message ({error}): {line}");
            return;
        }
    };
    match message {
        ToHost::MicVolume { percent } => backend.set_mic_volume(percent),
        ToHost::Call { id, call } => {
            let result = backend.call(call);
            send(&FromHost::Reply { id, result, state: backend.state() });
        }
    }
}

/// Builds for platforms libobs-rs does not support yet.
#[cfg(not(windows))]
struct Unavailable(#[allow(dead_code)] mpsc::Sender<Event>);

#[cfg(not(windows))]
impl Backend for Unavailable {
    fn state(&self) -> HostState {
        HostState::default()
    }

    fn call(&mut self, call: Call) -> Result<serde_json::Value, String> {
        match call {
            Call::Stop | Call::Shutdown => Ok(serde_json::Value::Null),
            Call::ListAudioDevices => Ok(serde_json::to_value(capture_protocol::AudioDevices::default()).unwrap()),
            _ => Err("Capture is only available on Windows for now".into()),
        }
    }

    fn set_mic_volume(&mut self, _percent: u32) {}
}
