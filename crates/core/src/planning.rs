// SPDX-License-Identifier: Apache-2.0
//! Planning across windows (concept §7.8): how the weekly quota spreads over the 5-hour
//! windows that are left until the weekly reset.

use crate::metrics::FIVE_HOUR_S;

/// How the rest of the weekly quota fits into the remaining 5-hour windows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeeklyPlan {
    /// Number of 5-hour windows left until the weekly reset; a started window counts.
    pub windows_left: u32,
    /// Share of the weekly quota (percent) that each remaining window can use to run the
    /// weekly quota down exactly at its reset.
    pub share_per_window_pct: f64,
}

/// The weekly plan; `None` once the weekly reset has been reached.
pub fn weekly(used7: f64, resets_at7: i64, now_s: i64) -> Option<WeeklyPlan> {
    // Positive here, so the unsigned integer division that rounds up (stable) can be used.
    let secs_left = u64::try_from(resets_at7.checked_sub(now_s)?)
        .ok()
        .filter(|s| *s > 0)?;
    let windows = secs_left.div_ceil(FIVE_HOUR_S.unsigned_abs());
    let remaining = 100.0 - used7.clamp(0.0, 100.0);
    Some(WeeklyPlan {
        windows_left: u32::try_from(windows).unwrap_or(u32::MAX),
        share_per_window_pct: remaining / windows as f64,
    })
}
