// SPDX-License-Identifier: Apache-2.0
//! Fixtures in `tests/fixtures/transcripts/` hold invented values only. Expected sums:
//!
//! - `session_a.jsonl`: msg_001 (alpha: 100/50/200/0), msg_002 twice (alpha, the second line wins:
//!   10/25/0/300), msg_003 (beta: 5/7/0/0 at 23:30 UTC); one broken line, one user line.
//! - `session_b.jsonl`: msg_004 (beta: 40/60/0/160 on the next day); a summary line and an
//!   assistant line without `usage`.

use std::collections::BTreeMap;
use std::fs;

use chrono::{NaiveDate, Utc};
use chrono_tz::Europe::Berlin;
use cockpit_core::transcripts::{
    Entry, Stats, UNKNOWN_MODEL, Usage, aggregate, dedupe, parse_lines,
};

fn fixture(name: &str) -> String {
    let path = format!(
        "{}/tests/fixtures/transcripts/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    fs::read_to_string(path).unwrap()
}

fn all_entries() -> Vec<Entry> {
    let mut entries = parse_lines(&fixture("session_a.jsonl"));
    entries.extend(parse_lines(&fixture("session_b.jsonl")));
    entries
}

fn usage(input: u64, output: u64, cache_creation: u64, cache_read: u64) -> Usage {
    Usage {
        input,
        output,
        cache_creation,
        cache_read,
    }
}

fn day(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 3, day).unwrap()
}

#[test]
fn req_014_parse_reads_only_assistant_lines_with_usage() {
    let a = parse_lines(&fixture("session_a.jsonl"));
    let ids: Vec<_> = a.iter().map(|e| e.message_id.as_deref().unwrap()).collect();
    assert_eq!(ids, ["msg_001", "msg_002", "msg_003"]);
    let b = parse_lines(&fixture("session_b.jsonl"));
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].message_id.as_deref(), Some("msg_004"));
}

#[test]
fn req_014_missing_counts_are_zero() {
    let entries = parse_lines(&fixture("session_a.jsonl"));
    assert_eq!(entries[2].usage, usage(5, 7, 0, 0));
}

#[test]
fn req_014_totals_per_model() {
    let stats = aggregate(&all_entries(), &Utc);
    let expected = BTreeMap::from([
        ("model-alpha".to_owned(), usage(110, 75, 200, 300)),
        ("model-beta".to_owned(), usage(45, 67, 0, 160)),
    ]);
    assert_eq!(stats.per_model, expected);
    assert_eq!(stats.total(), usage(155, 142, 200, 460));
    assert_eq!(stats.understood_lines, 4);
}

#[test]
fn req_014_totals_per_day() {
    let utc = aggregate(&all_entries(), &Utc);
    assert_eq!(
        utc.per_day,
        BTreeMap::from([
            (day(1), usage(115, 82, 200, 300)),
            (day(2), usage(40, 60, 0, 160)),
        ])
    );
}

#[test]
fn req_014_the_day_is_the_local_calendar_day() {
    // 23:30 UTC on 1 March is 00:30 on 2 March in Berlin (UTC+1 until the end of March).
    let berlin = aggregate(&all_entries(), &Berlin);
    assert_eq!(
        berlin.per_day,
        BTreeMap::from([
            (day(1), usage(110, 75, 200, 300)),
            (day(2), usage(45, 67, 0, 160)),
        ])
    );
}

#[test]
fn req_014_duplicates_counted_once() {
    let entries = parse_lines(&fixture("session_a.jsonl"));
    let second = entries
        .iter()
        .find(|e| e.message_id.as_deref() == Some("msg_002"));
    // The later line (output 25) replaces the earlier one (output 20).
    assert_eq!(second.unwrap().usage, usage(10, 25, 0, 300));
    // The same file read twice and handed over together changes nothing.
    let doubled = [all_entries(), all_entries()].concat();
    assert_eq!(aggregate(&doubled, &Utc), aggregate(&all_entries(), &Utc));
}

#[test]
fn req_014_same_message_with_another_request_id_is_not_a_duplicate() {
    let line = |request: &str, output: u32| {
        format!(
            r#"{{"type":"assistant","timestamp":"2026-03-01T10:00:00Z","requestId":"{request}","message":{{"id":"msg_x","model":"m","usage":{{"output_tokens":{output}}}}}}}"#
        )
    };
    let text = [line("r1", 5), line("r2", 7), line("r1", 9)].join("\n");
    let entries = parse_lines(&text);
    let outputs: Vec<u64> = entries.iter().map(|e| e.usage.output).collect();
    assert_eq!(
        outputs,
        [9, 7],
        "r1 kept at its first position with the last value"
    );
}

