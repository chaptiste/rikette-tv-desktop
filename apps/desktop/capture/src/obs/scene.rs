//! The scene: game capture (auto mode), display capture, or one window, scaled to the canvas.

use std::time::{Duration, Instant};

use capture_protocol::{CaptureMode, CaptureTarget, DisplayInfo, StartRequest, TargetKind, WindowInfo};
use libobs as sys;
use serde_json::json;

use super::ffi;
use crate::win::fullscreen::{self, FullscreenApp};

/// libobs `method` for monitor and window capture: Windows Graphics Capture, which works
/// where DXGI duplication returns a 0x0 desktop.
const WINDOWS_GRAPHICS_CAPTURE: i64 = 2;
/// libobs window `priority`: match the executable first, so the window is still found when
/// its title changes (a browser tab, a game's loading screen).
const WINDOW_PRIORITY_EXE: i64 = 2;
const UNHOOKED_LOG_TICKS: u32 = 5;
const TICK: Duration = Duration::from_secs(1);

struct Item {
    source: *mut sys::obs_source_t,
    item: *mut sys::obs_sceneitem_t,
}

pub struct Scene {
    scene: *mut sys::obs_scene_t,
    game: Option<Item>,
    display: Option<Item>,
    window: Option<Item>,
    /// Show the display while no game is hooked.
    fallback: bool,
    locked: Option<FullscreenApp>,
    miss_ticks: u32,
    unhooked_ticks: u32,
    last_tick: Option<Instant>,
    target: CaptureTarget,
}

impl Scene {
    pub fn build(request: &StartRequest) -> Result<Self, String> {
        let name = ffi::cstr("Rikette");
        let scene = unsafe { sys::obs_scene_create(name.as_ptr()) };
        if scene.is_null() {
            return Err("Could not create the OBS scene".into());
        }
        let canvas = (request.canvas.base_width as f32, request.canvas.base_height as f32);
        let auto = request.capture_mode == CaptureMode::AutoGame;
        let mut built = Scene {
            scene,
            game: None,
            display: None,
            window: None,
            fallback: auto && request.fallback_to_display,
            locked: None,
            miss_ticks: 0,
            unhooked_ticks: 0,
            last_tick: None,
            target: CaptureTarget::display(),
        };
        if request.capture_mode == CaptureMode::Display || built.fallback {
            let source = create_display_capture(&request.display, request.monitor_index)?;
            built.display = Some(add_scaled(scene, source, canvas));
        }
        if auto {
            let source = ffi::source_create(
                "game_capture",
                "Rikette Game",
                &json!({
                    "capture_mode": "window",
                    "window": "",
                    "capture_overlays": false,
                    "hook_rate": 1,
                    "allow_transparency": false,
                    "capture_cursor": true,
                    "anti_cheat_hook": true,
                    "capture_audio": false,
                }),
            )
            .ok_or("Could not create game capture")?;
            built.game = Some(add_scaled(scene, source, canvas));
            built.show_display();
        }
        if request.capture_mode == CaptureMode::Window {
            let picked = WindowInfo::parse(&request.window)
                .ok_or("Pick the window to record in Settings > Capture")?;
            let source = ffi::source_create(
                "window_capture",
                "Rikette Window",
                &json!({
                    "window": picked.id,
                    "priority": WINDOW_PRIORITY_EXE,
                    "capture_cursor": true,
                    "method": WINDOWS_GRAPHICS_CAPTURE,
                    "capture_audio": false,
                }),
            )
            .ok_or("Could not create window capture")?;
            log::info!("[capture] window capture {}", picked.id);
            built.window = Some(add_scaled(scene, source, canvas));
            built.target = CaptureTarget {
                kind: TargetKind::Window,
                title: if picked.title.is_empty() { picked.exe.clone() } else { picked.title },
                exe: picked.exe,
            };
        }
        unsafe { sys::obs_set_output_source(0, sys::obs_scene_get_source(scene)) };
        Ok(built)
    }

    pub fn target(&self) -> CaptureTarget {
        self.target.clone()
    }

