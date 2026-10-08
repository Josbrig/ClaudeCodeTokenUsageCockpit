// SPDX-License-Identifier: Apache-2.0

use cockpit_core::model::{Record, WindowSample};
use cockpit_core::parse::{ParseError, parse_status_line};

const BOTH: &str = include_str!("fixtures/statusline/both_windows.json");
const FIVE_ONLY: &str = include_str!("fixtures/statusline/five_hour_only.json");
const NO_LIMITS: &str = include_str!("fixtures/statusline/no_rate_limits.json");
const EXTRA: &str = include_str!("fixtures/statusline/extra_fields.json");
const INVALID: &str = include_str!("fixtures/statusline/invalid.json");
const NEGATIVE: &str = include_str!("fixtures/statusline/negative_percent.json");

const NOW_MS: i64 = 1_738_400_000_000;

fn sample(used_pct: f64, resets_at: i64) -> Option<WindowSample> {
    Some(WindowSample {
        used_pct,
        resets_at,
    })
}

fn parse_ok(input: &str) -> Record {
    parse_status_line(input, NOW_MS).expect("input should parse")
}

#[test]
fn req_011_parse_both_windows() {
    let r = parse_ok(BOTH);
    assert_eq!(r.received_at_ms, NOW_MS);
    assert_eq!(r.five_hour, sample(23.5, 1_738_425_600));
    assert_eq!(r.seven_day, sample(41.2, 1_738_857_600));
    assert_eq!(r.session_id.as_deref(), Some("test-session-0001"));
    assert_eq!(r.cc_version.as_deref(), Some("2.1.90"));
    assert_eq!(r.model.as_deref(), Some("Test Model"));
    assert_eq!(r.context_used_pct, Some(8.0));
    assert_eq!(r.cost_usd, Some(0.01234));
}

#[test]
fn req_011_parse_one_window() {
    let r = parse_ok(FIVE_ONLY);
    assert_eq!(r.five_hour, sample(12.0, 1_738_425_600));
    assert_eq!(r.seven_day, None);
}

#[test]
fn req_011_parse_without_rate_limits() {
    let r = parse_ok(NO_LIMITS);
    assert_eq!(r.five_hour, None);
    assert_eq!(r.seven_day, None);
    assert_eq!(r.model.as_deref(), Some("Test Model"));
    assert_eq!(r.context_used_pct, None, "null is no value");
    assert_eq!(r.cost_usd, Some(0.5));
}

#[test]
fn req_011_ignores_unknown_fields() {
    let r = parse_ok(EXTRA);
    let expected = parse_ok(BOTH);
    assert_eq!(r.five_hour, expected.five_hour);
    assert_eq!(r.seven_day, expected.seven_day);
    assert_eq!(r.model, expected.model);
    assert_eq!(r.cost_usd, expected.cost_usd);
    // The gateway spend limit is not a window and is not stored.
    let json = serde_json::to_string(&r).unwrap();
    assert!(!json.contains("spend"), "{json}");
    assert!(!json.contains("synthetic"), "{json}");
}

#[test]
fn req_108_invalid_json_is_error() {
    let err = parse_status_line(INVALID, NOW_MS).unwrap_err();
    assert!(matches!(err, ParseError::InvalidJson(_)));
    assert!(!err.to_string().contains("this is not json"), "{err}");
}

#[test]
fn req_108_non_object_is_error() {
    for input in ["[]", "5", "\"text\"", "null", ""] {
        assert!(
            parse_status_line(input, NOW_MS).is_err(),
            "input {input:?} should be an error"
        );
    }
}

#[test]
fn req_011_negative_percent_drops_window() {
    let r = parse_ok(NEGATIVE);
    assert_eq!(r.five_hour, None);
    assert_eq!(r.seven_day, sample(41.2, 1_738_857_600));
}

#[test]
fn req_011_window_needs_integer_reset_and_numeric_percent() {
    let float_reset = r#"{"rate_limits":{"five_hour":{"used_percentage":1,"resets_at":1.7e9}}}"#;
    assert_eq!(parse_ok(float_reset).five_hour, None);
    let string_percent =
        r#"{"rate_limits":{"five_hour":{"used_percentage":"5","resets_at":1738425600}}}"#;
    assert_eq!(parse_ok(string_percent).five_hour, None);
    let no_reset = r#"{"rate_limits":{"five_hour":{"used_percentage":5}}}"#;
    assert_eq!(parse_ok(no_reset).five_hour, None);
}

#[test]
fn req_011_values_above_100_are_kept() {
    let over = r#"{"rate_limits":{"seven_day":{"used_percentage":120.5,"resets_at":1738857600}}}"#;
    assert_eq!(parse_ok(over).seven_day, sample(120.5, 1_738_857_600));
}

#[test]
fn req_011_empty_object_is_a_record_without_data() {
    let r = parse_ok("{}");
    assert_eq!(r.five_hour, None);
    assert_eq!(r.seven_day, None);
    assert_eq!(r.session_id, None);
}

#[test]
fn req_011_serialised_record_uses_the_documented_field_names() {
    let json = serde_json::to_value(parse_ok(BOTH)).unwrap();
    let mut keys: Vec<&str> = json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "cc_version",
            "context_used_pct",
            "cost_usd",
            "five_hour",
            "model",
            "received_at_ms",
            "session_id",
            "seven_day"
        ]
    );
    assert_eq!(json["five_hour"]["used_pct"], 23.5);
    assert_eq!(json["five_hour"]["resets_at"], 1_738_425_600);
    // Absent values are left out, not written as null.
    let bare = serde_json::to_value(parse_ok("{}")).unwrap();
    assert_eq!(bare.as_object().unwrap().len(), 1);
}

#[test]
fn req_011_record_survives_a_json_round_trip() {
    let r = parse_ok(BOTH);
    let back: Record = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
    assert_eq!(back, r);
}
