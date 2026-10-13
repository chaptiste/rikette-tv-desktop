//! Drives `rikette-capture`, the process that owns libobs, over JSON lines on stdin/stdout.
//! A crash in OBS (or in a game hook) ends only that process; the engine restarts it.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use capture_protocol::{AudioDevices, Call, FromHost, HostState, LogLevel, StartRequest, ToHost};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, oneshot};

/// How long shutdown waits for OBS to tear down before killing the process.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
/// A save is answered once OBS has written the file; this bounds a stuck save.
const CALL_TIMEOUT: Duration = Duration::from_secs(60);

/// What the remote reports outside of a reply.
pub enum HostEvent {
    Changed,
    Failed(String),
}

type Reply = oneshot::Sender<Result<serde_json::Value, String>>;

struct Host {
    stdin: mpsc::UnboundedSender<String>,
    generation: u64,
}

static HOST: Mutex<Option<Host>> = Mutex::new(None);
static STATE: Mutex<Option<HostState>> = Mutex::new(None);
static PENDING: Mutex<Option<HashMap<u64, Reply>>> = Mutex::new(None);
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static GENERATION: AtomicU64 = AtomicU64::new(0);
static EVENTS: Mutex<Option<mpsc::UnboundedSender<HostEvent>>> = Mutex::new(None);

pub fn set_event_sink(sink: mpsc::UnboundedSender<HostEvent>) {
    *EVENTS.lock().unwrap() = Some(sink);
}

fn send_event(event: HostEvent) {
    if let Some(sink) = EVENTS.lock().unwrap().as_ref() {
        let _ = sink.send(event);
    }
}

pub fn state() -> HostState {
    STATE.lock().unwrap().clone().unwrap_or_default()
}

/// `rikette-capture` sits next to `obs.dll`, which it links against.
fn host_binary() -> PathBuf {
    let file = if cfg!(windows) { "rikette-capture.exe" } else { "rikette-capture" };
    crate::paths::obs_bin_dir().join(file)
}

/// Null when capture can run here, else a message shown in place of the buffer.
pub fn unavailable_reason() -> Option<String> {
    if !cfg!(windows) {
        return Some("Capture is only available on Windows for now".into());
    }
    if !host_binary().exists() {
        return Some("The capture process is missing from this build".into());
    }
    None
}

pub async fn start(request: StartRequest) -> Result<()> {
    call(Call::Start { request: Box::new(request) }).await.map(drop)
}

pub async fn pause() -> Result<()> {
    call(Call::Pause).await.map(drop)
}

pub async fn resume() -> Result<()> {
    call(Call::Resume).await.map(drop)
}

pub async fn save() -> Result<PathBuf> {
    let value = call(Call::Save).await?;
    let path = value.as_str().ok_or_else(|| anyhow!("Save did not return a path"))?;
    Ok(PathBuf::from(path))
}

pub async fn list_audio_devices() -> Result<AudioDevices> {
    Ok(serde_json::from_value(call(Call::ListAudioDevices).await?)?)
}

pub fn set_mic_volume(percent: u32) {
    if let Some(host) = HOST.lock().unwrap().as_ref() {
        let _ = write(host, &ToHost::MicVolume { percent });
    }
}

pub async fn shutdown() {
    if HOST.lock().unwrap().is_none() {
        return;
    }
    match tokio::time::timeout(SHUTDOWN_TIMEOUT, call(Call::Shutdown)).await {
        Ok(Err(error)) => log::warn!("[rikette] capture host shutdown failed: {error:#}"),
        Err(_) => log::warn!("[rikette] capture host did not stop in time"),
        Ok(Ok(_)) => {}
    }
    // Dropping the writer closes stdin; the reader task kills the process when it sees that.
    HOST.lock().unwrap().take();
    reject_all("Capture host stopped");
}

fn write(host: &Host, message: &ToHost) -> Result<()> {
    let line = serde_json::to_string(message)?;
    host.stdin.send(line).map_err(|_| anyhow!("Capture process is not running"))
}

