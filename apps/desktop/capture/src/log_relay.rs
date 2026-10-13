//! Logs go to the app as protocol messages; the app writes them to stdout and main.log.

use capture_protocol::{FromHost, LogLevel};
use log::{Level, LevelFilter, Log, Metadata, Record};

struct Relay;

impl Log for Relay {
    fn enabled(&self, metadata: &Metadata) -> bool {
        // libobs-wrapper traces every call; keep its debug output out of main.log.
        metadata.level() <= Level::Info || metadata.target().starts_with("rikette_capture")
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let level = match record.level() {
            Level::Error => LogLevel::Error,
            Level::Warn => LogLevel::Warn,
            Level::Info => LogLevel::Info,
            Level::Debug | Level::Trace => LogLevel::Debug,
        };
        crate::send(&FromHost::Log { level, text: record.args().to_string() });
    }

    fn flush(&self) {}
}

static RELAY: Relay = Relay;

pub fn install() {
    if log::set_logger(&RELAY).is_ok() {
        log::set_max_level(LevelFilter::Debug);
    }
}
