// SPDX-License-Identifier: Apache-2.0
//! Grouping of records into window periods (concept §6, REQ-022).
//!
//! A period is one run of a usage window from one reset to the next. Values from different
//! periods must never be combined in one calculation, so every calculation works on a single
//! [`Period`]. Records are expected in the order they were received.

use crate::model::{Record, WindowKind};

/// How far the reported reset time of a window may move between two records of the same
/// period, in seconds (a proposal of the requirements; small shifts are jitter).
pub const RESET_TOLERANCE_S: i64 = 600;

/// The records of one window between two resets.
#[derive(Debug, Clone, PartialEq)]
pub struct Period {
    /// Which window this is.
    pub kind: WindowKind,
    /// Reset time of the period, Unix seconds: the `resets_at` of its latest record.
    pub resets_at: i64,
    /// `(received_at_ms, used_pct)` per record, in received order.
    pub samples: Vec<(i64, f64)>,
}

/// Splits the records into the periods of one window, in received order.
///
/// A record continues the current period if its `resets_at` is at most [`RESET_TOLERANCE_S`]
/// away (in either direction) from the period's latest `resets_at` and it was received before
/// that reset; otherwise it starts a new period. Records without the window are skipped.
pub fn split(records: &[Record], kind: WindowKind) -> Vec<Period> {
    let mut periods: Vec<Period> = Vec::new();
    for record in records {
        let Some(window) = record.window(kind) else {
            continue;
        };
        let received_s = record.received_at_ms.div_euclid(1000);
        let sample = (record.received_at_ms, window.used_pct);
        match periods.last_mut() {
            Some(period)
                if (window.resets_at - period.resets_at).abs() <= RESET_TOLERANCE_S
                    && received_s < period.resets_at =>
            {
                period.resets_at = window.resets_at;
                period.samples.push(sample);
            }
            _ => periods.push(Period {
                kind,
                resets_at: window.resets_at,
                samples: vec![sample],
            }),
        }
    }
    periods
}

/// The current period: the latest one, as long as its reset lies in the future. Without
/// one, the window has reset and the cockpit waits for new data.
pub fn current(periods: &[Period], now_s: i64) -> Option<&Period> {
    // Periods are in received order, so the last one is the only candidate.
    periods.last().filter(|p| p.resets_at > now_s)
}
