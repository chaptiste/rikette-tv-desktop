//! The app handle, for modules that push events to the window on their own.

use std::sync::OnceLock;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

pub const MAIN_WINDOW: &str = "main";

static APP: OnceLock<AppHandle> = OnceLock::new();

pub fn init(app: AppHandle) {
    let _ = APP.set(app);
}

pub fn app() -> &'static AppHandle {
    APP.get().expect("events::init runs during setup")
}

pub fn main_window() -> Option<WebviewWindow> {
    APP.get()?.get_webview_window(MAIN_WINDOW)
}

/// Send to the main window only; the overlay and splash pages listen to nothing.
pub fn emit<S: Serialize + Clone>(event: &str, payload: S) {
    if let Some(app) = APP.get() {
        if let Err(error) = app.emit_to(MAIN_WINDOW, event, payload) {
            log::warn!("[rikette] emit {event} failed: {error}");
        }
    }
}

pub fn clips_changed() {
    emit("clips:changed", ());
}
