//! Start with OS. The login item passes `--autostart`, so those launches stay in the tray.

use anyhow::{bail, Result};
use tauri_plugin_autostart::ManagerExt;

use crate::events;

pub const AUTOSTART_ARG: &str = "--autostart";

pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_autostart::Builder::new()
        .args([AUTOSTART_ARG])
        .build()
}

pub fn apply(enabled: bool) -> Result<()> {
    // Debug builds share settings with the installed app; never point its login item here.
    if cfg!(debug_assertions) {
        bail!("Start with OS is only available in the packaged desktop app.");
    }
    let launcher = events::app().autolaunch();
    // Apply even if the saved value matches: Windows may have disabled the item.
    let result = if enabled { launcher.enable() } else { launcher.disable() };
    if let Err(error) = result {
        bail!("Could not update Start with OS: {error}");
    }
    if launcher.is_enabled().unwrap_or(!enabled) != enabled {
        bail!("Windows could not update Start with OS. Check your Startup apps settings.");
    }
    Ok(())
}

pub fn opened_at_login() -> bool {
    std::env::args().any(|arg| arg == AUTOSTART_ARG)
}
