// SPDX-License-Identifier: Apache-2.0

use cockpit_core::metrics::rate;
use cockpit_core::model::WindowKind;
use cockpit_core::periods::Period;

const MIN_MS: i64 = 60_000;
/// A fixed point in time: 10:00 on the day of the samples, in Unix milliseconds.
const T10_00_MS: i64 = 1_738_400_000_000;
const T10_00_S: i64 = T10_00_MS / 1000;
const RATE_PERIOD_S: i64 = 1_800;

fn period(samples: &[(i64, f64)]) -> Period {
    Period {
        kind: WindowKind::FiveHour,
        resets_at: T10_00_S + 3 * 3600,
        samples: samples.to_vec(),
    }
}

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
}

#[test]
fn req_021_rate_from_three_samples() {
    // 10:00 40 %, 10:15 45 %, 10:30 50 %: 20 percent per hour.
    let p = period(&[
        (T10_00_MS, 40.0),
        (T10_00_MS + 15 * MIN_MS, 45.0),
        (T10_00_MS + 30 * MIN_MS, 50.0),
    ]);
    let now = T10_00_S + 30 * 60;
    assert_close(rate(&p, now, RATE_PERIOD_S).unwrap(), 20.0);
}

#[test]
fn req_021_two_samples_are_enough() {
    let p = period(&[(T10_00_MS, 10.0), (T10_00_MS + 6 * MIN_MS, 11.0)]);
    assert_close(rate(&p, T10_00_S + 6 * 60, RATE_PERIOD_S).unwrap(), 10.0);
}

#[test]
fn req_021_single_sample_none() {
    let p = period(&[(T10_00_MS, 40.0)]);
    assert_eq!(rate(&p, T10_00_S, RATE_PERIOD_S), None);
}

#[test]
fn req_021_no_samples_none() {
    assert_eq!(rate(&period(&[]), T10_00_S, RATE_PERIOD_S), None);
}

#[test]
fn req_021_samples_at_the_same_time_none() {
    let p = period(&[(T10_00_MS, 40.0), (T10_00_MS, 41.0), (T10_00_MS, 42.0)]);
    assert_eq!(rate(&p, T10_00_S, RATE_PERIOD_S), None);
}

#[test]
fn req_021_old_samples_ignored() {
    // A very steep start one hour ago must not influence the rate of the last half hour.
    let p = period(&[
        (T10_00_MS - 60 * MIN_MS, 0.0),
        (T10_00_MS - 55 * MIN_MS, 30.0),
        (T10_00_MS, 40.0),
        (T10_00_MS + 15 * MIN_MS, 45.0),
        (T10_00_MS + 30 * MIN_MS, 50.0),
    ]);
    let now = T10_00_S + 30 * 60;
    assert_close(rate(&p, now, RATE_PERIOD_S).unwrap(), 20.0);
}

#[test]
fn req_021_the_start_of_the_period_is_included() {
    // A sample exactly 30 minutes old is still inside.
    let p = period(&[(T10_00_MS, 40.0), (T10_00_MS + 30 * MIN_MS, 50.0)]);
    assert_close(rate(&p, T10_00_S + 30 * 60, RATE_PERIOD_S).unwrap(), 20.0);
    // One second later the first sample is outside and only one sample is left.
    assert_eq!(rate(&p, T10_00_S + 30 * 60 + 1, RATE_PERIOD_S), None);
}

#[test]
fn req_021_negative_slope_is_zero() {
    let p = period(&[(T10_00_MS, 50.0), (T10_00_MS + 10 * MIN_MS, 40.0)]);
    assert_eq!(rate(&p, T10_00_S + 600, RATE_PERIOD_S), Some(0.0));
}

#[test]
fn req_021_flat_usage_is_a_rate_of_zero() {
    let p = period(&[(T10_00_MS, 40.0), (T10_00_MS + 10 * MIN_MS, 40.0)]);
    assert_eq!(rate(&p, T10_00_S + 600, RATE_PERIOD_S), Some(0.0));
}

#[test]
fn req_021_uneven_spacing_uses_the_least_squares_fit() {
    // Points on the line u = 40 + 12 * t (t in hours), at irregular times.
    let times_min = [0, 4, 5, 17, 29];
    let p = period(
        &times_min
            .iter()
            .map(|m| (T10_00_MS + m * MIN_MS, 40.0 + 12.0 * (*m as f64 / 60.0)))
            .collect::<Vec<_>>(),
    );
    assert_close(rate(&p, T10_00_S + 29 * 60, RATE_PERIOD_S).unwrap(), 12.0);
}

#[test]
fn req_021_a_shorter_rate_period_looks_only_at_recent_samples() {
    let p = period(&[
        (T10_00_MS, 40.0),
        (T10_00_MS + 15 * MIN_MS, 45.0),
        (T10_00_MS + 30 * MIN_MS, 80.0),
    ]);
    let now = T10_00_S + 30 * 60;
    // Over the last 15 minutes only the last two samples count: 35 points in 15 minutes.
    assert_close(rate(&p, now, 900).unwrap(), 140.0);
}
