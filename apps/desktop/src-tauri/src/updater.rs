//! Updates from GitHub releases (`latest.json`), signed with the key whose public half is
//! baked in at build time as `RIKETTE_UPDATER_PUBKEY`. Builds without it never update.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use tauri_plugin_updater::{Update, UpdaterExt};

use crate::notices::{self, Confirm, ToastKind};
use crate::{capture::engine, events, hotkey, window};

const ENDPOINT: &str = "https://github.com/chaptiste/rikette-tv-desktop/releases/latest/download/latest.json";
const CHECK_EVERY: Duration = Duration::from_secs(4 * 60 * 60);
/// Launch never waits longer than this for the update server to answer.
const LAUNCH_CHECK_TIMEOUT: Duration = Duration::from_secs(6);
/// A launch download that makes no progress for this long finishes in the background.
const LAUNCH_STALL: Duration = Duration::from_secs(20);

static CHECKING: AtomicBool = AtomicBool::new(false);
/// A downloaded update waiting for the user to restart.
static READY: Mutex<Option<(Update, Vec<u8>)>> = Mutex::new(None);

fn pubkey() -> Option<&'static str> {
    option_env!("RIKETTE_UPDATER_PUBKEY").filter(|key| !key.is_empty())
}

pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry, tauri_plugin_updater::Config> {
    let builder = tauri_plugin_updater::Builder::new();
    match pubkey() {
        Some(key) => builder.pubkey(key).build(),
        None => builder.build(),
    }
}

fn enabled() -> bool {
    !cfg!(debug_assertions) && pubkey().is_some()
}

async fn check() -> anyhow::Result<Option<Update>> {
    let updater = events::app()
        .updater_builder()
        .endpoints(vec![ENDPOINT.parse()?])?
        .build()?;
    Ok(updater.check().await?)
}

/// Before the app opens: find, download and install an update. True when the app is
/// quitting to install; false when launch should continue (no update, offline, slow, failed).
pub async fn install_on_launch(on_status: impl Fn(&str, Option<f64>)) -> bool {
    if !enabled() {
        return false;
    }
    let update = match tokio::time::timeout(LAUNCH_CHECK_TIMEOUT, check()).await {
        Ok(Ok(Some(update))) => update,
        Ok(Err(error)) => {
            log::warn!("[rikette] updater launch check: {error:#}");
            return false;
        }
        _ => return false,
    };
    let label = format!("Downloading update {}…", update.version);
    on_status(&label, Some(0.0));

    let bytes = {
        let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel::<f64>();
        let mut downloaded = 0u64;
        let download = update.download(
            move |chunk, total| {
                downloaded += chunk as u64;
                let percent = total.map_or(0.0, |t| downloaded as f64 * 100.0 / t.max(1) as f64);
                let _ = progress_tx.send(percent);
            },
            || {},
        );
        tokio::pin!(download);
        let bytes = loop {
            tokio::select! {
                result = &mut download => match result {
                    Ok(bytes) => break bytes,
                    Err(error) => {
                        log::warn!("[rikette] updater launch download: {error}");
                        return false;
                    }
                },
                percent = tokio::time::timeout(LAUNCH_STALL, progress_rx.recv()) => match percent {
                    Ok(Some(percent)) => on_status(&label, Some(percent)),
                    Ok(None) => {}
                    Err(_) => return false,
                },
            }
        };
        bytes
    };
    on_status("Installing update…", None);
    install(update, bytes).await
}

/// False when the install failed and the app should carry on.
async fn install(update: Update, bytes: Vec<u8>) -> bool {
    hotkey::unregister();
    engine::shutdown().await;
    // On Windows this hands over to the installer, which closes the app and reopens it.
    if let Err(error) = update.install(bytes) {
        log::error!("[rikette] update install failed: {error}");
        notices::toast("Update failed", Some(error.to_string()), ToastKind::Error);
        return false;
    }
    window::mark_quitting();
    events::app().restart();
}

/// `check_now` false when the launch splash has just checked.
pub fn start(check_now: bool) {
    if !enabled() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        if !check_now {
            tokio::time::sleep(CHECK_EVERY).await;
        }
        loop {
            check_now_or_prompt(false).await;
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

pub async fn check_now_or_prompt(user: bool) {
    let version = crate::version();
    if !enabled() {
        if user {
            notices::toast(
                format!("Rikette {version}"),
                Some("Updates are only checked in release builds.".into()),
                ToastKind::Info,
            );
        }
        return;
    }
    if READY.lock().unwrap().is_some() {
        if user {
            prompt_install().await;
        }
        return;
    }
    if CHECKING.swap(true, Ordering::SeqCst) {
        return;
    }
    let result = check().await;
    CHECKING.store(false, Ordering::SeqCst);
    match result {
        Ok(None) => {
            if user {
                notices::toast(
                    format!("Rikette {version}"),
                    Some("You are on the latest version.".into()),
                    ToastKind::Success,
                );
            }
        }
        Ok(Some(update)) => {
            if user {
                notices::toast(
                    "Update found",
                    Some(format!("Downloading {}…", update.version)),
                    ToastKind::Info,
                );
            }
            match update.download(|_, _| {}, || {}).await {
                Ok(bytes) => {
                    *READY.lock().unwrap() = Some((update, bytes));
                    prompt_install().await;
                }
                Err(error) => log::warn!("[rikette] update download: {error}"),
            }
        }
        Err(error) => {
            log::warn!("[rikette] updater check: {error:#}");
            if user {
                notices::toast("Could not check for updates", Some(format!("{error:#}")), ToastKind::Error);
            }
        }
    }
}

async fn prompt_install() {
    let ok = notices::ask_confirm(Confirm {
        title: "Update ready",
        description: "Rikette closes for a few seconds, installs it in the background and reopens on its own.",
        confirm_label: "Restart",
        cancel_label: "Later",
        destructive: false,
    })
    .await;
    if !ok {
        return;
    }
    let Some((update, bytes)) = READY.lock().unwrap().take() else { return };
    window::hide_main();
    if !install(update, bytes).await {
        // Back to recording.
        crate::ipc::apply_side_effects(None, &crate::settings::load());
        engine::start(crate::settings::load()).await;
    }
}
