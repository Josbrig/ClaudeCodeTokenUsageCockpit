// SPDX-License-Identifier: Apache-2.0
//! Calculations on the usage of one window (concept §7). All functions are pure: they get the
//! time as a parameter and never read a clock.

use crate::model::WindowKind;
use crate::periods::Period;

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
