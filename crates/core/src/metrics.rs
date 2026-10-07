// SPDX-License-Identifier: Apache-2.0
//! Calculations on the usage of one window (concept §7). All functions are pure: they get the
//! time as a parameter and never read a clock.

use crate::model::{Record, WindowKind};
use crate::periods::Period;
use crate::transcripts::Entry;

/// Length of the 5-hour window in seconds.
pub const FIVE_HOUR_S: i64 = 18_000;
/// Length of the 7-day window in seconds.
pub const SEVEN_DAY_S: i64 = 604_800;

/// Length of the given window in seconds.
pub const fn window_len_s(kind: WindowKind) -> i64 {
    match kind {
        WindowKind::FiveHour => FIVE_HOUR_S,
        WindowKind::SevenDay => SEVEN_DAY_S,
    }
}

/// How the usage compares with an even pace until the reset (REQ-003).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaceState {
    /// Using the quota slower than an even pace: part of it would expire unused.
    Under,
    /// Within the tolerance band around the even pace.
    On,
    /// Using the quota faster than an even pace: the limit would be reached before the reset.
    Over,
}

/// Values derived from the usage of one window at one moment (concept §7.1 and §7.2).
#[derive(Debug, Clone, PartialEq)]
pub struct Basic {
    /// Used share in percent, clamped to 0 to 100.
    pub used: f64,
    /// Remaining share in percent, `100 − used`.
    pub remaining: f64,
    /// Seconds until the reset, never negative.
    pub secs_to_reset: i64,
    /// Linear target: the percentage that an even pace would have used by now.
    pub target: f64,
    /// Actual minus target in percentage points.
    pub deviation_pp: f64,
    /// Actual divided by target; `None` while the target is 0.
    pub pace_factor: Option<f64>,
    /// Under, on or over pace, judged with the tolerance band.
    pub state: PaceState,
}

/// Computes the basic values of one window.
///
/// `used` is the delivered percentage (clamped to 0 to 100 here), `resets_at` and `now_s` are
/// Unix seconds, `window_len_s` the length of the window, `tol_pp` the tolerance band in
/// percentage points. The elapsed share of the window is the time since `resets_at −
/// window_len_s`, limited to 0 to 1.
pub fn basic(used: f64, resets_at: i64, window_len_s: i64, now_s: i64, tol_pp: f64) -> Basic {
    let used = used.clamp(0.0, 100.0);
    let secs_to_reset = (resets_at - now_s).max(0);
    let elapsed_s = now_s - (resets_at - window_len_s);
    let elapsed_share = (elapsed_s as f64 / window_len_s as f64).clamp(0.0, 1.0);
    let target = 100.0 * elapsed_share;
    let deviation_pp = used - target;
    let state = if deviation_pp < -tol_pp {
        PaceState::Under
    } else if deviation_pp > tol_pp {
        PaceState::Over
    } else {
        PaceState::On
    };
    Basic {
        used,
        remaining: 100.0 - used,
        secs_to_reset,
        target,
        deviation_pp,
        pace_factor: (target > 0.0).then(|| used / target),
        state,
    }
}

const MS_PER_HOUR: f64 = 3_600_000.0;

/// Current usage rate of a window in percent per hour (concept §7.3).
///
/// Uses the samples of `period` received within the last `rate_period_s` seconds before
/// `now_s` and fits a straight line through them by least squares; the rate is its slope.
/// Without two samples at distinct times the rate is not available (`None`). A falling usage
/// (a negative slope) is reported as 0.
pub fn rate(period: &Period, now_s: i64, rate_period_s: i64) -> Option<f64> {
    let cutoff_ms = now_s.saturating_sub(rate_period_s).saturating_mul(1000);
    let samples: Vec<(f64, f64)> = {
        let recent: Vec<&(i64, f64)> = period.samples.iter().filter(|s| s.0 >= cutoff_ms).collect();
        let first_ms = recent.iter().map(|s| s.0).min()?;
        recent
            .iter()
            .map(|&&(ms, pct)| ((ms - first_ms) as f64 / MS_PER_HOUR, pct))
            .collect()
    };
    let n = samples.len() as f64;
    let mean_t = samples.iter().map(|s| s.0).sum::<f64>() / n;
    let mean_u = samples.iter().map(|s| s.1).sum::<f64>() / n;
    let sxx: f64 = samples.iter().map(|s| (s.0 - mean_t).powi(2)).sum();
    if sxx <= 0.0 {
        return None; // all samples at the same time (or fewer than two)
    }
    let sxy: f64 = samples
        .iter()
        .map(|s| (s.0 - mean_t) * (s.1 - mean_u))
        .sum();
    let slope = sxy / sxx;
    slope.is_finite().then(|| slope.max(0.0))
}

/// What the current usage rate means for the end of the window (concept §7.4, REQ-005).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forecast {
    /// The limit is reached already.
    LimitReached,
    /// At the current rate the limit is reached at `at_s` (Unix seconds), before the reset.
    LimitFirst {
        /// Predicted time at which 100 % is reached.
        at_s: i64,
    },
    /// The reset comes first: the limit is not reached at the current rate.
    ResetFirst,
    /// There is no usage rate to base a forecast on.
    NotAvailable,
}

