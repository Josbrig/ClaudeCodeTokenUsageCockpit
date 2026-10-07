// SPDX-License-Identifier: Apache-2.0
//! Binding limit, weekly plan, data age, stale marker and previous periods of the view model.
//! `NOW` is 3 h before the 5-hour reset (Saturday 2025-02-01 16:00 UTC, 17:00 in Berlin).

use chrono_tz::Europe::Berlin;
use cockpit_core::model::{Record, WindowKind, WindowSample};
use cockpit_core::settings::Settings;
use cockpit_core::viewmodel::{Inputs, PreviousPeriod, ViewModel, WindowView, build};

const RESET_5H: i64 = 1_738_425_600;
const NOW_S: i64 = RESET_5H - 3 * 3_600;
const NOW_MS: i64 = NOW_S * 1000;
const FIVE_H: i64 = 18_000;

fn sample(used_pct: f64, resets_at: i64) -> Option<WindowSample> {
    Some(WindowSample {
        used_pct,
        resets_at,
    })
}

fn record(received_s: i64, five: Option<WindowSample>, seven: Option<WindowSample>) -> Record {
    Record {
        received_at_ms: received_s * 1000,
        session_id: None,
        cc_version: None,
        five_hour: five,
        seven_day: seven,
        model: None,
        context_used_pct: None,
        cost_usd: None,
    }
}

fn view_with(records: &[Record], now_ms: i64, last_error_ms: Option<i64>) -> ViewModel {
    let settings = Settings::default();
    build(&Inputs {
        now_ms,
        records,
        last_error_ms,
        load_error: None,
        settings: &settings,
        stats: None,
        tz: &Berlin,
    })
}

fn view(records: &[Record]) -> ViewModel {
    view_with(records, NOW_MS, None)
}

fn is_binding(window: &WindowView) -> bool {
    match window {
        WindowView::Data(data) => data.binding,
        WindowView::NoData { .. } => false,
    }
}

/// Three samples in 30 minutes that rise by `step` percentage points each 15 minutes, ending
/// at `used_now`: a rate of `step * 4` percent per hour for the window.
fn rising(
    used_now: f64,
    step: f64,
    resets_at: i64,
    seven: bool,
) -> Vec<(i64, Option<WindowSample>)> {
    let _ = seven;
    vec![
        (NOW_S - 1800, sample(used_now - 2.0 * step, resets_at)),
        (NOW_S - 900, sample(used_now - step, resets_at)),
        (NOW_S, sample(used_now, resets_at)),
    ]
}

#[test]
fn req_008_binding_flag_set() {
    // Five-hour window: 60 % at 20 %/h, the limit comes in 2 h, before the reset in 3 h.
    // Seven-day window: 10 % at 1 %/h, the reset comes first.
    let reset_7d = RESET_5H + 3 * 86_400;
    let five = rising(60.0, 5.0, RESET_5H, false);
    let seven = rising(10.0, 0.25, reset_7d, true);
    let records: Vec<Record> = five
        .into_iter()
        .zip(seven)
        .map(|((at, f), (_, s))| record(at, f, s))
        .collect();
    let vm = view(&records);
    assert_eq!(vm.binding, Some(WindowKind::FiveHour));
    assert!(is_binding(&vm.five_hour));
    assert!(!is_binding(&vm.seven_day));
}

#[test]
fn req_008_weekly_window_binds_when_it_runs_out_first() {
    // The weekly window runs out in 1 h at 20 %/h from 80 %, the five-hour window is calm.
    let reset_7d = RESET_5H + 3 * 86_400;
    let five = rising(10.0, 0.25, RESET_5H, false);
    let seven = rising(80.0, 5.0, reset_7d, true);
    let records: Vec<Record> = five
        .into_iter()
        .zip(seven)
        .map(|((at, f), (_, s))| record(at, f, s))
        .collect();
    let vm = view(&records);
    assert_eq!(vm.binding, Some(WindowKind::SevenDay));
    assert!(is_binding(&vm.seven_day));
    assert!(!is_binding(&vm.five_hour));
}

#[test]
fn req_008_no_binding_with_one_window_or_without_data() {
    let one = [record(NOW_S, sample(60.0, RESET_5H), None)];
    let vm = view(&one);
    assert_eq!(vm.binding, None);
    assert!(!is_binding(&vm.five_hour));
    assert_eq!(view(&[]).binding, None);
}

#[test]
fn req_027_weekly_text() {
    // 50 h to the weekly reset and 60 % remaining: 10 windows, 6.0 % each (concept 7.8).
    let reset_7d = NOW_S + 50 * 3_600;
    let vm = view(&[record(
        NOW_S,
        sample(10.0, RESET_5H),
        sample(40.0, reset_7d),
    )]);
    assert_eq!(
        vm.weekly_text.as_deref(),
        Some("10 windows left · 6.0% per window")
    );
}

