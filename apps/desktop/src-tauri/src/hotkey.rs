//! The global save-clip hotkey. Settings store Electron accelerators ("Alt+1",
//! "CommandOrControl+Shift+S"), which the global-shortcut plugin parses as they are.

use std::sync::Mutex;

use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::{clips, events, overlay};

static REGISTERED: Mutex<Option<Shortcut>> = Mutex::new(None);

#[derive(Debug, Clone, Copy)]
pub enum SaveReason {
    Hotkey,
    Tray,
    Ui,
}

pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|_app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            if REGISTERED.lock().unwrap().as_ref() == Some(shortcut) {
                request_save_clip(SaveReason::Hotkey);
            }
        })
        .build()
}

pub fn request_save_clip(reason: SaveReason) {
    let bound = REGISTERED
        .lock()
        .unwrap()
        .as_ref()
        .map(|s| s.into_string())
        .unwrap_or_else(|| "unbound".into());
    log::info!("[rikette] save clip ({reason:?}) accelerator={bound}");
    overlay::show("Clip Saved");
    tauri::async_runtime::spawn(async {
        if let Err(error) = clips::save_current_clip().await {
            clips::toast_error(&format!("{error:#}"));
        }
    });
}

pub fn register(accelerator: &str) -> bool {
    unregister();
    let shortcut: Shortcut = match accelerator.parse() {
        Ok(shortcut) => shortcut,
        Err(error) => {
            log::warn!("[rikette] bad hotkey {accelerator}: {error}");
            return false;
        }
    };
    match events::app().global_shortcut().register(shortcut) {
        Ok(()) => {
            *REGISTERED.lock().unwrap() = Some(shortcut);
            log::info!("[rikette] registered global hotkey {accelerator}");
            true
        }
        Err(error) => {
            log::warn!("[rikette] failed to register global hotkey {accelerator}: {error}");
            false
        }
    }
}

pub fn unregister() {
    let _ = events::app().global_shortcut().unregister_all();
    *REGISTERED.lock().unwrap() = None;
}

pub fn is_registered() -> bool {
    let shortcut = *REGISTERED.lock().unwrap();
    shortcut.is_some_and(|s| events::app().global_shortcut().is_registered(s))
}