#[test]
fn req_014_a_message_without_id_is_never_merged() {
    let line = r#"{"type":"assistant","timestamp":"2026-03-01T10:00:00Z","message":{"usage":{"output_tokens":4}}}"#;
    let entries = parse_lines(&format!("{line}\n{line}"));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].model, UNKNOWN_MODEL);
    assert_eq!(dedupe(entries).len(), 2);
}

#[test]
fn req_014_cache_share() {
    let stats = aggregate(&all_entries(), &Utc);
    let alpha = stats.per_model["model-alpha"].cache_share().unwrap();
    let beta = stats.per_model["model-beta"].cache_share().unwrap();
    assert!((alpha - 500.0 / 610.0).abs() < 1e-12, "{alpha}");
    assert!((beta - 160.0 / 205.0).abs() < 1e-12, "{beta}");
    let total = stats.total().cache_share().unwrap();
    assert!((total - 660.0 / 815.0).abs() < 1e-12, "{total}");
}

#[test]
fn req_014_cache_share_is_none_without_input() {
    assert_eq!(Usage::default().cache_share(), None);
    assert_eq!(usage(0, 9, 0, 0).cache_share(), None);
    assert_eq!(usage(1, 0, 0, 0).cache_share(), Some(0.0));
    assert_eq!(usage(0, 0, 3, 1).cache_share(), Some(1.0));
}

#[test]
fn req_014_unknown_format_yields_zero_understood_lines() {
    let entries = parse_lines(&fixture("unknown_format.jsonl"));
    assert!(entries.is_empty());
    let stats = aggregate(&entries, &Utc);
    assert_eq!(stats, Stats::default());
    assert_eq!(stats.understood_lines, 0);
    assert_eq!(parse_lines("").len(), 0);
}

#[test]
fn req_014_odd_values_do_not_stop_the_parser() {
    let lines = [
        // counts of the wrong type count as 0, the line is still understood
        r#"{"type":"assistant","timestamp":"2026-03-01T10:00:00Z","message":{"id":"a","usage":{"input_tokens":"5","output_tokens":-3,"cache_read_input_tokens":2.5}}}"#,
        // no or invalid timestamp: not usable for a day, skipped
        r#"{"type":"assistant","message":{"id":"b","usage":{"input_tokens":1}}}"#,
        r#"{"type":"assistant","timestamp":"yesterday","message":{"id":"c","usage":{"input_tokens":1}}}"#,
        // a message that is not an object
        r#"{"type":"assistant","timestamp":"2026-03-01T10:00:00Z","message":"text"}"#,
        // windows line ending and surrounding blanks
        "  {\"type\":\"assistant\",\"timestamp\":\"2026-03-01T10:00:00+02:00\",\"message\":{\"id\":\"d\",\"usage\":{\"input_tokens\":4}}}  \r",
    ]
    .join("\n");
    let entries = parse_lines(&lines);
    let ids: Vec<_> = entries
        .iter()
        .map(|e| e.message_id.as_deref().unwrap())
        .collect();
    assert_eq!(ids, ["a", "d"]);
    assert_eq!(entries[0].usage, Usage::default());
    assert_eq!(entries[1].usage.input, 4);
    // 10:00 at UTC+2 is 08:00 UTC
    assert_eq!(
        entries[1].timestamp.to_rfc3339(),
        "2026-03-01T08:00:00+00:00"
    );
}

#[test]
fn req_014_counts_saturate_instead_of_overflowing() {
    let mut sum = usage(u64::MAX, 0, 0, 0);
    sum.add(&usage(5, 1, 0, 0));
    assert_eq!(sum, usage(u64::MAX, 1, 0, 0));
}

#[test]
fn req_015_tokens_between_sums_a_closed_time_range() {
    let stats = aggregate(&all_entries(), &Utc);
    let t = |s: &str| {
        chrono::DateTime::parse_from_rfc3339(s)
            .unwrap()
            .timestamp_millis()
    };
    // msg_001 (350), msg_002 (335), msg_003 (12) on 1 March, msg_004 (260) on 2 March.
    assert_eq!(stats.tokens_between(0, i64::MAX), 350 + 335 + 12 + 260);
    assert_eq!(
        stats.tokens_between(t("2026-03-01T10:00:05Z"), t("2026-03-01T10:01:30Z")),
        350 + 335,
        "both ends are included"
    );
    assert_eq!(
        stats.tokens_between(t("2026-03-01T10:00:06Z"), t("2026-03-01T10:01:29Z")),
        0
    );
    assert_eq!(stats.tokens_between(t("2026-03-03T00:00:00Z"), i64::MAX), 0);
    assert_eq!(Stats::default().tokens_between(0, i64::MAX), 0);
}
