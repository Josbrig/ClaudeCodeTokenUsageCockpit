// SPDX-License-Identifier: Apache-2.0

use chrono::{TimeZone, Utc};
use cockpit_core::metrics::{tokens_in_period, tokens_per_pp};
use cockpit_core::model::WindowKind;
use cockpit_core::periods::Period;
use cockpit_core::transcripts::{Entry, Usage};

const T10_00_MS: i64 = 1_738_400_000_000;
const MIN_MS: i64 = 60_000;

fn period(samples: &[(i64, f64)]) -> Period {
    Period {
        kind: WindowKind::FiveHour,
        resets_at: T10_00_MS / 1000 + 3 * 3600,
        samples: samples.to_vec(),
    }
}

fn entry(at_ms: i64, usage: Usage) -> Entry {
    Entry {
        timestamp: Utc.timestamp_millis_opt(at_ms).unwrap(),
        model: "m".to_owned(),
        message_id: None,
        request_id: None,
        usage,
    }
}

fn usage(input: u64, output: u64, cache_creation: u64, cache_read: u64) -> Usage {
    Usage {
        input,
        output,
        cache_creation,
        cache_read,
    }
}

#[test]
fn req_015_estimate_value() {
    // 10 percentage points for 1,000,000 tokens: 100,000 tokens per point.
    let p = period(&[(T10_00_MS, 40.0), (T10_00_MS + 30 * MIN_MS, 50.0)]);
    assert_eq!(tokens_per_pp(&p, 1_000_000), Some(100_000.0));
}

#[test]
fn req_015_none_below_one_pp() {
    let p = period(&[(T10_00_MS, 40.0), (T10_00_MS + 30 * MIN_MS, 40.9)]);
    assert_eq!(tokens_per_pp(&p, 1_000_000), None);
    let exactly = period(&[(T10_00_MS, 40.0), (T10_00_MS + 30 * MIN_MS, 41.0)]);
    assert_eq!(tokens_per_pp(&exactly, 500), Some(500.0));
    let falling = period(&[(T10_00_MS, 40.0), (T10_00_MS + MIN_MS, 30.0)]);
    assert_eq!(tokens_per_pp(&falling, 500), None);
}

#[test]
fn req_015_none_without_tokens() {
    let p = period(&[(T10_00_MS, 40.0), (T10_00_MS + 30 * MIN_MS, 50.0)]);
    assert_eq!(tokens_per_pp(&p, 0), None);
}

#[test]
fn req_015_none_for_a_period_without_a_rise_or_samples() {
    assert_eq!(tokens_per_pp(&period(&[]), 100), None);
    assert_eq!(tokens_per_pp(&period(&[(T10_00_MS, 40.0)]), 100), None);
}

#[test]
fn req_015_tokens_in_period_sums_the_four_counts_within_the_time_range() {
    let p = period(&[(T10_00_MS, 40.0), (T10_00_MS + 30 * MIN_MS, 50.0)]);
    let entries = [
        entry(T10_00_MS - 1, usage(1000, 0, 0, 0)), // before the period
        entry(T10_00_MS, usage(1, 2, 3, 4)),        // on the first record: included
        entry(T10_00_MS + 10 * MIN_MS, usage(10, 20, 30, 40)),
        entry(T10_00_MS + 40 * MIN_MS, usage(100, 0, 0, 0)), // exactly now: included
        entry(T10_00_MS + 40 * MIN_MS + 1, usage(5000, 0, 0, 0)), // after now
    ];
    assert_eq!(
        tokens_in_period(&entries, &p, T10_00_MS + 40 * MIN_MS),
        10 + 100 + 100
    );
}

#[test]
fn req_015_tokens_in_period_is_zero_without_samples_or_entries() {
    assert_eq!(
        tokens_in_period(&[], &period(&[(T10_00_MS, 1.0)]), T10_00_MS + 1),
        0
    );
    let e = [entry(T10_00_MS, usage(5, 5, 5, 5))];
    assert_eq!(tokens_in_period(&e, &period(&[]), T10_00_MS + 1), 0);
}

#[test]
fn req_015_estimate_from_entries() {
    let p = period(&[(T10_00_MS, 40.0), (T10_00_MS + 30 * MIN_MS, 45.0)]);
    let entries = [entry(T10_00_MS + MIN_MS, usage(100, 200, 300, 400))];
    let tokens = tokens_in_period(&entries, &p, T10_00_MS + 31 * MIN_MS);
    assert_eq!(tokens_per_pp(&p, tokens), Some(200.0));
}
