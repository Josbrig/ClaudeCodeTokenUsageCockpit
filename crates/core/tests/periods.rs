// SPDX-License-Identifier: Apache-2.0

use cockpit_core::model::{Record, WindowKind, WindowSample};
use cockpit_core::periods::{RESET_TOLERANCE_S, current, split};

const HOUR_MS: i64 = 3_600_000;
const T0_S: i64 = 1_738_400_000;
const T0_MS: i64 = T0_S * 1000;

fn sample(used_pct: f64, resets_at: i64) -> Option<WindowSample> {
    Some(WindowSample {
        used_pct,
        resets_at,
    })
}

fn record(received_at_ms: i64, five: Option<WindowSample>, seven: Option<WindowSample>) -> Record {
    Record {
        received_at_ms,
        session_id: None,
        cc_version: None,
        five_hour: five,
        seven_day: seven,
        model: None,
        context_used_pct: None,
        cost_usd: None,
    }
}

fn five(received_at_ms: i64, used_pct: f64, resets_at: i64) -> Record {
    record(received_at_ms, sample(used_pct, resets_at), None)
}

#[test]
fn req_022_reset_starts_new_period() {
    // Used 95 % shortly before the reset, 3 % after it; the reset time moves forward by 5 h.
    let reset1 = T0_S + 600;
    let reset2 = reset1 + 5 * 3600;
    let records = vec![
        five(T0_MS - HOUR_MS, 80.0, reset1),
        five(T0_MS, 95.0, reset1),
        five(T0_MS + 1_200_000, 3.0, reset2),
        five(T0_MS + 1_800_000, 5.0, reset2),
    ];
    let periods = split(&records, WindowKind::FiveHour);
    assert_eq!(periods.len(), 2);
    assert_eq!(periods[0].resets_at, reset1);
    assert_eq!(
        periods[0].samples,
        vec![(T0_MS - HOUR_MS, 80.0), (T0_MS, 95.0)]
    );
    assert_eq!(periods[1].resets_at, reset2);
    assert_eq!(
        periods[1].samples,
        vec![(T0_MS + 1_200_000, 3.0), (T0_MS + 1_800_000, 5.0)]
    );
}

#[test]
fn req_022_small_reset_jitter_stays_one_period() {
    let reset = T0_S + 3 * 3600;
    let jitters = [0, 60, -60, 30, -45, 1, -1];
    let records: Vec<_> = jitters
        .iter()
        .enumerate()
        .map(|(i, j)| five(T0_MS + i as i64 * 60_000, 10.0 + i as f64, reset + j))
        .collect();
    let periods = split(&records, WindowKind::FiveHour);
    assert_eq!(periods.len(), 1);
    assert_eq!(periods[0].samples.len(), jitters.len());
    // The period's reset time is the one of its latest record.
    assert_eq!(periods[0].resets_at, reset - 1);
}

#[test]
fn req_022_tolerance_is_symmetric_and_has_a_sharp_edge() {
    let reset = T0_S + 3 * 3600;
    let at_limit = [
        five(T0_MS, 10.0, reset),
        five(T0_MS + 1000, 11.0, reset + RESET_TOLERANCE_S),
    ];
    assert_eq!(split(&at_limit, WindowKind::FiveHour).len(), 1);
    let back_at_limit = [
        five(T0_MS, 10.0, reset),
        five(T0_MS + 1000, 11.0, reset - RESET_TOLERANCE_S),
    ];
    assert_eq!(split(&back_at_limit, WindowKind::FiveHour).len(), 1);
    let beyond = [
        five(T0_MS, 10.0, reset),
        five(T0_MS + 1000, 11.0, reset + RESET_TOLERANCE_S + 1),
    ];
    assert_eq!(split(&beyond, WindowKind::FiveHour).len(), 2);
    let backwards = [
        five(T0_MS, 10.0, reset),
        five(T0_MS + 1000, 11.0, reset - RESET_TOLERANCE_S - 1),
    ];
    assert_eq!(split(&backwards, WindowKind::FiveHour).len(), 2);
}

#[test]
fn req_022_a_record_received_after_the_reset_starts_a_new_period() {
    // The reported reset time did not move, but the record arrives after it.
    let reset = T0_S + 100;
    let records = [five(T0_MS, 40.0, reset), five(T0_MS + 200_000, 41.0, reset)];
    assert_eq!(split(&records, WindowKind::FiveHour).len(), 2);
}

#[test]
fn req_022_passed_reset_has_no_current_period() {
    let reset = T0_S + 3600;
    let records = [five(T0_MS, 40.0, reset)];
    let periods = split(&records, WindowKind::FiveHour);
    assert!(current(&periods, reset - 1).is_some());
    assert!(current(&periods, reset).is_none(), "reset reached");
    assert!(current(&periods, reset + 1).is_none());
}

#[test]
fn req_022_current_is_the_latest_period() {
    let reset1 = T0_S + 600;
    let reset2 = reset1 + 5 * 3600;
    let records = [
        five(T0_MS, 95.0, reset1),
        five(T0_MS + 1_200_000, 3.0, reset2),
    ];
    let periods = split(&records, WindowKind::FiveHour);
    let now = T0_S + 1300;
    assert_eq!(current(&periods, now).unwrap().resets_at, reset2);
}

#[test]
fn req_022_no_records_no_periods() {
    assert!(split(&[], WindowKind::FiveHour).is_empty());
    assert!(current(&[], T0_S).is_none());
}

#[test]
fn req_022_records_without_window_skipped() {
    let reset = T0_S + 3600;
    let records = [
        five(T0_MS, 10.0, reset),
        record(T0_MS + 1000, None, None),
        record(T0_MS + 2000, None, sample(50.0, T0_S + 86_400)),
        five(T0_MS + 3000, 12.0, reset),
    ];
    let five_periods = split(&records, WindowKind::FiveHour);
    assert_eq!(five_periods.len(), 1);
    assert_eq!(five_periods[0].samples.len(), 2);
    let seven_periods = split(&records, WindowKind::SevenDay);
    assert_eq!(seven_periods.len(), 1);
    assert_eq!(seven_periods[0].samples, vec![(T0_MS + 2000, 50.0)]);
    assert_eq!(seven_periods[0].kind, WindowKind::SevenDay);
}

#[test]
fn req_022_the_two_windows_are_split_independently() {
    let five_reset1 = T0_S + 600;
    let five_reset2 = five_reset1 + 5 * 3600;
    let week_reset = T0_S + 3 * 86_400;
    let records = [
        record(T0_MS, sample(90.0, five_reset1), sample(20.0, week_reset)),
        record(
            T0_MS + 1_200_000,
            sample(2.0, five_reset2),
            sample(21.0, week_reset),
        ),
    ];
    assert_eq!(split(&records, WindowKind::FiveHour).len(), 2);
    assert_eq!(split(&records, WindowKind::SevenDay).len(), 1);
}
