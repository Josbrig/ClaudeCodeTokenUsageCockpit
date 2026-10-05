// SPDX-License-Identifier: Apache-2.0
//! Data model shared by the bridge and the cockpit (concept §4). The JSON field names are
//! the ones of concept §5.2.

use serde::{Deserialize, Serialize};

/// One of the two usage windows of a Claude subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WindowKind {
    /// The rolling 5-hour window.
    FiveHour,
    /// The weekly (7-day) window.
    SevenDay,
}

/// One window inside one record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowSample {
    /// Used share of the window in percent, as delivered (values above 100 are kept; the
    /// calculations clamp them).
    pub used_pct: f64,
    /// Reset time of the window, Unix seconds.
    pub resets_at: i64,
}

/// What the bridge stores for one status line update.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    /// Receive time of the bridge, Unix milliseconds.
    pub received_at_ms: i64,
    /// Claude Code session id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Claude Code version that delivered the record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cc_version: Option<String>,
    /// The 5-hour window, if delivered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub five_hour: Option<WindowSample>,
    /// The 7-day window, if delivered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seven_day: Option<WindowSample>,
    /// Display name of the active model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Context window usage in percent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_used_pct: Option<f64>,
    /// Estimated session cost in USD, as computed by Claude Code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
}

impl Record {
    /// The sample of the given window, if delivered.
    pub fn window(&self, kind: WindowKind) -> Option<&WindowSample> {
        match kind {
            WindowKind::FiveHour => self.five_hour.as_ref(),
            WindowKind::SevenDay => self.seven_day.as_ref(),
        }
    }
}
