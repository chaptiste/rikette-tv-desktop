//! The launch window shown while the app checks for an update.

use tauri::webview::PageLoadEvent;
use tauri::{Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::{events, window};

const LABEL: &str = "splash";

pub fn show() {
    if events::app().get_webview_window(LABEL).is_some() {
        return;
    }
    let result = WebviewWindowBuilder::new(events::app(), LABEL, WebviewUrl::App("splash.html".into()))
        .title("Rikette")
        .inner_size(340.0, 320.0)
        .visible(false)
        .decorations(false)
        .resizable(false)
        .maximizable(false)
        .center()
        .additional_browser_args(window::BROWSER_ARGS)
        .background_color(tauri::window::Color(0x07, 0x0b, 0x15, 0xff))
        .on_page_load(|win, payload| {
            if payload.event() == PageLoadEvent::Finished {
                let _ = win.show();
            }
        })
        .build();
    if let Err(error) = result {
        log::warn!("[rikette] splash: {error}");
    }
}

fn window() -> Option<WebviewWindow> {
    events::app().get_webview_window(LABEL)
}

/// `percent` None shows an indeterminate bar.
pub fn set_status(label: &str, percent: Option<f64>) {
    let Some(win) = window() else { return };
    let label = serde_json::to_string(label).unwrap_or_default();
    let percent = percent.map_or("null".to_string(), |p| p.to_string());
    let _ = win.eval(format!("window.setStatus && window.setStatus({label}, {percent})"));
}

/// Brings the splash forward if it is open; false when there is none.
pub fn focus() -> bool {
    let Some(win) = window() else { return false };
    let _ = win.show();
    let _ = win.set_focus();
    true
}

pub fn close() {
    if let Some(win) = window() {
        let _ = win.destroy();
    }
}
