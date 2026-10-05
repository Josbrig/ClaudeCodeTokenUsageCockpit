// SPDX-License-Identifier: Apache-2.0

use chrono::{FixedOffset, TimeZone, Utc};
use chrono_tz::Europe::Berlin;
use cockpit_core::format::{
    age, deviation, duration, factor, forecast, local_time, pct, rate, stale, unused, weekly,
};
use cockpit_core::metrics::Forecast;

#[test]
fn req_001_pct_formatting() {
    assert_eq!(pct(23.5), "23.5%");
    assert_eq!(pct(76.5), "76.5%");
    assert_eq!(pct(62.0), "62.0%");
    assert_eq!(pct(4.5), "4.5%");
    assert_eq!(pct(0.0), "0.0%");
    assert_eq!(pct(100.0), "100.0%");
}

#[test]
fn req_001_pct_rounds_half_away_from_zero() {
    // The standard formatting of Rust would give 0.2 and 62.2 here (round half to even).
    assert_eq!(pct(0.25), "0.3%");
    assert_eq!(pct(62.25), "62.3%");
    assert_eq!(pct(0.04), "0.0%");
    assert_eq!(pct(0.05), "0.1%");
    assert_eq!(pct(99.96), "100.0%");
}

#[test]
fn req_001_pct_handles_negative_and_missing_values() {
    assert_eq!(pct(-3.55), "-3.6%");
    assert_eq!(pct(-0.04), "0.0%", "no minus sign in front of a zero");
    assert_eq!(pct(f64::NAN), "–");
    assert_eq!(pct(f64::INFINITY), "–");
}

#[test]
fn req_002_duration_days() {
    assert_eq!(duration(3 * 86_400 + 4 * 3_600), "3d 4h");
    assert_eq!(duration(86_400), "1d 0h");
    assert_eq!(duration(7 * 86_400 + 59 * 60), "7d 0h");
}

#[test]
fn req_001_duration_hours_and_minutes() {
    assert_eq!(duration(0), "0m");
    assert_eq!(duration(59), "0m");
    assert_eq!(duration(60), "1m");
    assert_eq!(duration(42 * 60), "42m");
    assert_eq!(duration(3_600), "1h 0m");
    assert_eq!(duration(3_600 + 12 * 60), "1h 12m");
    assert_eq!(duration(2 * 3_600 + 10 * 60), "2h 10m");
    assert_eq!(duration(86_399), "23h 59m");
}

#[test]
fn req_001_a_negative_duration_is_zero() {
    assert_eq!(duration(-1), "0m");
    assert_eq!(duration(-100_000), "0m");
}

#[test]
fn req_009_age_texts() {
    assert_eq!(age(12), "updated 12 s ago");
    assert_eq!(age(59), "updated 59 s ago");
    assert_eq!(age(60), "updated 1 min ago");
    assert_eq!(age(4 * 60 + 30), "updated 4 min ago");
    assert_eq!(age(3_599), "updated 59 min ago");
    assert_eq!(age(2 * 3_600 + 5 * 60), "updated 2h 5m ago");
    assert_eq!(age(3 * 86_400 + 4 * 3_600), "updated 3d 4h ago");
    assert_eq!(age(-5), "updated 0 s ago");
}

#[test]
fn req_009_stale_texts() {
    assert_eq!(stale(14 * 60), "stale, 14 min old");
    assert_eq!(stale(45), "stale, 45 s old");
    assert_eq!(stale(2 * 3_600 + 5 * 60), "stale, 2h 5m old");
}

#[test]
fn req_004_metric_texts() {
    assert_eq!(deviation(20.0), "+20.0 pp");
    assert_eq!(factor(Some(1.5)), "1.50×");
    assert_eq!(factor(None), "–");
}