/// Forecast of the exhaustion time. Checked in this order: usage at 100 % (limit reached),
/// no rate (not available), rate 0 (reset first), otherwise the time at which 100 % would be
/// reached is compared with the reset; reaching 100 % exactly at the reset counts as reset
/// first.
pub fn forecast(used: f64, resets_at: i64, rate: Option<f64>, now_s: i64) -> Forecast {
    let used = used.clamp(0.0, 100.0);
    if used >= 100.0 {
        return Forecast::LimitReached;
    }
    // A rate that is not a finite number cannot be forecast from.
    let Some(rate) = rate.filter(|r| r.is_finite()) else {
        return Forecast::NotAvailable;
    };
    if rate <= 0.0 {
        return Forecast::ResetFirst;
    }
    let secs_to_limit = ((100.0 - used) / rate * 3600.0).round();
    let at_s = now_s.saturating_add(secs_to_limit as i64);
    if at_s < resets_at {
        Forecast::LimitFirst { at_s }
    } else {
        Forecast::ResetFirst
    }
}

/// Share of the quota (percent) that would remain unused at the reset if the current rate
/// continued (concept §7.5, REQ-006). `None` without a rate.
pub fn unused_at_reset(used: f64, resets_at: i64, rate: Option<f64>, now_s: i64) -> Option<f64> {
    let used = used.clamp(0.0, 100.0);
    let rate = rate.filter(|r| r.is_finite())?;
    let hours_left = (resets_at - now_s).max(0) as f64 / 3600.0;
    Some((100.0 - (used + rate * hours_left)).max(0.0))
}

/// Usage rate in percent per hour that would use up the remaining quota exactly at the reset
/// (concept §7.6, REQ-007). `None` once the reset time has been reached.
pub fn recommended_rate(used: f64, resets_at: i64, now_s: i64) -> Option<f64> {
    let used = used.clamp(0.0, 100.0);
    let secs_left = resets_at - now_s;
    (secs_left > 0).then(|| (100.0 - used) / (secs_left as f64 / 3600.0))
}

/// The window whose limit would be reached first (concept §7.7, REQ-008).
///
/// A window that has already reached its limit counts as exhausted now, a window with
/// `LimitFirst` as exhausted at its predicted time; a window where the reset comes first (or
/// that has no forecast) is never exhausted. The earlier exhaustion binds; if both are
/// exhausted at the same moment the 7-day window binds, because it holds you back longer.
/// Without a forecast for both windows there is nothing to compare, and if neither window
/// reaches its limit before its reset there is no binding limit.
pub fn binding(five: Option<&Forecast>, seven: Option<&Forecast>) -> Option<WindowKind> {
    fn exhausted_at(forecast: &Forecast) -> Option<i64> {
        match forecast {
            Forecast::LimitReached => Some(i64::MIN),
            Forecast::LimitFirst { at_s } => Some(*at_s),
            Forecast::ResetFirst | Forecast::NotAvailable => None,
        }
    }
    let five_at = exhausted_at(five?);
    let seven_at = exhausted_at(seven?);
    match (five_at, seven_at) {
        (Some(f), Some(s)) if f < s => Some(WindowKind::FiveHour),
        (Some(_), Some(_)) => Some(WindowKind::SevenDay),
        (Some(_), None) => Some(WindowKind::FiveHour),
        (None, Some(_)) => Some(WindowKind::SevenDay),
        (None, None) => None,
    }
}

/// Age of a record in whole seconds at `now_ms` (concept §7.9, REQ-009). A record that lies
/// in the future (clock difference) has age 0.
pub fn data_age_s(latest: &Record, now_ms: i64) -> i64 {
    (now_ms - latest.received_at_ms).max(0) / 1000
}

/// Whether the displayed data counts as stale (concept §7.9, REQ-009 and REQ-108).
///
/// Stale when the newest record is older than `stale_after_s` seconds, or when the newest
/// malformed input (`last_error_ms`) is more recent than the newest record `latest_ms`: the
/// last values are then kept but marked at once.
pub fn is_stale(
    age_s: i64,
    stale_after_s: u32,
    last_error_ms: Option<i64>,
    latest_ms: i64,
) -> bool {
    age_s > i64::from(stale_after_s) || last_error_ms.is_some_and(|error_ms| error_ms > latest_ms)
}

/// The most recently received record, whichever Claude Code session delivered it (REQ-017).
/// With equal receive times the later one in the list wins.
pub fn latest(records: &[Record]) -> Option<&Record> {
    records.iter().max_by_key(|r| r.received_at_ms)
}

/// Tokens that one percentage point of the window cost in this period (concept §7.10, REQ-015).
///
/// `tokens_in_period` are the tokens used since the first record of the period (see
/// [`tokens_in_period`]); they are divided by the rise of the used share from the first to the
/// last sample. `None` if the rise is below 1 percentage point or there were no tokens. The
/// result is an estimate and must be shown as such.
pub fn tokens_per_pp(period: &Period, tokens_in_period: u64) -> Option<f64> {
    let first = period.samples.first()?.1;
    let last = period.samples.last()?.1;
    let rise = last - first;
    (rise >= 1.0 && tokens_in_period > 0).then(|| tokens_in_period as f64 / rise)
}

/// Input, output, cache creation and cache read tokens of the entries written from the receive
/// time of the first sample of `period` until `now_ms` (both ends included); `0` for a period
/// without samples. The entries must not contain duplicates (see `transcripts::dedupe`).
pub fn tokens_in_period(entries: &[Entry], period: &Period, now_ms: i64) -> u64 {
    let Some(&(from_ms, _)) = period.samples.first() else {
        return 0;
    };
    entries
        .iter()
        .filter(|e| (from_ms..=now_ms).contains(&e.timestamp.timestamp_millis()))
        .fold(0u64, |sum, e| {
            let u = &e.usage;
            sum.saturating_add(u.input)
                .saturating_add(u.output)
                .saturating_add(u.cache_creation)
                .saturating_add(u.cache_read)
        })
}
