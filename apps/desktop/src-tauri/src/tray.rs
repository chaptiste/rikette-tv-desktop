//! Tray icon and its menu.

use std::sync::Mutex;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::Wry;

use crate::capture::engine;
use crate::hotkey::{self, SaveReason};
use crate::{events, window};

struct Tray {
    icon: TrayIcon,
    pause: MenuItem<Wry>,
}

static TRAY: Mutex<Option<Tray>> = Mutex::new(None);

pub fn create() -> tauri::Result<()> {
    let app = events::app();
    let open = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", "Pause buffering", true, None::<&str>)?;
    let save = MenuItem::with_id(app, "save", "Save clip", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&open, &pause, &save, &settings, &PredefinedMenuItem::separator(app)?, &quit],
    )?;
    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("Rikette")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|_app, event| match event.id().as_ref() {
            "open" => window::show_main(None),
            "pause" => {
                tauri::async_runtime::spawn(async {
                    if engine::is_paused() {
                        engine::resume().await;
                    } else {
                        engine::pause().await;
                    }
                });
            }
            "save" => hotkey::request_save_clip(SaveReason::Tray),
            "settings" => window::show_main(Some("/settings")),
            "quit" => crate::quit(),
            _ => {}
        })
        .on_tray_icon_event(|_tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                window::show_main(None);
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    let icon = builder.build(app)?;
    *TRAY.lock().unwrap() = Some(Tray { icon, pause });
    refresh();
    Ok(())
}

/// Follow the capture status: pause label and the game or window in the tooltip.
pub fn refresh() {
    let tray = TRAY.lock().unwrap();
    let Some(tray) = tray.as_ref() else { return };
    let status = engine::status();
    let _ = tray
        .pause
        .set_text(if status.paused { "Resume buffering" } else { "Pause buffering" });
    let target = &status.capture_target;
    let tooltip = if target.kind != capture_protocol::TargetKind::Display && !target.exe.is_empty() {
        format!("Rikette · {}", target.exe)
    } else {
        "Rikette".to_string()
    };
    let _ = tray.icon.set_tooltip(Some(tooltip));
}
