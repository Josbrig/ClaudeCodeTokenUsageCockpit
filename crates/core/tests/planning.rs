// SPDX-License-Identifier: Apache-2.0

use cockpit_core::metrics::{Forecast, binding};
use cockpit_core::model::WindowKind;
use cockpit_core::planning::weekly;

const NOW: i64 = 1_738_400_000;
const HOUR: i64 = 3600;

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
}

fn limit_first(at_s: i64) -> Forecast {
    Forecast::LimitFirst { at_s }
}

// ---- binding limit (REQ-008) ------------------------------------------------------

#[test]
fn req_008_weekly_binding_when_exhausted_first() {
    // The weekly limit is predicted before the 5-hour limit.
    assert_eq!(
        binding(
            Some(&limit_first(NOW + 4 * HOUR)),
            Some(&limit_first(NOW + HOUR))
        ),
        Some(WindowKind::SevenDay)
    );
    assert_eq!(
        binding(
            Some(&limit_first(NOW + HOUR)),
            Some(&limit_first(NOW + 4 * HOUR))
        ),
        Some(WindowKind::FiveHour)
    );
}

#[test]
fn req_008_weekly_binding_when_the_five_hour_window_resets_first() {
    // REQ-008 acceptance: the weekly limit is exhausted before the 5-hour window resets.
    assert_eq!(
        binding(Some(&Forecast::ResetFirst), Some(&limit_first(NOW + HOUR))),
        Some(WindowKind::SevenDay)
    );
}

#[test]
fn req_008_five_hour_binding_when_only_it_reaches_its_limit() {
    assert_eq!(
        binding(Some(&limit_first(NOW + HOUR)), Some(&Forecast::ResetFirst)),
        Some(WindowKind::FiveHour)
    );
    assert_eq!(
        binding(
            Some(&limit_first(NOW + HOUR)),
            Some(&Forecast::NotAvailable)
        ),
        Some(WindowKind::FiveHour)
    );
}

#[test]
fn req_008_none_when_both_reset_first() {
    assert_eq!(
        binding(Some(&Forecast::ResetFirst), Some(&Forecast::ResetFirst)),
        None
    );
    assert_eq!(
        binding(Some(&Forecast::NotAvailable), Some(&Forecast::ResetFirst)),
        None
    );
}

#[test]
fn req_008_limit_reached_is_binding() {
    // A window that has reached its limit counts as exhausted now, before any prediction.
    assert_eq!(
        binding(Some(&Forecast::LimitReached), Some(&limit_first(NOW))),
        Some(WindowKind::FiveHour)
    );
    assert_eq!(
        binding(Some(&limit_first(NOW)), Some(&Forecast::LimitReached)),
        Some(WindowKind::SevenDay)
    );
    assert_eq!(
        binding(Some(&Forecast::LimitReached), Some(&Forecast::ResetFirst)),
        Some(WindowKind::FiveHour)
    );
}

#[test]
fn req_008_a_tie_goes_to_the_weekly_window() {
    assert_eq!(
        binding(
            Some(&limit_first(NOW + HOUR)),
            Some(&limit_first(NOW + HOUR))
        ),
        Some(WindowKind::SevenDay)
    );
    assert_eq!(
        binding(Some(&Forecast::LimitReached), Some(&Forecast::LimitReached)),
        Some(WindowKind::SevenDay)
    );
}

#[test]
fn req_008_needs_a_forecast_for_both_windows() {
    // With only one window delivered there is nothing to compare.
    assert_eq!(binding(Some(&limit_first(NOW + HOUR)), None), None);
    assert_eq!(binding(None, Some(&limit_first(NOW + HOUR))), None);
    assert_eq!(binding(None, None), None);
}

// ---- weekly planning (REQ-027) ----------------------------------------------------

#[test]
fn req_027_ten_windows_six_percent() {
    // 50 h to the weekly reset, 40 % used: 10 windows and 6 % of the week per window.
    let plan = weekly(40.0, NOW + 50 * HOUR, NOW).unwrap();
    assert_eq!(plan.windows_left, 10);
    assert_close(plan.share_per_window_pct, 6.0);
}

#[test]
fn req_027_partial_window_rounds_up() {
    // 52 h are 10.4 windows of 5 h: the started window counts, so 11.
    let plan = weekly(40.0, NOW + 52 * HOUR, NOW).unwrap();
    assert_eq!(plan.windows_left, 11);
    assert_close(plan.share_per_window_pct, 60.0 / 11.0);
}

#[test]
fn req_027_one_second_left_is_one_window() {
    let plan = weekly(40.0, NOW + 1, NOW).unwrap();
    assert_eq!(plan.windows_left, 1);
    assert_close(plan.share_per_window_pct, 60.0);
}

#[test]
fn req_027_exactly_five_hours_are_one_window() {
    assert_eq!(weekly(0.0, NOW + 5 * HOUR, NOW).unwrap().windows_left, 1);
    assert_eq!(
        weekly(0.0, NOW + 5 * HOUR + 1, NOW).unwrap().windows_left,
        2
    );
}

#[test]
fn req_027_no_plan_after_reset() {
    assert_eq!(weekly(40.0, NOW, NOW), None);
    assert_eq!(weekly(40.0, NOW - 1, NOW), None);
}

#[test]
fn req_027_a_full_week_has_thirty_four_windows() {
    // 7 days are 33.6 windows of 5 h.
    let plan = weekly(0.0, NOW + 7 * 24 * HOUR, NOW).unwrap();
    assert_eq!(plan.windows_left, 34);
}

#[test]
fn req_027_nothing_left_means_a_share_of_zero() {
    assert_close(
        weekly(100.0, NOW + 10 * HOUR, NOW)
            .unwrap()
            .share_per_window_pct,
        0.0,
    );
    assert_close(
        weekly(130.0, NOW + 10 * HOUR, NOW)
            .unwrap()
            .share_per_window_pct,
        0.0,
    );
}
