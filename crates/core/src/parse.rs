// SPDX-License-Identifier: Apache-2.0
//! Parsing of the status line JSON that Claude Code hands to the bridge (concept §4.1).
//!
//! The input is read as a generic JSON value so that missing or unexpected fields never
//! fail the parse. Only the fields the cockpit uses are taken; everything else is dropped
//! without being stored or logged.

use serde_json::Value;

use crate::model::{Record, WindowSample};

/// Why an input could not be turned into a record.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    /// The input is not JSON, or its top level is not an object. The message never contains
    /// parts of the input.
    #[error("input is not a JSON object: {0}")]
    InvalidJson(String),
}

/// Parses one status line update received at `received_at_ms` (Unix milliseconds).
pub fn parse_status_line(input: &str, received_at_ms: i64) -> Result<Record, ParseError> {
    let value: Value =
        serde_json::from_str(input).map_err(|e| ParseError::InvalidJson(e.to_string()))?;
    if !value.is_object() {
        return Err(ParseError::InvalidJson(
            "the top-level value is not an object".to_string(),
        ));
    }
    let rate_limits = value.get("rate_limits");
    Ok(Record {
        received_at_ms,
        session_id: text(value.get("session_id")),
        cc_version: text(value.get("version")),
        five_hour: rate_limits.and_then(|r| window(r.get("five_hour"))),
        seven_day: rate_limits.and_then(|r| window(r.get("seven_day"))),
        model: text(value.get("model").and_then(|m| m.get("display_name"))),
        context_used_pct: number(
            value
                .get("context_window")
                .and_then(|c| c.get("used_percentage")),
        ),
        cost_usd: number(value.get("cost").and_then(|c| c.get("total_cost_usd"))),
    })
}

fn text(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_owned)
}

fn number(value: Option<&Value>) -> Option<f64> {
    value.and_then(Value::as_f64).filter(|n| n.is_finite())
}

/// A window counts only with a finite `used_percentage` of at least 0 and an integer
/// `resets_at` (a float such as `1.7e9` is not an integer and drops the window).
fn window(value: Option<&Value>) -> Option<WindowSample> {
    let value = value?;
    let used_pct = number(value.get("used_percentage"))?;
    let resets_at = value.get("resets_at")?.as_i64()?;
    (used_pct >= 0.0).then_some(WindowSample {
        used_pct,
        resets_at,
    })
}
