//! Capture size for the chosen monitor and output resolution cap.

use capture_protocol::{Canvas, DisplayInfo};
use serde::Serialize;
use tauri::Monitor;

use crate::events;
use crate::settings::AppSettings;

#[derive(Debug, Clone, Serialize)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// One monitor as the settings page lists it.
#[derive(Debug, Clone, Serialize)]
pub struct ListedDisplay {
    pub id: u32,
    pub index: u32,
    pub label: String,
    pub bounds: Bounds,
    pub primary: bool,
}

fn monitors() -> Vec<Monitor> {
    events::app().available_monitors().unwrap_or_default()
}

fn primary() -> Option<Monitor> {
    events::app().primary_monitor().ok().flatten()
}

fn same(a: &Monitor, b: &Monitor) -> bool {
    a.position() == b.position() && a.size() == b.size()
}

fn label(monitor: &Monitor, index: usize) -> String {
    match monitor.name() {
        // Windows names monitors by device path; show something people recognise.
        Some(name) if !name.is_empty() && !name.starts_with(r"\\.\") => name.clone(),
        _ => format!("Display {}", index + 1),
    }
}

pub fn list_displays() -> Vec<ListedDisplay> {
    let primary = primary();
    monitors()
        .iter()
        .enumerate()
        .map(|(index, monitor)| {
            let scale = monitor.scale_factor();
            let position = monitor.position().to_logical::<f64>(scale);
            let size = monitor.size().to_logical::<f64>(scale);
            ListedDisplay {
                id: index as u32,
                index: index as u32,
                label: label(monitor, index),
                bounds: Bounds {
                    x: position.x.round() as i32,
                    y: position.y.round() as i32,
                    width: size.width.round() as u32,
                    height: size.height.round() as u32,
                },
                primary: primary.as_ref().is_some_and(|p| same(p, monitor)),
            }
        })
        .collect()
}

fn monitor_at(index: u32) -> Option<(Monitor, usize)> {
    let all = monitors();
    if let Some(monitor) = all.get(index as usize) {
        return Some((monitor.clone(), index as usize));
    }
    let primary = primary()?;
    let position = all.iter().position(|m| same(m, &primary)).unwrap_or(0);
    Some((primary, position))
}

pub fn display_info(index: u32) -> DisplayInfo {
    match monitor_at(index) {
        Some((monitor, position)) => {
            let scale = monitor.scale_factor();
            let size = monitor.size().to_logical::<f64>(scale);
            DisplayInfo {
                label: label(&monitor, position),
                width: size.width.round() as u32,
                height: size.height.round() as u32,
                scale_factor: scale,
            }
        }
        None => DisplayInfo { label: String::new(), width: 1920, height: 1080, scale_factor: 1.0 },
    }
}

pub fn for_settings(settings: &AppSettings) -> Canvas {
    let display = display_info(settings.monitor_index);
    compute(&display, settings.output_res.max_height())
}

fn compute(display: &DisplayInfo, cap: Option<u32>) -> Canvas {
    let base_width = even(display.width as f64 * display.scale_factor);
    let base_height = even(display.height as f64 * display.scale_factor);
    let scale = match cap {
        Some(cap) if base_height > cap => cap as f64 / base_height as f64,
        _ => 1.0,
    };
    Canvas {
        base_width,
        base_height,
        output_width: even(base_width as f64 * scale),
        output_height: even(base_height as f64 * scale),
    }
}

fn even(value: f64) -> u32 {
    ((value.round() as u32) & !1).max(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display(width: u32, height: u32, scale_factor: f64) -> DisplayInfo {
        DisplayInfo { label: String::new(), width, height, scale_factor }
    }

    #[test]
    fn native_keeps_physical_pixels() {
        let canvas = compute(&display(1280, 720, 1.5), None);
        assert_eq!((canvas.base_width, canvas.base_height), (1920, 1080));
        assert_eq!((canvas.output_width, canvas.output_height), (1920, 1080));
    }

    #[test]
    fn the_cap_scales_down_and_stays_even() {
        let canvas = compute(&display(3440, 1440, 1.0), Some(1080));
        assert_eq!((canvas.output_width, canvas.output_height), (2580, 1080));
        let canvas = compute(&display(1366, 768, 1.0), Some(720));
        assert_eq!(canvas.output_width % 2, 0);
        assert_eq!(canvas.output_height, 720);
    }

    #[test]
    fn a_cap_above_the_monitor_does_nothing() {
        let canvas = compute(&display(1920, 1080, 1.0), Some(1440));
        assert_eq!((canvas.output_width, canvas.output_height), (1920, 1080));
    }
}
