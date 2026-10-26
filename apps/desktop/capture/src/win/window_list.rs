//! The windows window mode can record: what Alt+Tab would show, front to back.

use capture_protocol::WindowInfo;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetTopWindow, GetWindow, GetWindowLongW, GetWindowTextW, GetWindowThreadProcessId,
    IsWindowVisible, GWL_EXSTYLE, GW_HWNDNEXT, GW_OWNER, WS_EX_TOOLWINDOW,
};

use super::fullscreen::{clean, cloaked, exe_name, wide};

/// The desktop and taskbar, and Rikette itself.
const SKIPPED_CLASSES: &[&str] = &["Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd"];
const SKIPPED_EXES: &[&str] = &["rikette.exe", "rikette-capture.exe", "textinputhost.exe"];

pub fn list() -> Vec<WindowInfo> {
    let own = unsafe { GetCurrentProcessId() };
    let mut found: Vec<WindowInfo> = Vec::new();
    let mut hwnd = unsafe { GetTopWindow(None) }.ok();
    while let Some(window) = hwnd.filter(|h| !h.is_invalid()) {
        if let Some(info) = pickable(window, own) {
            if !found.iter().any(|w| w.id == info.id) {
                found.push(info);
            }
        }
        hwnd = unsafe { GetWindow(window, GW_HWNDNEXT) }.ok();
    }
    found
}

fn pickable(hwnd: HWND, own_pid: u32) -> Option<WindowInfo> {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() || cloaked(hwnd) {
            return None;
        }
        if GetWindow(hwnd, GW_OWNER).is_ok_and(|owner| !owner.is_invalid()) {
            return None;
        }
        if GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 != 0 {
            return None;
        }
        let mut title = [0u16; 512];
        let title_len = GetWindowTextW(hwnd, &mut title);
        let title = clean(&wide(&title, title_len));
        if title.is_empty() {
            return None;
        }
        let mut class = [0u16; 256];
        let class_len = GetClassNameW(hwnd, &mut class);
        let class = wide(&class, class_len);
        if SKIPPED_CLASSES.contains(&class.as_str()) {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 || pid == own_pid {
            return None;
        }
        let exe = exe_name(pid)?;
        if SKIPPED_EXES.iter().any(|skip| skip.eq_ignore_ascii_case(&exe)) {
            return None;
        }
        Some(WindowInfo::new(&title, &class, &exe))
    }
}
