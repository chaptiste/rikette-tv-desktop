//! Which version ran last, so the page can show "What's new" once after an update.

use std::fs;
use std::path::Path;
use std::sync::Mutex;

use serde::Serialize;

use crate::paths;

#[derive(Debug, Clone, Serialize)]
pub struct VersionChange {
    /// Version that ran before this launch; None when it predates version tracking.
    pub from: Option<String>,
    pub to: String,
}

static PENDING: Mutex<Option<VersionChange>> = Mutex::new(None);

/// Call before `settings::load`: a missing settings file is how a first install is told
/// apart from an update.
pub fn track_installed_version() {
    *PENDING.lock().unwrap() = detect(&paths::data_dir(), &crate::version());
}

/// What changed since the last launch, and records `current` as the last version.
fn detect(data_dir: &Path, current: &str) -> Option<VersionChange> {
    let file = data_dir.join("last-version.json");
    let previous = fs::read_to_string(&file)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .map(|value| value["version"].as_str().map(str::to_string));
    let change = match &previous {
        Some(from) if from.as_deref() != Some(current) => {
            Some(VersionChange { from: from.clone(), to: current.to_string() })
        }
        // No version file yet. Settings on disk means the user updated from a build older
        // than this tracking; no settings means a fresh install, which gets no notes.
        None if data_dir.join("settings.json").exists() => {
            Some(VersionChange { from: None, to: current.to_string() })
        }
        _ => None,
    };
    if previous.flatten().as_deref() != Some(current) {
        let body = format!("{}\n", serde_json::json!({ "version": current }));
        if let Err(error) = fs::create_dir_all(data_dir).and_then(|_| fs::write(&file, body)) {
            log::warn!("[rikette] could not record version: {error}");
        }
    }
    change
}

/// The update that just happened, once per launch.
pub fn take_version_change() -> Option<VersionChange> {
    PENDING.lock().unwrap().take()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("rikette-notes-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn change(from: Option<&str>, to: &str) -> Option<(Option<String>, String)> {
        Some((from.map(str::to_string), to.to_string()))
    }

    fn pair(change: Option<VersionChange>) -> Option<(Option<String>, String)> {
        change.map(|c| (c.from, c.to))
    }

    #[test]
    fn a_fresh_install_shows_no_notes_then_an_update_shows_the_jump() {
        let dir = fresh();
        assert_eq!(pair(detect(&dir, "0.1.23")), None);
        fs::write(dir.join("settings.json"), "{}").unwrap();
        assert_eq!(pair(detect(&dir, "0.1.23")), None);
        assert_eq!(pair(detect(&dir, "0.1.25")), change(Some("0.1.23"), "0.1.25"));
    }

    #[test]
    fn updating_from_a_build_older_than_version_tracking_shows_the_new_version() {
        let dir = fresh();
        fs::write(dir.join("settings.json"), "{}").unwrap();
        assert_eq!(pair(detect(&dir, "0.1.23")), change(None, "0.1.23"));
    }

    #[test]
    fn the_change_is_handed_out_once_per_launch() {
        let dir = fresh();
        fs::write(dir.join("settings.json"), "{}").unwrap();
        *PENDING.lock().unwrap() = detect(&dir, "0.1.23");
        assert!(take_version_change().is_some());
        assert!(take_version_change().is_none());
    }
}
