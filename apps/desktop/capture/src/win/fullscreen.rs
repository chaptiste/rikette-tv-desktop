//! Finds the fullscreen game to hook: the foreground window when it covers its monitor, or
//! failing that the largest window of a known game engine that does.

use std::collections::HashSet;
use std::sync::OnceLock;

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HWND, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetForegroundWindow, GetTopWindow, GetWindow, GetWindowLongW, GetWindowRect,
    GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, IsZoomed, GWL_EXSTYLE, GWL_STYLE,
    GW_HWNDNEXT, WS_CAPTION, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FullscreenApp {
    pub exe: String,
    pub title: String,
    pub class_name: String,
}

/// Apps that go fullscreen but are not games, plus Rikette and OBS themselves.
const BLOCKED_EXES: &[&str] = &[
    "explorer.exe", "dwm.exe", "shellexperiencehost.exe", "searchhost.exe", "searchapp.exe",
    "startmenuexperiencehost.exe", "textinputhost.exe", "applicationframehost.exe",
    "systemsettings.exe", "lockapp.exe", "logonui.exe", "shellhost.exe", "pickerhost.exe",
    "nvidia share.exe", "nvidia overlay.exe", "gamebar.exe", "gamebarftw.exe", "xboxpcappft.exe",
    "rikette.exe", "rikette-capture.exe", "echoframe.exe", "electron.exe", "obs64.exe", "obs32.exe",
    "cursor.exe", "code.exe", "revidi.exe", "discord.exe", "discordcanary.exe", "discordptb.exe",
    "slack.exe", "telegram.exe", "whatsapp.exe", "ms-teams.exe", "teams.exe", "spotify.exe",
    "chrome.exe", "msedge.exe", "msedgewebview2.exe", "firefox.exe", "brave.exe", "opera.exe",
    "vivaldi.exe", "steam.exe", "steamwebhelper.exe", "epicgameslauncher.exe", "epicwebhelper.exe",
    "widgets.exe", "widgetservice.exe", "taskmgr.exe", "windowsterminal.exe", "powershell.exe",
    "pwsh.exe", "cmd.exe", "vlc.exe", "mpv.exe", "mpvnet.exe", "mpc-hc.exe", "mpc-hc64.exe",
    "mpc-be.exe", "mpc-be64.exe", "potplayer.exe", "potplayer64.exe", "potplayermini.exe",
    "potplayermini64.exe", "wmplayer.exe", "microsoft.media.player.exe", "video.ui.exe",
    "photos.exe", "microsoft.photos.exe", "itunes.exe", "applemusic.exe", "snippingtool.exe",
    "screenclippinghost.exe", "screensketch.exe", "mspaint.exe", "paint.exe", "notepad.exe",
    "wordpad.exe", "calc.exe", "calculator.exe", "clipchamp.exe", "winword.exe", "excel.exe",
    "powerpnt.exe", "outlook.exe", "onenote.exe", "zoom.exe", "skype.exe", "dllhost.exe",
    "rundll32.exe", "mmc.exe", "control.exe",
];

/// Window classes of engines whose windows are always games.
const GAME_CLASSES: &[&str] = &[
    "UnrealWindow", "UnityWndClass", "GLFW30", "SDL_app", "RiotWindowClass", "Valve001",
    "TankWindowClass", "CryENGINE", "grcWindow", "RAGE", "LaunchUnrealUWPWindow", "RenderWindow",
    "OgreD3D9Wnd", "OgreD3D11Wnd", "LWJGL",
];

fn blocked() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| BLOCKED_EXES.iter().copied().collect())
}

fn is_game_class(class: &str) -> bool {
    GAME_CLASSES.iter().any(|c| c.eq_ignore_ascii_case(class))
}

pub fn is_blocked(exe: &str) -> bool {
    blocked().contains(exe.trim().to_lowercase().as_str())
}