async fn call(call: Call) -> Result<serde_json::Value> {
    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
    let (tx, rx) = oneshot::channel();
    PENDING.lock().unwrap().get_or_insert_with(HashMap::new).insert(id, tx);
    {
        let mut host = HOST.lock().unwrap();
        if host.is_none() {
            *host = Some(spawn().context("start capture process")?);
        }
        write(host.as_ref().unwrap(), &ToHost::Call { id, call })?;
    }
    match tokio::time::timeout(CALL_TIMEOUT, rx).await {
        Ok(Ok(Ok(value))) => Ok(value),
        Ok(Ok(Err(message))) => bail!(message),
        Ok(Err(_)) => bail!("Capture process stopped"),
        Err(_) => {
            PENDING.lock().unwrap().as_mut().map(|p| p.remove(&id));
            bail!("Capture process did not answer")
        }
    }
}

fn spawn() -> Result<Host> {
    let binary = host_binary();
    let mut command = Command::new(&binary);
    if let Some(dir) = binary.parent() {
        command.current_dir(dir);
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command.spawn()?;
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    log::info!("[rikette] capture host started pid={}", child.id().unwrap_or(0));
    let stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
    let stdout = child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?;
    let stderr = child.stderr.take().ok_or_else(|| anyhow!("no stderr"))?;

    let (tx, rx) = mpsc::unbounded_channel::<String>();
    tauri::async_runtime::spawn(pump_stdin(stdin, rx));
    tauri::async_runtime::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            log::warn!("[capture] {line}");
        }
    });
    tauri::async_runtime::spawn(read_stdout(child, stdout, generation));
    Ok(Host { stdin: tx, generation })
}

async fn pump_stdin(mut stdin: ChildStdin, mut rx: mpsc::UnboundedReceiver<String>) {
    while let Some(line) = rx.recv().await {
        if stdin.write_all(line.as_bytes()).await.is_err()
            || stdin.write_all(b"\n").await.is_err()
            || stdin.flush().await.is_err()
        {
            break;
        }
    }
    // Closing stdin tells the capture process to exit.
}

async fn read_stdout(mut child: Child, stdout: tokio::process::ChildStdout, generation: u64) {
    let mut lines = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        match serde_json::from_str::<FromHost>(&line) {
            Ok(message) => on_message(message),
            Err(error) => log::warn!("[rikette] bad capture message ({error}): {line}"),
        }
    }
    let code = match tokio::time::timeout(Duration::from_secs(2), child.wait()).await {
        Ok(Ok(status)) => status.code().map_or("killed".to_string(), |c| c.to_string()),
        _ => {
            let _ = child.kill().await;
            "killed".to_string()
        }
    };
    let current = {
        let mut host = HOST.lock().unwrap();
        let current = host.as_ref().is_some_and(|h| h.generation == generation);
        if current {
            host.take();
        }
        current
    };
    if let Some(state) = STATE.lock().unwrap().as_mut() {
        state.target = capture_protocol::CaptureTarget::display();
    }
    let message = format!("Capture process exited ({code})");
    reject_all(&message);
    if current {
        log::error!("[rikette] {message}");
        send_event(HostEvent::Failed(message));
    }
}

fn on_message(message: FromHost) {
    match message {
        FromHost::Log { level, text } => match level {
            LogLevel::Debug => log::debug!("{text}"),
            LogLevel::Info => log::info!("{text}"),
            LogLevel::Warn => log::warn!("{text}"),
            LogLevel::Error => log::error!("{text}"),
        },
        FromHost::Reply { id, result, state } => {
            *STATE.lock().unwrap() = Some(state);
            let job = PENDING.lock().unwrap().as_mut().and_then(|p| p.remove(&id));
            if let Some(job) = job {
                let _ = job.send(result);
            }
        }
        FromHost::Changed { state } => {
            *STATE.lock().unwrap() = Some(state);
            send_event(HostEvent::Changed);
        }
        FromHost::Failed { message, state } => {
            *STATE.lock().unwrap() = Some(state);
            send_event(HostEvent::Failed(message));
        }
    }
}

fn reject_all(message: &str) {
    let jobs: Vec<Reply> = PENDING
        .lock()
        .unwrap()
        .as_mut()
        .map(|p| p.drain().map(|(_, job)| job).collect())
        .unwrap_or_default();
    for job in jobs {
        let _ = job.send(Err(message.to_string()));
    }
}
