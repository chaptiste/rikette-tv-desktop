//! App logs go to stdout and to `<data dir>/logs/main.log`, rolled to `main.old.log` past 5 MB.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::sync::Mutex;

use log::{Level, LevelFilter, Log, Metadata, Record};

use crate::paths;

const MAX_BYTES: u64 = 5 * 1024 * 1024;

struct Logger {
    file: Mutex<Option<File>>,
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info || metadata.target().starts_with("rikette")
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!(
            "{} {} {}",
            chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            record.level(),
            record.args()
        );
        println!("{line}");
        if let Ok(mut file) = self.file.lock() {
            if let Some(file) = file.as_mut() {
                let _ = writeln!(file, "{line}");
            }
        }
    }

    fn flush(&self) {
        if let Ok(mut file) = self.file.lock() {
            if let Some(file) = file.as_mut() {
                let _ = file.flush();
            }
        }
    }
}

static LOGGER: Logger = Logger { file: Mutex::new(None) };

pub fn install() {
    *LOGGER.file.lock().unwrap() = open_log_file();
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(LevelFilter::Debug);
    }
}

/// Marks the start of a run in main.log.
pub fn header(version: &str) {
    if let Some(file) = LOGGER.file.lock().unwrap().as_mut() {
        let _ = writeln!(
            file,
            "\n=== {} {} {version} ===",
            chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            crate::brand::APP_NAME
        );
    }
}

fn open_log_file() -> Option<File> {
    let dir = paths::logs_dir();
    fs::create_dir_all(&dir).ok()?;
    let path = dir.join("main.log");
    if fs::metadata(&path).map(|meta| meta.len() > MAX_BYTES).unwrap_or(false) {
        let _ = fs::rename(&path, dir.join("main.old.log"));
    }
    OpenOptions::new().create(true).append(true).open(path).ok()
}
