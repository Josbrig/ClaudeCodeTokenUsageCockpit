// SPDX-License-Identifier: Apache-2.0
//! Display texts of the cockpit (concept §11.5). Everything is plain functions so that the
//! view model can be tested without a window.
//!
//! Special characters used: `−` (minus sign, U+2212), `×` (U+00D7), `–` (en dash, U+2013) for
//! "no value" and `·` (middle dot, U+00B7).

use std::fmt::Display;

use chrono::{DateTime, TimeZone};

use crate::metrics::Forecast;

/// Text for a value that does not exist.
const NO_VALUE: &str = "–";
const NOT_AVAILABLE: &str = "not available";

/// `value` with `decimals` decimals, rounded half away from zero (`f64::round`), without a
/// minus sign in front of a zero. Not for non-finite values.
fn fixed(value: f64, decimals: u32) -> String {
    let scale = 10f64.powi(decimals as i32);
    let rounded = (value.abs() * scale).round() / scale;
    let text = format!("{rounded:.prec$}", prec = decimals as usize);
    if value < 0.0 && rounded > 0.0 {
        format!("-{text}")
    } else {
        text
    }
}

/// A percentage with one decimal: `4.5%`, `62.0%`. Values that are not finite show as `–`.
pub fn pct(v: f64) -> String {
    if v.is_finite() {
        format!("{}%", fixed(v, 1))
    } else {
        NO_VALUE.to_owned()
    }
}

/// A span of time: `42m`, `1h 12m`, `3d 4h`; the smaller unit is rounded down; negative
/// spans show as `0m`. Compute spans from Unix timestamps, never from local clock fields.
pub fn duration(secs: i64) -> String {
    let secs = secs.max(0);
    let (days, hours, minutes) = (secs / 86_400, secs % 86_400 / 3_600, secs % 3_600 / 60);
    if secs < 3_600 {
        format!("{minutes}m")
    } else if secs < 86_400 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{days}d {hours}h")
    }
}

/// Clock time of a Unix timestamp in the given time zone: `Mon 14:30`. A timestamp outside
/// the supported range shows as `–`.
pub fn local_time<Tz: TimeZone>(ts: i64, tz: &Tz) -> String
where
    Tz::Offset: Display,
{
    match DateTime::from_timestamp(ts, 0) {
        Some(utc) => utc.with_timezone(tz).format("%a %H:%M").to_string(),
        None => NO_VALUE.to_owned(),
    }
}

/// `12 s`, `4 min`, or the duration text from one hour on.
fn span(secs: i64) -> String {
    let secs = secs.max(0);
    if secs < 60 {
        format!("{secs} s")
    } else if secs < 3_600 {
        format!("{} min", secs / 60)
    } else {
        duration(secs)
    }
}

/// Age of the displayed data: `updated 12 s ago`, `updated 4 min ago`, `updated 2h 5m ago`.
pub fn age(secs: i64) -> String {
    format!("updated {} ago", span(secs))
}

/// Marker text for stale data: `stale, 14 min old` (same units as [`age`]).
pub fn stale(secs: i64) -> String {
    format!("stale, {} old", span(secs))
}

/// Deviation from the even pace: `+20.0 pp`, `−3.5 pp`; zero (after rounding) has no sign.
pub fn deviation(pp: f64) -> String {
    if !pp.is_finite() {
        return NO_VALUE.to_owned();
    }
    let size = fixed(pp.abs(), 1);
    if size == "0.0" {
        format!("{size} pp")
    } else if pp > 0.0 {
        format!("+{size} pp")
    } else {
        format!("−{size} pp")
    }
}

/// Pace factor with two decimals: `1.50×`; `–` where it is not defined.
pub fn factor(value: Option<f64>) -> String {
    match value.filter(|v| v.is_finite()) {
        Some(v) => format!("{}×", fixed(v, 2)),
        None => NO_VALUE.to_owned(),
    }
}

/// A rate in percent per hour: `20.0 %/h`, or `not available`.
pub fn rate(value: Option<f64>) -> String {
    match value.filter(|v| v.is_finite()) {
        Some(v) => format!("{} %/h", fixed(v, 1)),
        None => NOT_AVAILABLE.to_owned(),
    }
}

/// Projected unused remainder: `40.0% unused at reset`, or `not available`.
pub fn unused(value: Option<f64>) -> String {
    match value.filter(|v| v.is_finite()) {
        Some(v) => format!("{}% unused at reset", fixed(v, 1)),
        None => NOT_AVAILABLE.to_owned(),
    }
}

/// Weekly plan: `10 windows left · 6.0% per window` (`1 window left` for one).
pub fn weekly(windows: u32, share_per_window_pct: f64) -> String {
    let noun = if windows == 1 { "window" } else { "windows" };
    format!(
        "{windows} {noun} left · {} per window",
        pct(share_per_window_pct)
    )
}

/// Text for the exhaustion forecast at time `now_s`: `limit in 3h 0m, before reset`,
/// `reset first`, `limit reached` or `not available`.
pub fn forecast(forecast: &Forecast, now_s: i64) -> String {
    match forecast {
        Forecast::LimitReached => "limit reached".to_owned(),
        Forecast::LimitFirst { at_s } => {
            format!("limit in {}, before reset", duration(at_s - now_s))
        }
        Forecast::ResetFirst => "reset first".to_owned(),
        Forecast::NotAvailable => NOT_AVAILABLE.to_owned(),
    }
}
