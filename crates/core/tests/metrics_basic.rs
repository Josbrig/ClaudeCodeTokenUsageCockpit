// SPDX-License-Identifier: Apache-2.0

use cockpit_core::metrics::{FIVE_HOUR_S, PaceState, SEVEN_DAY_S, basic, window_len_s};
use cockpit_core::model::WindowKind;

const NOW: i64 = 1_738_400_000;
const TOLERANCE: f64 = 5.0;

/// Reset time such that `elapsed` of the five-hour window have passed at `NOW`.
fn reset_for_elapsed_share(elapsed: f64) -> i64 {
    NOW + ((1.0 - elapsed) * FIVE_HOUR_S as f64).round() as i64
}

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
}

#[test]
fn req_001_window_lengths() {
    assert_eq!(FIVE_HOUR_S, 5 * 3600);
    assert_eq!(SEVEN_DAY_S, 7 * 24 * 3600);
    assert_eq!(window_len_s(WindowKind::FiveHour), FIVE_HOUR_S);
    assert_eq!(window_len_s(WindowKind::SevenDay), SEVEN_DAY_S);
}

#[test]
fn req_001_remaining_and_time_to_reset() {
    // 23.5 % used, reset in 2 h 10 min.
    let b = basic(23.5, NOW + 2 * 3600 + 10 * 60, FIVE_HOUR_S, NOW, TOLERANCE);
    assert_close(b.used, 23.5);
    assert_close(b.remaining, 76.5);
    assert_eq!(b.secs_to_reset, 7_800);
}

#[test]
fn req_002_seven_day_basic() {
    // Half of the week has passed; 41.2 % used.
    let reset = NOW + SEVEN_DAY_S / 2;
    let b = basic(41.2, reset, SEVEN_DAY_S, NOW, TOLERANCE);
    assert_close(b.remaining, 58.8);
    assert_eq!(b.secs_to_reset, SEVEN_DAY_S / 2);
    assert_close(b.target, 50.0);
    assert_close(b.deviation_pp, -8.8);
    assert_eq!(b.state, PaceState::Under);
}

#[test]
fn req_003_under_on_over() {
    let reset = reset_for_elapsed_share(0.5);
    let state = |used| basic(used, reset, FIVE_HOUR_S, NOW, TOLERANCE).state;
    assert_eq!(state(30.0), PaceState::Under);
    assert_eq!(state(52.0), PaceState::On);
    assert_eq!(state(70.0), PaceState::Over);
}

#[test]
fn req_003_edge_of_the_tolerance_band_counts_as_on_pace() {
    let reset = reset_for_elapsed_share(0.5); // target exactly 50
    let state = |used| basic(used, reset, FIVE_HOUR_S, NOW, TOLERANCE).state;
    assert_eq!(state(45.0), PaceState::On, "exactly -5 pp");
    assert_eq!(state(55.0), PaceState::On, "exactly +5 pp");
    assert_eq!(state(44.9), PaceState::Under);
    assert_eq!(state(55.1), PaceState::Over);
}

#[test]
fn req_003_a_zero_tolerance_has_no_on_pace_band() {
    let reset = reset_for_elapsed_share(0.5);
    let state = |used| basic(used, reset, FIVE_HOUR_S, NOW, 0.0).state;
    assert_eq!(state(50.0), PaceState::On);
    assert_eq!(state(49.9), PaceState::Under);
    assert_eq!(state(50.1), PaceState::Over);
}

#[test]
fn req_004_deviation_and_factor() {
    // 40 % of the window have passed (target 40); 60 % used.
    let b = basic(
        60.0,
        reset_for_elapsed_share(0.4),
        FIVE_HOUR_S,
        NOW,
        TOLERANCE,
    );
    assert_close(b.target, 40.0);
    assert_close(b.deviation_pp, 20.0);
    assert_close(b.pace_factor.unwrap(), 1.5);
    assert_eq!(b.state, PaceState::Over);
}

#[test]
fn req_004_factor_undefined_at_zero_elapsed() {
    // The window has just started: no division by zero.
    let b = basic(0.0, NOW + FIVE_HOUR_S, FIVE_HOUR_S, NOW, TOLERANCE);
    assert_close(b.target, 0.0);
    assert_eq!(b.pace_factor, None);
    let early = basic(3.0, NOW + FIVE_HOUR_S, FIVE_HOUR_S, NOW, TOLERANCE);
    assert_eq!(early.pace_factor, None);
    assert_close(early.deviation_pp, 3.0);
}

#[test]
fn req_004_time_before_the_start_of_the_window_counts_as_zero_elapsed() {
    let b = basic(1.0, NOW + 2 * FIVE_HOUR_S, FIVE_HOUR_S, NOW, TOLERANCE);
    assert_close(b.target, 0.0);
    assert_eq!(b.pace_factor, None);
}

#[test]
fn req_001_after_the_reset_nothing_is_left_to_wait_for() {
    let b = basic(80.0, NOW - 100, FIVE_HOUR_S, NOW, TOLERANCE);
    assert_eq!(b.secs_to_reset, 0);
    assert_close(b.target, 100.0);
}

#[test]
fn req_001_usage_is_clamped_to_zero_to_one_hundred() {
    let over = basic(120.5, NOW + 3600, FIVE_HOUR_S, NOW, TOLERANCE);
    assert_close(over.used, 100.0);
    assert_close(over.remaining, 0.0);
    let under = basic(-4.0, NOW + 3600, FIVE_HOUR_S, NOW, TOLERANCE);
    assert_close(under.used, 0.0);
    assert_close(under.remaining, 100.0);
}
