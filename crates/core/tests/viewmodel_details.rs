// SPDX-License-Identifier: Apache-2.0
//! Session details, transcript table and the token estimate of the view model.
//! `NOW` is 13:00 UTC on Saturday 2025-02-01, three hours before the 5-hour reset.

use chrono::{TimeZone, Utc};
use chrono_tz::Europe::Berlin;
use cockpit_core::model::{Record, WindowSample};
use cockpit_core::settings::Settings;
use cockpit_core::transcripts::{Entry, Stats, Usage, aggregate};
use cockpit_core::viewmodel::{
    Inputs, MAX_TRANSCRIPT_DAYS, SESSION_HEADING, TRANSCRIPTS_NOT_AVAILABLE, ViewModel, build,
};

const RESET_5H: i64 = 1_738_425_600;
const NOW_S: i64 = RESET_5H - 3 * 3_600;
const NOW_MS: i64 = NOW_S * 1000;

fn record(received_s: i64, used: f64) -> Record {
    Record {
        received_at_ms: received_s * 1000,
        session_id: None,
        cc_version: None,
        five_hour: Some(WindowSample {
            used_pct: used,
            resets_at: RESET_5H,
        }),
        seven_day: None,
        model: None,
        context_used_pct: None,
        cost_usd: None,
    }
}

fn view(records: &[Record], stats: Option<&Stats>) -> ViewModel {
    let settings = Settings::default();
    build(&Inputs {
        now_ms: NOW_MS,
        records,
        last_error_ms: None,
        load_error: None,
        settings: &settings,
        stats,
        tz: &Berlin,
    })
}

fn entry(at_ms: i64, model: &str, usage: Usage) -> Entry {
    Entry {
        timestamp: Utc.timestamp_millis_opt(at_ms).unwrap(),
        model: model.to_owned(),
        message_id: None,
        request_id: None,
        usage,
    }
}

fn tokens(input: u64, output: u64, cache_creation: u64, cache_read: u64) -> Usage {
    Usage {
        input,
        output,
        cache_creation,
        cache_read,
    }
}

#[test]
fn req_028_session_details() {
    let mut r = record(NOW_S, 10.0);
    r.model = Some("Opus".to_owned());
    r.context_used_pct = Some(42.0);
    r.cost_usd = Some(0.0123);
    let vm = view(&[r], None);
    assert_eq!(vm.session.heading, "From Claude Code");
    assert_eq!(vm.session.heading, SESSION_HEADING);
    assert_eq!(vm.session.model_text, "Opus");
    assert_eq!(vm.session.context_text, "42.0%");
    assert_eq!(vm.session.cost_text, "0.01 USD");
}

#[test]
fn req_028_session_details_missing() {
    for records in [vec![], vec![record(NOW_S, 10.0)]] {
        let s = view(&records, None).session;
        assert_eq!(s.heading, "From Claude Code");
        assert_eq!(s.model_text, "no data");
        assert_eq!(s.context_text, "no data");
        assert_eq!(s.cost_text, "no data");
    }
}

#[test]
fn req_028_cost_has_two_decimals_and_the_newest_record_decides() {
    let mut old = record(NOW_S - 60, 10.0);
    old.cost_usd = Some(9.0);
    old.model = Some("Old".to_owned());
    let mut new = record(NOW_S, 10.0);
    new.cost_usd = Some(12.0);
    let s = view(&[old, new], None).session;
    assert_eq!(s.cost_text, "12.00 USD");
    assert_eq!(
        s.model_text, "no data",
        "no value of an older record is mixed in"
    );
}

#[test]
fn req_014_transcript_table_rows() {
    // 23:30 UTC on 31 January is already 1 February in Berlin.
    let day1 = Utc
        .with_ymd_and_hms(2025, 1, 31, 23, 30, 0)
        .unwrap()
        .timestamp_millis();
    let day2 = Utc
        .with_ymd_and_hms(2025, 2, 1, 9, 0, 0)
        .unwrap()
        .timestamp_millis();
    let entries = [
        entry(day1, "model-b", tokens(1_000, 2_000, 0, 0)),
        entry(day2, "model-a", tokens(1_234_567, 5, 100, 300)),
        entry(day2 + 1, "model-a", tokens(1, 1, 0, 0)),
    ];
    let stats = aggregate(&entries, &Berlin);
    let vm = view(&[], Some(&stats));
    let t = &vm.transcripts;
    assert!(t.available);
    let models: Vec<&str> = t.per_model.iter().map(|(m, _)| m.as_str()).collect();
    assert_eq!(models, ["model-a", "model-b"]);
    let a = &t.per_model[0].1;
    assert_eq!(a.input, "1,234,568");
    assert_eq!(a.output, "6");
    assert_eq!(a.cache_creation, "100");
    assert_eq!(a.cache_read, "300");
    // Both messages fall on 1 February in Berlin, newest day first.
    assert_eq!(t.per_day.len(), 1);
    assert_eq!(t.per_day[0].0, "2025-02-01");
    assert_eq!(t.per_day[0].1.input, "1,235,568");
    // cache (400) / (input 1,235,568 + cache 400)
    assert_eq!(t.cache_share_text, "cache share 0.0%");
}

