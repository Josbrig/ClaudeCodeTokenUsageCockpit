// SPDX-License-Identifier: Apache-2.0

use cockpit_core::metrics::{data_age_s, is_stale, latest};
use cockpit_core::model::Record;

const NOW_MS: i64 = 1_738_400_000_000;
const MINUTE_MS: i64 = 60_000;
const DEFAULT_STALE_S: u32 = 600;

fn record(received_at_ms: i64, session: &str) -> Record {
    Record {
        received_at_ms,
        session_id: Some(session.to_owned()),
        cc_version: None,
        five_hour: None,
        seven_day: None,
        model: None,
        context_used_pct: None,
        cost_usd: None,
    }
}

#[test]
fn req_009_age_in_whole_seconds() {
    let r = record(NOW_MS - 12_999, "a");
    assert_eq!(data_age_s(&r, NOW_MS), 12);
    assert_eq!(data_age_s(&record(NOW_MS, "a"), NOW_MS), 0);
}

#[test]
fn req_009_a_record_from_the_future_has_age_zero() {
    assert_eq!(data_age_s(&record(NOW_MS + 5_000, "a"), NOW_MS), 0);
}

#[test]
fn req_009_eleven_minutes_is_stale() {
    let r = record(NOW_MS - 11 * MINUTE_MS, "a");
    let age = data_age_s(&r, NOW_MS);
    assert_eq!(age, 660);
    assert!(is_stale(age, DEFAULT_STALE_S, None, r.received_at_ms));
}

#[test]
fn req_009_fresh_not_stale() {
    let r = record(NOW_MS - 12_000, "a");
    assert!(!is_stale(
        data_age_s(&r, NOW_MS),
        DEFAULT_STALE_S,
        None,
        r.received_at_ms
    ));
}

#[test]
fn req_009_exactly_the_threshold_is_not_yet_stale() {
    assert!(!is_stale(600, DEFAULT_STALE_S, None, NOW_MS));
    assert!(is_stale(601, DEFAULT_STALE_S, None, NOW_MS));
}

#[test]
fn req_009_the_threshold_is_configurable() {
    assert!(is_stale(130, 120, None, NOW_MS));
    assert!(!is_stale(130, 3_600, None, NOW_MS));
}

#[test]
fn req_108_newer_error_marks_stale_immediately() {
    // A fresh record, but a malformed input arrived after it: stale right away.
    let latest_ms = NOW_MS - 5_000;
    assert!(is_stale(
        5,
        DEFAULT_STALE_S,
        Some(NOW_MS - 1_000),
        latest_ms
    ));
}

#[test]
fn req_108_an_older_or_equal_error_does_not_mark_stale() {
    let latest_ms = NOW_MS - 5_000;
    assert!(!is_stale(
        5,
        DEFAULT_STALE_S,
        Some(latest_ms - 1),
        latest_ms
    ));
    assert!(!is_stale(5, DEFAULT_STALE_S, Some(latest_ms), latest_ms));
    assert!(!is_stale(5, DEFAULT_STALE_S, None, latest_ms));
}

#[test]
fn req_017_latest_of_interleaved_sessions() {
    let records = vec![
        record(NOW_MS - 50_000, "a"),
        record(NOW_MS - 40_000, "b"),
        record(NOW_MS - 30_000, "a"),
        record(NOW_MS - 10_000, "b"),
        record(NOW_MS - 20_000, "a"),
    ];
    let newest = latest(&records).unwrap();
    assert_eq!(newest.received_at_ms, NOW_MS - 10_000);
    assert_eq!(newest.session_id.as_deref(), Some("b"));
}

#[test]
fn req_017_latest_does_not_depend_on_the_order_of_the_list() {
    let mut records = vec![
        record(NOW_MS - 10_000, "b"),
        record(NOW_MS - 30_000, "a"),
        record(NOW_MS - 20_000, "a"),
    ];
    assert_eq!(latest(&records).unwrap().received_at_ms, NOW_MS - 10_000);
    records.reverse();
    assert_eq!(latest(&records).unwrap().received_at_ms, NOW_MS - 10_000);
}

#[test]
fn req_017_latest_of_nothing_is_none() {
    assert!(latest(&[]).is_none());
}

#[test]
fn req_017_equal_times_take_the_later_entry() {
    let records = vec![record(NOW_MS, "a"), record(NOW_MS, "b")];
    assert_eq!(latest(&records).unwrap().session_id.as_deref(), Some("b"));
}
