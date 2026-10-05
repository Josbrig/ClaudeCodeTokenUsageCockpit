// SPDX-License-Identifier: Apache-2.0

use cockpit_core::metrics::{Forecast, forecast, recommended_rate, unused_at_reset};

const NOW: i64 = 1_738_400_000;
const HOUR: i64 = 3600;

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
}

#[test]
fn req_005_forecast_three_hours() {
    // 40 % used, 20 %/h, reset in 5 h: the limit is reached in 3 h, before the reset.
    let f = forecast(40.0, NOW + 5 * HOUR, Some(20.0), NOW);
    assert_eq!(
        f,
        Forecast::LimitFirst {
            at_s: NOW + 3 * HOUR
        }
    );
}

#[test]
fn req_005_reset_first() {
    // Same usage, but the reset comes in 2 h: the reset comes first.
    assert_eq!(
        forecast(40.0, NOW + 2 * HOUR, Some(20.0), NOW),
        Forecast::ResetFirst
    );
}

#[test]
fn req_005_reaching_the_limit_exactly_at_the_reset_counts_as_reset_first() {
    assert_eq!(
        forecast(40.0, NOW + 3 * HOUR, Some(20.0), NOW),
        Forecast::ResetFirst
    );
    assert_eq!(
        forecast(40.0, NOW + 3 * HOUR + 1, Some(20.0), NOW),
        Forecast::LimitFirst {
            at_s: NOW + 3 * HOUR
        }
    );
}

#[test]
fn req_005_no_rate_not_available() {
    assert_eq!(
        forecast(40.0, NOW + HOUR, None, NOW),
        Forecast::NotAvailable
    );
}

#[test]
fn req_005_rate_zero_means_reset_first() {
    assert_eq!(
        forecast(40.0, NOW + HOUR, Some(0.0), NOW),
        Forecast::ResetFirst
    );
}

#[test]
fn req_005_limit_reached_at_100() {
    assert_eq!(
        forecast(100.0, NOW + HOUR, Some(5.0), NOW),
        Forecast::LimitReached
    );
    // The check comes first: also without a rate, and above 100 as delivered.
    assert_eq!(
        forecast(100.0, NOW + HOUR, None, NOW),
        Forecast::LimitReached
    );
    assert_eq!(
        forecast(120.0, NOW + HOUR, Some(0.0), NOW),
        Forecast::LimitReached
    );
}

#[test]
fn req_005_a_tiny_rate_does_not_overflow() {
    assert_eq!(
        forecast(1.0, NOW + HOUR, Some(1e-300), NOW),
        Forecast::ResetFirst
    );
}

#[test]
fn req_006_unused_forty_percent() {
    // Rate 5 %/h, 40 % used, reset in 4 h: 60 % would be used, 40 % stay unused.
    assert_close(
        unused_at_reset(40.0, NOW + 4 * HOUR, Some(5.0), NOW).unwrap(),
        40.0,
    );
}

#[test]
fn req_006_unused_is_never_negative() {
    assert_close(
        unused_at_reset(40.0, NOW + 4 * HOUR, Some(50.0), NOW).unwrap(),
        0.0,
    );
}

#[test]
fn req_006_unused_needs_a_rate() {
    assert_eq!(unused_at_reset(40.0, NOW + 4 * HOUR, None, NOW), None);
}

#[test]
fn req_006_after_the_reset_the_unused_part_is_what_was_left() {
    assert_close(
        unused_at_reset(30.0, NOW - 10, Some(50.0), NOW).unwrap(),
        70.0,
    );
}

#[test]
fn req_007_recommended_ten_per_hour() {
    // 40 % remaining (60 % used), reset in 4 h: 10 percent per hour.
    assert_close(recommended_rate(60.0, NOW + 4 * HOUR, NOW).unwrap(), 10.0);
}

#[test]
fn req_007_past_reset_none() {
    assert_eq!(recommended_rate(60.0, NOW, NOW), None);
    assert_eq!(recommended_rate(60.0, NOW - 1, NOW), None);
}

#[test]
fn req_007_nothing_left_to_use_recommends_zero() {
    assert_close(recommended_rate(100.0, NOW + HOUR, NOW).unwrap(), 0.0);
    assert_close(recommended_rate(130.0, NOW + HOUR, NOW).unwrap(), 0.0);
}