#[test]
fn req_004_deviation_uses_a_real_minus_sign_and_no_sign_for_zero() {
    assert_eq!(deviation(-3.5), "\u{2212}3.5 pp");
    assert_eq!(deviation(0.0), "0.0 pp");
    assert_eq!(deviation(-0.04), "0.0 pp");
    assert_eq!(deviation(0.04), "0.0 pp");
    assert_eq!(deviation(f64::NAN), "–");
}

#[test]
fn req_004_factor_has_two_decimals() {
    assert_eq!(factor(Some(0.125)), "0.13×");
    assert_eq!(factor(Some(0.0)), "0.00×");
    assert_eq!(factor(Some(f64::NAN)), "–");
}

#[test]
fn req_005_rate_unused_and_weekly_texts() {
    assert_eq!(rate(Some(20.0)), "20.0 %/h");
    assert_eq!(rate(Some(10.0)), "10.0 %/h");
    assert_eq!(rate(None), "not available");
    assert_eq!(unused(Some(40.0)), "40.0% unused at reset");
    assert_eq!(unused(None), "not available");
    assert_eq!(weekly(10, 6.0), "10 windows left · 6.0% per window");
    assert_eq!(weekly(1, 60.0), "1 window left · 60.0% per window");
}

#[test]
fn req_005_forecast_texts() {
    let now = 1_738_400_000;
    assert_eq!(
        forecast(
            &Forecast::LimitFirst {
                at_s: now + 3 * 3_600
            },
            now
        ),
        "limit in 3h 0m, before reset"
    );
    assert_eq!(forecast(&Forecast::ResetFirst, now), "reset first");
    assert_eq!(forecast(&Forecast::LimitReached, now), "limit reached");
    assert_eq!(forecast(&Forecast::NotAvailable, now), "not available");
}

#[test]
fn req_002_local_time_in_a_given_zone() {
    // 2025-02-03 13:30 UTC is a Monday.
    let ts = Utc
        .with_ymd_and_hms(2025, 2, 3, 13, 30, 0)
        .unwrap()
        .timestamp();
    assert_eq!(local_time(ts, &Utc), "Mon 13:30");
    let plus_one = FixedOffset::east_opt(3_600).unwrap();
    assert_eq!(local_time(ts, &plus_one), "Mon 14:30");
    assert_eq!(local_time(ts, &Berlin), "Mon 14:30");
}

#[test]
fn req_026_dst_change_europe_berlin() {
    // The clocks go back at 03:00 on 2026-10-25. A reset at 03:30 local time, seen from
    // 22:00 local time the evening before, is 6 h 30 min away, not 5 h 30 min.
    let reset = Berlin
        .with_ymd_and_hms(2026, 10, 25, 3, 30, 0)
        .unwrap()
        .timestamp();
    let now = Berlin
        .with_ymd_and_hms(2026, 10, 24, 22, 0, 0)
        .unwrap()
        .timestamp();
    assert_eq!(local_time(reset, &Berlin), "Sun 03:30");
    assert_eq!(duration(reset - now), "6h 30m");
}

#[test]
fn req_026_dst_change_in_spring_shortens_the_clock_difference() {
    // The clocks go forward at 02:00 on 2026-03-29: from 22:00 to 08:00 local time are only
    // 9 hours.
    let from = Berlin
        .with_ymd_and_hms(2026, 3, 28, 22, 0, 0)
        .unwrap()
        .timestamp();
    let to = Berlin
        .with_ymd_and_hms(2026, 3, 29, 8, 0, 0)
        .unwrap()
        .timestamp();
    assert_eq!(duration(to - from), "9h 0m");
    assert_eq!(local_time(to, &Berlin), "Sun 08:00");
}

#[test]
fn req_002_a_timestamp_out_of_range_shows_a_dash() {
    assert_eq!(local_time(i64::MAX, &Utc), "–");
}

#[test]
fn req_005_forecast_text_does_not_overflow_for_extreme_times() {
    let text = forecast(&Forecast::LimitFirst { at_s: i64::MAX }, i64::MIN);
    assert!(text.starts_with("limit in "), "{text}");
}
