//! The "Clip Saved" HUD: a click-through window over the game that plays the save sound.
//! It is created on first use and freed after a few idle minutes.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use tauri::webview::PageLoadEvent;
use tauri::{Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::{events, media_protocol, window};

const LABEL: &str = "overlay";
const HUD_W: f64 = 380.0;
const HUD_H: f64 = 88.0;
const SHOW_FOR: Duration = Duration::from_millis(3200);
const KEEP_IDLE: Duration = Duration::from_secs(300);
/// Fullscreen games keep raising themselves; the HUD re-asserts topmost this often.
const PIN_EVERY: Duration = Duration::from_millis(200);

/// Bumped on every show; timers from an older show do nothing.
static SHOWS: AtomicU64 = AtomicU64::new(0);
/// Title waiting for the page to load.
static QUEUED: Mutex<Option<String>> = Mutex::new(None);

pub fn show(title: &str) {
    let app = events::app();
    let show = SHOWS.fetch_add(1, Ordering::SeqCst) + 1;
    let win = match app.get_webview_window(LABEL) {
        Some(win) => {
            present(&win, title);
            win
        }
        None => {
            *QUEUED.lock().unwrap() = Some(title.to_string());
            match create() {
                Ok(win) => win,
                Err(error) => {
                    log::warn!("[rikette] overlay: {error}");
                    return;
                }
            }
        }
    };
    tauri::async_runtime::spawn(async move {
        let started = std::time::Instant::now();
        while started.elapsed() < SHOW_FOR {
            if SHOWS.load(Ordering::SeqCst) != show {
                return;
            }
            let _ = win.set_always_on_top(true);
            tokio::time::sleep(PIN_EVERY).await;
        }
        let _ = win.hide();
        tokio::time::sleep(KEEP_IDLE).await;
        if SHOWS.load(Ordering::SeqCst) == show {
            let _ = win.destroy();
        }
    });
}

fn create() -> tauri::Result<WebviewWindow> {
    WebviewWindowBuilder::new(events::app(), LABEL, WebviewUrl::App("overlay.html".into()))
        .title("Rikette HUD")
        .inner_size(HUD_W, HUD_H)
        .visible(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .focused(false)
        .focusable(false)
        .content_protected(true)
        .additional_browser_args(window::BROWSER_ARGS)
        .on_page_load(|win, payload| {
            if payload.event() != PageLoadEvent::Finished {
                return;
            }
            let sound = serde_json::to_string(&media_protocol::sound_url()).unwrap_or_default();
            let _ = win.eval(format!("window.__sfx = {sound}"));
            if let Some(title) = QUEUED.lock().unwrap().take() {
                present(&win, &title);
            }
        })
        .build()
        .inspect(|win| {
            let _ = win.set_ignore_cursor_events(true);
        })
}

fn present(win: &WebviewWindow, title: &str) {
    place_on_active_display(win);
    let _ = win.show();
    let _ = win.set_always_on_top(true);
    let title = serde_json::to_string(title).unwrap_or_else(|_| "\"\"".into());
    let _ = win.eval(format!("showToast({title})"));
}

fn place_on_active_display(win: &WebviewWindow) {
    let monitor = win
        .cursor_position()
        .ok()
        .and_then(|p| win.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| win.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else { return };
    let scale = monitor.scale_factor();
    let width = (HUD_W * scale).round() as u32;
    let height = (HUD_H * scale).round() as u32;
    let area = monitor.position();
    let size = monitor.size();
    let x = area.x + (size.width as i32 - width as i32) / 2;
    let y = area.y + (52.0 * scale).round() as i32;
    let _ = win.set_size(PhysicalSize::new(width, height));
    let _ = win.set_position(PhysicalPosition::new(x, y));
}