pub fn current() -> Option<FullscreenApp> {
    let own = unsafe { GetCurrentProcessId() };
    let foreground = unsafe { GetForegroundWindow() };
    if let Some((app, _)) = try_game(foreground, own, true) {
        return Some(app);
    }
    let mut best: Option<(FullscreenApp, i64)> = None;
    let mut hwnd = unsafe { GetTopWindow(None) }.ok();
    while let Some(window) = hwnd.filter(|h| !h.is_invalid()) {
        if window != foreground {
            if let Some((app, area)) = try_game(window, own, false) {
                if best.as_ref().is_none_or(|(_, a)| area >= *a) {
                    best = Some((app, area));
                }
            }
        }
        hwnd = unsafe { GetWindow(window, GW_HWNDNEXT) }.ok();
    }
    best.map(|(app, _)| app)
}

pub(super) fn wide(buf: &[u16], len: i32) -> String {
    String::from_utf16_lossy(&buf[..len.max(0) as usize])
}

pub(super) fn clean(text: &str) -> String {
    text.replace(['\t', '\n'], " ").replace('\r', "").trim().to_string()
}

/// A game window and its area, when `hwnd` looks like one.
fn try_game(hwnd: HWND, own_pid: u32, unknown_ok: bool) -> Option<(FullscreenApp, i64)> {
    unsafe {
        if hwnd.is_invalid() || !IsWindowVisible(hwnd).as_bool() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 || pid == own_pid {
            return None;
        }
        let exe = exe_name(pid)?;
        if is_blocked(&exe) {
            return None;
        }
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        if ex_style & WS_EX_TOOLWINDOW.0 != 0 || ex_style & WS_EX_NOACTIVATE.0 != 0 {
            return None;
        }
        let mut class = [0u16; 256];
        let class_len = GetClassNameW(hwnd, &mut class);
        let class_name = wide(&class, class_len);
        let lower = exe.to_lowercase();
        let engine = is_game_class(&class_name) || lower.ends_with("-shipping.exe") || lower.contains("win64");
        if !engine && cloaked(hwnd) {
            return None;
        }
        let mut rect = RECT::default();
        GetWindowRect(hwnd, &mut rect).ok()?;
        let (width, height) = ((rect.right - rect.left).max(0), (rect.bottom - rect.top).max(0));
        if width < 800 || height < 500 {
            return None;
        }
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if monitor.is_invalid() || !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return None;
        }
        let covers = |area: &RECT| {
            const SLACK: i32 = 8;
            rect.left <= area.left + SLACK
                && rect.top <= area.top + SLACK
                && rect.right >= area.right - SLACK
                && rect.bottom >= area.bottom - SLACK
        };
        let covers_monitor = covers(&info.rcMonitor);
        let covers_work = covers(&info.rcWork);
        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
        let caption = style & WS_CAPTION.0 == WS_CAPTION.0;
        let ok = if engine {
            covers_monitor || covers_work || IsZoomed(hwnd).as_bool()
        } else {
            unknown_ok && covers_monitor && !caption
        };
        if !ok {
            return None;
        }
        let mut title = [0u16; 512];
        let title_len = GetWindowTextW(hwnd, &mut title);
        Some((
            FullscreenApp { exe, title: clean(&wide(&title, title_len)), class_name },
            width as i64 * height as i64,
        ))
    }
}

pub(super) fn cloaked(hwnd: HWND) -> bool {
    let mut value = 0i32;
    let result = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut value as *mut i32 as *mut _,
            std::mem::size_of::<i32>() as u32,
        )
    };
    result.is_ok() && value != 0
}

/// `game.exe` for a process id, or None when it cannot be opened.
pub(super) fn exe_name(pid: u32) -> Option<String> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let result = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(process);
        result.ok()?;
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit(['\\', '/']).next().map(str::to_string).filter(|s| !s.is_empty())
    }
}

/// Whether any process runs this executable (`game.exe`).
pub fn exe_is_running(exe: &str) -> bool {
    let wanted = exe.rsplit(['\\', '/']).next().unwrap_or(exe).to_lowercase();
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            // Unknown counts as running, so a game is not released on a failed check.
            return true;
        };
        let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut found = false;
        let mut more = Process32FirstW(snapshot, &mut entry).is_ok();
        while more {
            let len = entry.szExeFile.iter().position(|c| *c == 0).unwrap_or(entry.szExeFile.len());
            if String::from_utf16_lossy(&entry.szExeFile[..len]).to_lowercase() == wanted {
                found = true;
                break;
            }
            more = Process32NextW(snapshot, &mut entry).is_ok();
        }
        let _ = CloseHandle(snapshot);
        found
    }
}