#[test]
fn req_027_weekly_text_needs_both_windows() {
    let only_five = view(&[record(NOW_S, sample(10.0, RESET_5H), None)]);
    assert_eq!(only_five.weekly_text, None);
    let only_seven = view(&[record(NOW_S, None, sample(40.0, NOW_S + 50 * 3_600))]);
    assert_eq!(only_seven.weekly_text, None);
    assert_eq!(view(&[]).weekly_text, None);
}

#[test]
fn req_027_weekly_text_is_absent_after_the_weekly_reset() {
    let records = [record(
        NOW_S,
        sample(10.0, RESET_5H),
        sample(40.0, NOW_S + 3_600),
    )];
    let later = view_with(&records, (NOW_S + 7_200) * 1000, None);
    assert_eq!(later.weekly_text, None);
}

#[test]
fn req_009_age_text_when_fresh() {
    let records = [record(NOW_S - 12, sample(10.0, RESET_5H), None)];
    let vm = view(&records);
    assert_eq!(vm.age_text, "updated 12 s ago");
    assert!(!vm.stale);
    let four_min = view_with(&records, (NOW_S - 12 + 240) * 1000, None);
    assert_eq!(four_min.age_text, "updated 4 min ago");
}

#[test]
fn req_009_stale_after_eleven_minutes() {
    // The default limit is 600 s; 11 minutes is over it.
    let records = [record(NOW_S - 660, sample(10.0, RESET_5H), None)];
    let vm = view(&records);
    assert!(vm.stale);
    assert_eq!(vm.age_text, "stale, 11 min old");
    // Exactly at the limit the data is still fresh.
    let at_limit = view(&[record(NOW_S - 600, sample(10.0, RESET_5H), None)]);
    assert!(!at_limit.stale);
}

#[test]
fn req_108_stale_after_newer_error() {
    let records = [record(NOW_S - 30, sample(10.0, RESET_5H), None)];
    let calm = view_with(&records, NOW_MS, Some((NOW_S - 60) * 1000));
    assert!(
        !calm.stale,
        "an error older than the record does not matter"
    );
    let after = view_with(&records, NOW_MS, Some((NOW_S - 10) * 1000));
    assert!(
        after.stale,
        "a malformed record arrived after the last good one"
    );
    assert_eq!(after.age_text, "stale, 30 s old");
}

#[test]
fn req_009_without_a_record_the_age_text_is_empty() {
    let vm = view(&[]);
    assert_eq!(vm.age_text, "");
    assert!(!vm.stale);
    // Even with an error there is no record to mark.
    assert!(!view_with(&[], NOW_MS, Some(NOW_MS)).stale);
}

#[test]
fn req_013_previous_periods_listed() {
    // Four finished five-hour periods, each with two records, and the current one.
    let mut records = Vec::new();
    for (i, final_used) in [(4_i64, 11.0), (3, 22.0), (2, 33.0), (1, 44.0)] {
        let reset = RESET_5H - i * FIVE_H - 3 * 3_600;
        records.push(record(reset - 7_200, sample(final_used - 5.0, reset), None));
        records.push(record(reset - 600, sample(final_used, reset), None));
    }
    records.push(record(NOW_S, sample(5.0, RESET_5H), None));
    let vm = view(&records);
    let previous: Vec<&PreviousPeriod> = vm.previous.iter().collect();
    assert_eq!(previous.len(), 3, "the oldest of four is left out");
    let used: Vec<&str> = previous
        .iter()
        .map(|p| p.final_used_text.as_str())
        .collect();
    assert_eq!(used, ["44.0%", "33.0%", "22.0%"], "newest first");
    assert!(previous.iter().all(|p| p.kind == WindowKind::FiveHour));
    // The newest finished period ended 8 h before RESET_5H: 08:00 UTC, 09:00 in Berlin.
    assert_eq!(previous[0].reset_local_text, "Sat 09:00");
}

#[test]
fn req_013_the_current_period_is_not_listed_and_values_are_clamped() {
    let reset_old = RESET_5H - 2 * FIVE_H;
    let records = [
        record(reset_old - 600, sample(130.0, reset_old), None),
        record(NOW_S, sample(5.0, RESET_5H), None),
    ];
    let vm = view(&records);
    assert_eq!(vm.previous.len(), 1);
    assert_eq!(vm.previous[0].final_used_text, "100.0%");
}

#[test]
fn req_013_both_windows_have_their_own_list() {
    let old5 = RESET_5H - 2 * FIVE_H;
    let old7 = RESET_5H - 8 * 86_400;
    let records = [
        record(old7 - 600, None, sample(70.0, old7)),
        record(old5 - 600, sample(20.0, old5), None),
        record(
            NOW_S,
            sample(5.0, RESET_5H),
            sample(10.0, RESET_5H + 3 * 86_400),
        ),
    ];
    let vm = view(&records);
    let kinds: Vec<WindowKind> = vm.previous.iter().map(|p| p.kind).collect();
    assert_eq!(kinds, [WindowKind::FiveHour, WindowKind::SevenDay]);
}
