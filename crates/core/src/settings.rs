// SPDX-License-Identifier: Apache-2.0
//! The user settings file `settings.toml` (concept §9).
//!
//! Loading never fails: a missing file gives the defaults, a file that is not valid TOML is
//! renamed to `settings.toml.invalid` and the defaults are used, and a single value that is
//! missing, of the wrong type or out of range falls back to its default while the other
//! values are kept.

use std::fs;
use std::io;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use toml::{Table, Value};

use crate::store::write_atomic;

/// File format version written to the settings file.
pub const FORMAT_VERSION: u32 = 1;
/// Allowed tolerance band around the even pace, in percentage points (REQ-003).
pub const TOLERANCE_RANGE: RangeInclusive<f64> = 0.0..=50.0;
/// Allowed age in seconds after which data counts as stale (REQ-009).
pub const STALE_AFTER_RANGE: RangeInclusive<u32> = 60..=86_400;
/// Allowed length in seconds of the period the usage rate is computed over (REQ-021).
pub const RATE_PERIOD_RANGE: RangeInclusive<u32> = 300..=7_200;
/// Smallest accepted window width in logical pixels.
pub const MIN_WINDOW_WIDTH: f64 = 100.0;
/// Smallest accepted window height in logical pixels.
pub const MIN_WINDOW_HEIGHT: f64 = 60.0;

/// Which view the cockpit opens with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StartView {
    /// The small always-visible view.
    Compact,
    /// The larger view with all statistics.
    Detailed,
}

/// Position and size of the cockpit window in logical pixels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowSettings {
    /// Left edge (may be negative on multi-monitor setups).
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

impl Default for WindowSettings {
    fn default() -> Self {
        Self {
            x: 100.0,
            y: 100.0,
            width: 320.0,
            height: 120.0,
        }
    }
}

/// All user settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// Tolerance band for the pace state, percentage points.
    pub tolerance_pp: f64,
    /// Seconds after which data counts as stale.
    pub stale_after_s: u32,
    /// Seconds over which the usage rate is computed.
    pub rate_period_s: u32,
    /// Keep the window on top of other windows.
    pub always_on_top: bool,
    /// View shown at start.
    pub start_view: StartView,
    /// Window position and size.
    pub window: WindowSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            tolerance_pp: 5.0,
            stale_after_s: 600,
            rate_period_s: 1_800,
            always_on_top: true,
            start_view: StartView::Compact,
            window: WindowSettings::default(),
        }
    }
}

/// What is written to disk: the settings plus the format version.
#[derive(Serialize)]
struct FileOut<'a> {
    version: u32,
    #[serde(flatten)]
    settings: &'a Settings,
}

/// Writes the settings atomically (temp file, then rename), creating the directory if needed.
pub fn save(path: &Path, settings: &Settings) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let text = toml::to_string_pretty(&FileOut {
        version: FORMAT_VERSION,
        settings,
    })
    .map_err(io::Error::other)?;
    write_atomic(path, text.as_bytes())
}

/// Loads the settings; never fails (see the module documentation).
pub fn load(path: &Path) -> Settings {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Settings::default(),
        Err(e) => {
            log::warn!(
                "settings: cannot read {}: {e}; using defaults",
                path.display()
            );
            return Settings::default();
        }
    };
    // Some Windows editors put a byte order mark at the start of a UTF-8 file.
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    let table = std::str::from_utf8(bytes)
        .ok()
        .and_then(|text| text.parse::<Table>().ok());
    let Some(table) = table else {
        match quarantine(path) {
            Some(moved) => log::warn!(
                "settings: {} is not valid TOML; using defaults (kept as {})",
                path.display(),
                moved.display()
            ),
            None => log::warn!(
                "settings: {} is not valid TOML and could not be moved away; using defaults",
                path.display()
            ),
        }
        return Settings::default();
    };
    from_table(&table)
}

/// Renames an invalid settings file to `<name>.invalid`, replacing an older one. Returns the
/// new path, or `None` if the file could not be moved.
fn quarantine(path: &Path) -> Option<PathBuf> {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".invalid");
    let target = path.with_file_name(name);
    let _ = fs::remove_file(&target);
    fs::rename(path, &target).ok().map(|()| target)
}

fn from_table(table: &Table) -> Settings {
    let defaults = Settings::default();
    Settings {
        tolerance_pp: float_in(
            table,
            "tolerance_pp",
            &TOLERANCE_RANGE,
            defaults.tolerance_pp,
        ),
        stale_after_s: int_in(
            table,
            "stale_after_s",
            &STALE_AFTER_RANGE,
            defaults.stale_after_s,
        ),
        rate_period_s: int_in(
            table,
            "rate_period_s",
            &RATE_PERIOD_RANGE,
            defaults.rate_period_s,
        ),
        always_on_top: bool_value(table, "always_on_top", defaults.always_on_top),
        start_view: start_view(table, defaults.start_view),
        window: window(table.get("window"), &defaults.window),
    }
}

fn warn_default(key: &str) {
    log::warn!(
        "settings: value of '{key}' is missing, of the wrong type or out of range; using the default"
    );
}

/// A number given as TOML float or integer.
fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Float(f) if f.is_finite() => Some(*f),
        Value::Integer(i) => Some(*i as f64),
        _ => None,
    }
}

fn float_in(table: &Table, key: &str, range: &RangeInclusive<f64>, default: f64) -> f64 {
    match table.get(key) {
        None => default,
        Some(v) => match number(v).filter(|n| range.contains(n)) {
            Some(n) => n,
            None => {
                warn_default(key);
                default
            }
        },
    }
}

fn int_in(table: &Table, key: &str, range: &RangeInclusive<u32>, default: u32) -> u32 {
    match table.get(key) {
        None => default,
        Some(Value::Integer(i)) => match u32::try_from(*i).ok().filter(|n| range.contains(n)) {
            Some(n) => n,
            None => {
                warn_default(key);
                default
            }
        },
        Some(_) => {
            warn_default(key);
            default
        }
    }
}

fn bool_value(table: &Table, key: &str, default: bool) -> bool {
    match table.get(key) {
        None => default,
        Some(Value::Boolean(b)) => *b,
        Some(_) => {
            warn_default(key);
            default
        }
    }
}

fn start_view(table: &Table, default: StartView) -> StartView {
    match table.get("start_view") {
        None => default,
        Some(Value::String(s)) if s == "compact" => StartView::Compact,
        Some(Value::String(s)) if s == "detailed" => StartView::Detailed,
        Some(_) => {
            warn_default("start_view");
            default
        }
    }
}

fn window(value: Option<&Value>, defaults: &WindowSettings) -> WindowSettings {
    let Some(Value::Table(t)) = value else {
        if value.is_some() {
            warn_default("window");
        }
        return defaults.clone();
    };
    let any = f64::NEG_INFINITY..=f64::INFINITY;
    let wide = MIN_WINDOW_WIDTH..=100_000.0;
    let tall = MIN_WINDOW_HEIGHT..=100_000.0;
    WindowSettings {
        x: float_in(t, "x", &any, defaults.x),
        y: float_in(t, "y", &any, defaults.y),
        width: float_in(t, "width", &wide, defaults.width),
        height: float_in(t, "height", &tall, defaults.height),
    }
}