    /// Auto capture: lock onto the fullscreen game, follow it, release it when it closes.
    /// True when the capture target changed.
    pub fn tick(&mut self) -> bool {
        let Some(game) = self.game.as_ref().map(|g| g.source) else { return false };
        if self.last_tick.is_some_and(|at| at.elapsed() < TICK) {
            return false;
        }
        self.last_tick = Some(Instant::now());
        let before = self.target.clone();
        let found = fullscreen::current();

        if let Some(locked) = self.locked.clone() {
            match found {
                Some(app) if !app.exe.eq_ignore_ascii_case(&locked.exe) => self.lock_game(game, app),
                Some(_) => {
                    self.miss_ticks = 0;
                    self.show_locked(game, &locked);
                }
                None => {
                    self.miss_ticks += 1;
                    if self.miss_ticks >= 4 {
                        if fullscreen::exe_is_running(&locked.exe) {
                            self.miss_ticks = 0;
                            self.show_locked(game, &locked);
                        } else {
                            log::info!("[capture] auto-capture released {}", locked.exe);
                            ffi::source_update(game, &json!({ "capture_mode": "window", "window": "", "hook_rate": 1 }));
                            self.locked = None;
                            self.show_display();
                            self.target = CaptureTarget::display();
                        }
                    }
                }
            }
        } else if let Some(app) = found {
            self.lock_game(game, app);
        } else {
            self.show_display();
            self.target = CaptureTarget::display();
        }
        self.target != before
    }

    fn lock_game(&mut self, game: *mut sys::obs_source_t, app: FullscreenApp) {
        let window = obs_window_id(game, &app);
        ffi::source_update(
            game,
            &json!({ "capture_mode": "window", "window": window, "priority": 2, "hook_rate": 1, "anti_cheat_hook": true }),
        );
        log::info!("[capture] auto-capture locked {} window={window}", app.exe);
        self.miss_ticks = 0;
        self.unhooked_ticks = 0;
        self.show_locked(game, &app);
        self.locked = Some(app);
    }

    // Game capture renders nothing (0x0) until its hook lands, and forever if the hook is
    // blocked (anti-cheat, a non-game fullscreen app). Showing it alone then records black
    // video with live audio. Until it reports a size, keep the display under it: the game
    // source stays enabled so it keeps trying to hook, and covers the display once it draws.
    fn show_locked(&mut self, game: *mut sys::obs_source_t, app: &FullscreenApp) {
        let hooked = unsafe { sys::obs_source_get_width(game) > 0 && sys::obs_source_get_height(game) > 0 };
        if !self.fallback || hooked {
            if self.unhooked_ticks >= UNHOOKED_LOG_TICKS {
                log::info!("[capture] auto-capture hooked {} after {}s", app.exe, self.unhooked_ticks);
            }
            self.unhooked_ticks = 0;
            set_active(self.game.as_ref(), true);
            set_active(self.display.as_ref(), false);
            move_top(self.game.as_ref());
            self.target = CaptureTarget {
                kind: TargetKind::Game,
                exe: app.exe.clone(),
                title: if app.title.is_empty() { app.exe.clone() } else { app.title.clone() },
            };
            return;
        }
        self.unhooked_ticks += 1;
        if self.unhooked_ticks == UNHOOKED_LOG_TICKS {
            log::warn!(
                "[capture] auto-capture has not hooked {} after {UNHOOKED_LOG_TICKS}s, recording the display",
                app.exe
            );
        }
        set_active(self.display.as_ref(), true);
        set_active(self.game.as_ref(), true);
        move_top(self.game.as_ref());
        self.target = CaptureTarget::display();
    }

    fn show_display(&self) {
        if !self.fallback {
            set_active(self.game.as_ref(), true);
            return;
        }
        set_active(self.display.as_ref(), true);
        set_active(self.game.as_ref(), false);
        move_top(self.display.as_ref());
    }

    pub fn release(self) {
        unsafe {
            sys::obs_set_output_source(0, std::ptr::null_mut());
            for item in [self.game, self.display, self.window].into_iter().flatten() {
                sys::obs_sceneitem_remove(item.item);
                sys::obs_source_release(item.source);
            }
            sys::obs_scene_release(self.scene);
        }
    }
}

