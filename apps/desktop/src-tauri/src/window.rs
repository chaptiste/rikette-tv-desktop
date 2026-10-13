//! The main window. Closing or minimising it hides it to the tray; a window hidden for a
//! while is destroyed so its webview stops using memory, and comes back on the next open.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use tauri::webview::PageLoadEvent;
use tauri::{Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

use crate::events::{self, MAIN_WINDOW};
use crate::autostart;

/// Every webview must start with the same arguments, or WebView2 refuses to create it.
/// Tauri's defaults, plus sound without a click for the save HUD.
pub const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection \
                                --autoplay-policy=no-user-gesture-required";

/// A hidden window is kept this long for a quick reopen, then destroyed.
const DESTROY_HIDDEN_AFTER: Duration = Duration::from_secs(90);

static QUITTING: AtomicBool = AtomicBool::new(false);
/// Bumped on every show and hide, so a stale destroy timer does nothing.
static VISIBILITY: AtomicU64 = AtomicU64::new(0);
/// Route to open once the page has loaded.
static PENDING_ROUTE: Mutex<Option<String>> = Mutex::new(None);
/// Set while the first window of this launch is loading, so login launches stay hidden.
static FIRST_WINDOW: AtomicBool = AtomicBool::new(true);
/// The page in the current main window has finished loading.
static PAGE_LOADED: AtomicBool = AtomicBool::new(false);

pub fn mark_quitting() {
    QUITTING.store(true, Ordering::SeqCst);
}

pub fn is_quitting() -> bool {
    QUITTING.load(Ordering::SeqCst)
}

/// What the page reads before any script runs (see `src/tauri/bridge.ts`).
fn init_script() -> String {
    let platform = if cfg!(windows) {
        "win32"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    };
    format!("window.__RIKETTE__ = {{ platform: {platform:?} }};")
}

pub fn create_main() -> tauri::Result<WebviewWindow> {
    let app = events::app();
    if let Some(win) = app.get_webview_window(MAIN_WINDOW) {
        return Ok(win);
    }
    PAGE_LOADED.store(false, Ordering::SeqCst);
    let builder = WebviewWindowBuilder::new(app, MAIN_WINDOW, WebviewUrl::App("index.html".into()))
        .title("Rikette.tv")
        .inner_size(1180.0, 760.0)
        .min_inner_size(960.0, 600.0)
        .visible(false)
        .background_color(tauri::window::Color(0x07, 0x0b, 0x15, 0xff))
        .initialization_script(init_script())
        .additional_browser_args(BROWSER_ARGS)
        .on_navigation(|url| {
            if is_app_url(url) {
                return true;
            }
            open_external(url.as_str());
            false
        })
        .on_new_window(|url, _| {
            open_external(url.as_str());
            tauri::webview::NewWindowResponse::Deny
        })
        .on_page_load(|win, payload| {
            if payload.event() != PageLoadEvent::Finished {
                return;
            }
            PAGE_LOADED.store(true, Ordering::SeqCst);
            let first = FIRST_WINDOW.swap(false, Ordering::SeqCst);
            if first && autostart::opened_at_login() {
                hide_main();
            } else if !is_quitting() {
                show(&win);
            }
            if let Some(route) = PENDING_ROUTE.lock().unwrap().take() {
                events::emit("app:navigate", route);
            }
        });
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .traffic_light_position(tauri::LogicalPosition::new(14.0, 14.0));
    // Elsewhere the page draws the whole title bar, buttons included (TopNav.tsx).
    #[cfg(not(target_os = "macos"))]
    let builder = builder.decorations(false);

    let win = builder.build()?;
    let handle = win.clone();
    win.on_window_event(move |event| match event {
        WindowEvent::CloseRequested { api, .. } if !is_quitting() => {
            api.prevent_close();
            hide_main();
        }
        WindowEvent::Resized(_) if handle.is_minimized().unwrap_or(false) => {
            let _ = handle.unminimize();
            hide_main();
        }
        _ => {}
    });
    Ok(win)
}

fn is_app_url(url: &tauri::Url) -> bool {
    let host = url.host_str().unwrap_or("");
    matches!(url.scheme(), "tauri" | "asset" | "efmedia" | "ipc")
        || host == "tauri.localhost"
        || host.ends_with(".localhost")
        || (cfg!(debug_assertions) && matches!(host, "localhost" | "127.0.0.1"))
}

fn open_external(url: &str) {
    use tauri_plugin_opener::OpenerExt;
    if let Err(error) = events::app().opener().open_url(url, None::<&str>) {
        log::warn!("[rikette] open {url} failed: {error}");
    }
}

/// Opens (or re-creates) the window, optionally on a route like `/settings`.
pub fn show_main(route: Option<&str>) {
    VISIBILITY.fetch_add(1, Ordering::SeqCst);
    let existing = events::main_window();
    let loading = existing.is_none();
    let win = match existing.map(Ok).unwrap_or_else(create_main) {
        Ok(win) => win,
        Err(error) => {
            log::error!("[rikette] could not open the window: {error}");
            return;
        }
    };
    if let Some(route) = route {
        if loading {
            *PENDING_ROUTE.lock().unwrap() = Some(route.to_string());
        } else {
            events::emit("app:navigate", route.to_string());
        }
    }
    if !loading {
        show(&win);
    }
}

fn show(win: &WebviewWindow) {
    VISIBILITY.fetch_add(1, Ordering::SeqCst);
    let _ = win.set_skip_taskbar(false);
    let _ = win.unminimize();
    let _ = win.show();
    let _ = win.set_focus();
}

pub fn hide_main() {
    let Some(win) = events::main_window() else { return };
    let _ = win.set_skip_taskbar(true);
    let _ = win.hide();
    let token = VISIBILITY.fetch_add(1, Ordering::SeqCst) + 1;
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(DESTROY_HIDDEN_AFTER).await;
        if VISIBILITY.load(Ordering::SeqCst) != token {
            return;
        }
        if let Some(win) = events::main_window() {
            if !win.is_visible().unwrap_or(true) {
                log::info!("[rikette] window hidden for a while, freeing it");
                let _ = win.destroy();
            }
        }
    });
}

/// Waits for the page to load (and its listeners to mount), up to `limit`.
pub async fn wait_until_loaded(limit: Duration) {
    let start = std::time::Instant::now();
    let mut was_loaded = PAGE_LOADED.load(Ordering::SeqCst);
    while !PAGE_LOADED.load(Ordering::SeqCst) && start.elapsed() < limit {
        was_loaded = false;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    if !was_loaded {
        // React mounts its listeners a moment after the load event.
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
}