#[test]
fn req_014_days_newest_first_and_at_most_35() {
    let start = Utc
        .with_ymd_and_hms(2024, 12, 1, 12, 0, 0)
        .unwrap()
        .timestamp_millis();
    let entries: Vec<Entry> = (0..40)
        .map(|d| entry(start + d * 86_400_000, "m", tokens(1, 0, 0, 0)))
        .collect();
    let stats = aggregate(&entries, &Utc);
    let days = &view(&[], Some(&stats)).transcripts.per_day;
    assert_eq!(days.len(), MAX_TRANSCRIPT_DAYS);
    assert_eq!(days[0].0, "2025-01-09", "the 40th day is the newest");
    assert_eq!(days[34].0, "2024-12-06");
}

#[test]
fn req_014_cache_share_text() {
    let entries = [entry(NOW_MS, "m", tokens(100, 7, 300, 600))];
    let stats = aggregate(&entries, &Utc);
    // (300 + 600) / (100 + 300 + 600) = 90 %
    assert_eq!(
        view(&[], Some(&stats)).transcripts.cache_share_text,
        "cache share 90.0%"
    );
    let no_input = aggregate(&[entry(NOW_MS, "m", tokens(0, 5, 0, 0))], &Utc);
    let t = view(&[], Some(&no_input)).transcripts;
    assert!(t.available);
    assert_eq!(t.cache_share_text, "cache share not available");
}

#[test]
fn req_014_transcripts_not_available() {
    for stats in [None, Some(Stats::default())] {
        let vm = view(&[], stats.as_ref());
        assert!(!vm.transcripts.available);
        assert!(vm.transcripts.per_model.is_empty());
        assert!(vm.transcripts.per_day.is_empty());
        assert_eq!(
            vm.transcripts.cache_share_text,
            "transcript statistics not available"
        );
        assert_eq!(vm.transcripts.cache_share_text, TRANSCRIPTS_NOT_AVAILABLE);
        assert_eq!(vm.estimate_text, None);
    }
}

#[test]
fn req_015_estimate_labelled() {
    // 40 % at the start of the period (received 30 min ago), 50 % now: 10 pp.
    let records = [record(NOW_S - 1800, 40.0), record(NOW_S, 50.0)];
    // 1,000,000 tokens in the period, plus tokens before and after that must not count.
    let entries = [
        entry(NOW_MS - 1_800_001, "m", tokens(5_000_000, 0, 0, 0)),
        entry(
            NOW_MS - 1_800_000,
            "m",
            tokens(400_000, 100_000, 200_000, 300_000),
        ),
        entry(NOW_MS, "m", tokens(0, 0, 0, 0)),
        entry(NOW_MS + 1, "m", tokens(7_000_000, 0, 0, 0)),
    ];
    let stats = aggregate(&entries, &Utc);
    let vm = view(&records, Some(&stats));
    assert_eq!(
        vm.estimate_text.as_deref(),
        Some("≈ 100,000 tokens per 1% (estimate)")
    );
}

#[test]
fn req_015_no_estimate_without_a_rise_or_tokens() {
    let entries = [entry(NOW_MS - 60_000, "m", tokens(1_000, 0, 0, 0))];
    let stats = aggregate(&entries, &Utc);
    // less than 1 percentage point
    let flat = [record(NOW_S - 1800, 40.0), record(NOW_S, 40.5)];
    assert_eq!(view(&flat, Some(&stats)).estimate_text, None);
    // no tokens in the period
    let rising = [record(NOW_S - 1800, 40.0), record(NOW_S, 50.0)];
    let old_only = aggregate(&[entry(NOW_MS - 3_600_000, "m", tokens(9, 9, 9, 9))], &Utc);
    assert_eq!(view(&rising, Some(&old_only)).estimate_text, None);
    // no five-hour period at all
    assert_eq!(view(&[], Some(&stats)).estimate_text, None);
}
