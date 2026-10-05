// SPDX-License-Identifier: Apache-2.0
//! Calculations on the usage of one window (concept §7). All functions are pure: they get the
//! time as a parameter and never read a clock.

use crate::model::WindowKind;

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