fn add_scaled(scene: *mut sys::obs_scene_t, source: *mut sys::obs_source_t, canvas: (f32, f32)) -> Item {
    unsafe {
        let item = sys::obs_scene_add(scene, source);
        sys::obs_sceneitem_set_alignment(item, sys::OBS_ALIGN_LEFT | sys::OBS_ALIGN_TOP);
        sys::obs_sceneitem_set_pos(item, &ffi::vec2(0.0, 0.0));
        // A game running below the monitor's resolution still fills the frame.
        sys::obs_sceneitem_set_bounds_type(item, sys::obs_bounds_type_OBS_BOUNDS_SCALE_INNER);
        sys::obs_sceneitem_set_bounds_alignment(item, 0);
        sys::obs_sceneitem_set_bounds(item, &ffi::vec2(canvas.0, canvas.1));
        sys::obs_sceneitem_set_visible(item, true);
        log::info!(
            "[capture] source {} type={}",
            ffi::string(sys::obs_source_get_name(source)),
            ffi::string(sys::obs_source_get_id(source))
        );
        Item { source, item }
    }
}

fn set_active(item: Option<&Item>, active: bool) {
    if let Some(item) = item {
        unsafe {
            sys::obs_sceneitem_set_visible(item.item, active);
            sys::obs_source_set_enabled(item.source, active);
        }
    }
}

fn move_top(item: Option<&Item>) {
    if let Some(item) = item {
        unsafe { sys::obs_sceneitem_set_order(item.item, sys::obs_order_movement_OBS_ORDER_MOVE_TOP) };
    }
}

fn create_display_capture(display: &DisplayInfo, monitor_index: u32) -> Result<*mut sys::obs_source_t, String> {
    let source = ffi::source_create("monitor_capture", "Rikette Display", &json!({}))
        .ok_or("Could not create display capture")?;
    let mut settings = json!({ "capture_cursor": true, "method": WINDOWS_GRAPHICS_CAPTURE, "force_sdr": false });
    match pick_monitor_id(source, display, monitor_index) {
        Some(id) => settings["monitor_id"] = json!(id),
        None => settings["monitor"] = json!(monitor_index),
    }
    ffi::source_update(source, &settings);
    Ok(source)
}

/// OBS's id for the monitor: by name, then by resolution, then by position in its list.
fn pick_monitor_id(source: *mut sys::obs_source_t, display: &DisplayInfo, index: u32) -> Option<String> {
    let items: Vec<_> = ffi::list_items(source, "monitor_id")
        .into_iter()
        .filter(|item| !item.name.eq_ignore_ascii_case("auto") && !item.value.eq_ignore_ascii_case("auto"))
        .collect();
    if items.is_empty() {
        return None;
    }
    log::info!(
        "[capture] monitor_id options={}",
        items.iter().map(|i| format!("{}={}", i.name, i.value)).collect::<Vec<_>>().join(" | ")
    );
    let label = display.label.trim();
    if !label.is_empty() {
        if let Some(hit) = items.iter().find(|item| item.name.contains(label)) {
            return Some(hit.value.clone());
        }
    }
    let logical = format!("{}x{}", display.width, display.height);
    let physical = format!(
        "{}x{}",
        (display.width as f64 * display.scale_factor).round(),
        (display.height as f64 * display.scale_factor).round()
    );
    items
        .iter()
        .find(|item| item.name.contains(&logical) || item.name.contains(&physical))
        .or_else(|| items.get(index as usize))
        .or_else(|| items.first())
        .map(|item| item.value.clone())
}

/// The game capture `window` value: OBS's own entry when it lists the game, else
/// `title:class:exe` with OBS's escaping.
fn obs_window_id(source: *mut sys::obs_source_t, app: &FullscreenApp) -> String {
    let exe = app.exe.to_lowercase();
    if let Some(item) = ffi::list_items(source, "window")
        .into_iter()
        .find(|item| item.name.to_lowercase().contains(&exe) || item.value.to_lowercase().contains(&exe))
    {
        return item.value;
    }
    WindowInfo::new(&app.title, &app.class_name, &app.exe).id
}
