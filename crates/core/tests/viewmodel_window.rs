// SPDX-License-Identifier: Apache-2.0
//! Per-window values and no-data texts of the view model. Times: the window resets at
//! Saturday 2025-02-01 16:00 UTC (17:00 in Berlin); `NOW` is 3 h before the reset, which is
//! 40 % of the 5-hour window (target 40 %).

use chrono_tz::Europe::Berlin;
use cockpit_core::metrics::PaceState;
use cockpit_core::model::{Record, WindowSample};
use cockpit_core::settings::Settings;
use cockpit_core::viewmodel::{
    BANNER_LOAD_ERROR, BANNER_NO_DATA_YET, BANNER_NO_LIMITS, Inputs, ViewModel, WINDOW_NO_DATA,
    WINDOW_RESET_PASSED, WindowData, WindowView, build, glyph, label,
};

const RESET_5H: i64 = 1_738_425_600;
const RESET_7D: i64 = RESET_5H + 3 * 86_400;
const NOW_S: i64 = RESET_5H - 3 * 3_600;
const NOW_MS: i64 = NOW_S * 1000;

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

fn view(records: &[Record], now_ms: i64, load_error: Option<&str>) -> ViewModel {
    let settings = Settings::default();
    build(&Inputs {
        now_ms,
        records,
        last_error_ms: None,
        load_error,
        settings: &settings,
        stats: None,
        tz: &Berlin,
    })
}

fn data(window: &WindowView) -> &WindowData {
    match window {
        WindowView::Data(data) => data,
        WindowView::NoData { text } => panic!("expected values, got no data: {text}"),
    }
}

fn no_data_text(window: &WindowView) -> &str {
    match window {
        WindowView::NoData { text } => text,
        WindowView::Data(_) => panic!("expected no data"),
    }
}

#[test]
fn req_016_banner_without_latest() {
    let vm = view(&[], NOW_MS, None);
    assert_eq!(vm.banner.as_deref(), Some(BANNER_NO_DATA_YET));
    assert_eq!(
        BANNER_NO_DATA_YET,
        "No data yet. Set up the bridge and use Claude Code once."
    );
    assert_eq!(no_data_text(&vm.five_hour), WINDOW_NO_DATA);
    assert_eq!(no_data_text(&vm.seven_day), WINDOW_NO_DATA);
}

#[test]
fn req_016_banner_no_limits() {
    let vm = view(&[record(NOW_S, None, None)], NOW_MS, None);
    assert_eq!(vm.banner.as_deref(), Some(BANNER_NO_LIMITS));
    assert!(BANNER_NO_LIMITS.starts_with("Claude Code sent no usage limits."));
    assert_eq!(no_data_text(&vm.five_hour), WINDOW_NO_DATA);
}

#[test]
fn req_016_banner_load_error() {
    let vm = view(&[], NOW_MS, Some("C:/data"));
    assert_eq!(
        vm.banner.as_deref(),
        Some("Cannot read the data folder: C:/data")
    );
    assert!(BANNER_LOAD_ERROR.ends_with(": "));
    // The error wins over the "no data yet" message.
    let with_records = view(
        &[record(NOW_S, sample(10.0, RESET_5H), None)],
        NOW_MS,
        Some("x"),
    );
    assert!(with_records.banner.unwrap().starts_with(BANNER_LOAD_ERROR));
}

#[test]
fn req_016_one_window_missing_gives_no_data_for_that_row_only() {
    let vm = view(&[record(NOW_S, sample(60.0, RESET_5H), None)], NOW_MS, None);
    assert_eq!(vm.banner, None);
    assert!(matches!(vm.five_hour, WindowView::Data(_)));
    assert_eq!(no_data_text(&vm.seven_day), "no data");
}

#[test]
fn req_022_reset_passed_text() {
    let records = [record(
        NOW_S,
        sample(60.0, RESET_5H),
        sample(30.0, RESET_7D),
    )];
    let after_reset = (RESET_5H + 60) * 1000;
    let vm = view(&records, after_reset, None);
    assert_eq!(no_data_text(&vm.five_hour), WINDOW_RESET_PASSED);
    assert_eq!(
        WINDOW_RESET_PASSED,
        "Window has reset. Nothing reported since; it fills again with the next report from Claude Code."
    );
    assert!(
        matches!(vm.seven_day, WindowView::Data(_)),
        "the other window goes on"
    );
    assert_eq!(vm.banner, None);
    // Exactly at the reset the window counts as reset as well.
    let at_reset = view(&records, RESET_5H * 1000, None);
    assert_eq!(no_data_text(&at_reset.five_hour), WINDOW_RESET_PASSED);
}

#[test]
fn req_003_state_has_glyph_and_label() {
    let state_of = |used: f64| {
        let vm = view(&[record(NOW_S, sample(used, RESET_5H), None)], NOW_MS, None);
        let window = data(&vm.five_hour).clone();
        (window.state, window.glyph, window.label)
    };
    assert_eq!(state_of(20.0), (PaceState::Under, "▼", "under"));
    assert_eq!(state_of(40.0), (PaceState::On, "●", "on pace"));
    assert_eq!(state_of(62.0), (PaceState::Over, "▲", "over"));
    // The edge of the tolerance band (5 pp) still counts as on pace.
    assert_eq!(state_of(45.0).0, PaceState::On);
    assert_eq!(state_of(35.0).0, PaceState::On);
}

#[test]
fn req_004_window_texts() {
    let vm = view(&[record(NOW_S, sample(60.0, RESET_5H), None)], NOW_MS, None);
    let w = data(&vm.five_hour);
    assert_eq!(w.used, 60.0);
    assert!((w.target - 40.0).abs() < 1e-9);
    assert_eq!(w.used_text, "60.0%");
    assert_eq!(w.remaining_text, "40.0%");
    assert_eq!(w.deviation_text, "+20.0 pp");
    assert_eq!(w.factor_text, "1.50×");
    assert_eq!(w.countdown_text, "3h 0m");
    assert_eq!(w.reset_local_text, "Sat 17:00");
    assert_eq!(w.recommended_text, "13.3 %/h");
    assert!(!w.binding);
    // One record gives no usage rate and therefore no forecast.
    assert_eq!(w.rate_text, "not available");
    assert_eq!(w.forecast_text, "not available");
    assert_eq!(w.unused_text, "not available");
}

#[test]
fn req_005_forecast_texts_with_a_rate() {
    // 50 %, 55 %, 60 % within the last 30 minutes: 20 percent per hour.
    let records = [
        record(NOW_S - 1800, sample(50.0, RESET_5H), None),
        record(NOW_S - 900, sample(55.0, RESET_5H), None),
        record(NOW_S, sample(60.0, RESET_5H), None),
    ];
    let vm = view(&records, NOW_MS, None);
    let w = data(&vm.five_hour);
    assert_eq!(w.rate_text, "20.0 %/h");
    // 40 % left at 20 %/h lasts 2 hours, the reset is in 3 hours.
    assert_eq!(w.forecast_text, "limit in 2h 0m, before reset");
    // 60 % + 3 h * 20 %/h is more than 100 %: nothing remains unused.
    assert_eq!(w.unused_text, "0.0% unused at reset");
}

#[test]
fn req_006_unused_remainder_and_reset_first() {
    // 20 % now, 5 percent per hour: 20 + 15 = 35 % used at the reset, 65 % unused.
    let records = [
        record(NOW_S - 1800, sample(17.5, RESET_5H), None),
        record(NOW_S, sample(20.0, RESET_5H), None),
    ];
    let vm = view(&records, NOW_MS, None);
    let w = data(&vm.five_hour);
    assert_eq!(w.rate_text, "5.0 %/h");
    assert_eq!(w.forecast_text, "reset first");
    assert_eq!(w.unused_text, "65.0% unused at reset");
}

#[test]
fn req_004_a_limit_that_is_reached_is_shown_as_such() {
    let vm = view(
        &[record(NOW_S, sample(120.0, RESET_5H), None)],
        NOW_MS,
        None,
    );
    let w = data(&vm.five_hour);
    assert_eq!(w.used, 100.0, "values above 100 are clamped");
    assert_eq!(w.used_text, "100.0%");
    assert_eq!(w.remaining_text, "0.0%");
}

#[test]
fn req_016_the_latest_record_of_any_session_decides() {
    let mut older = record(NOW_S - 600, sample(10.0, RESET_5H), None);
    older.session_id = Some("a".to_owned());
    let mut newer = record(NOW_S, sample(60.0, RESET_5H), None);
    newer.session_id = Some("b".to_owned());
    let vm = view(&[older, newer], NOW_MS, None);
    assert_eq!(data(&vm.five_hour).used_text, "60.0%");
}

#[test]
fn req_114_every_state_has_text() {
    let states = [PaceState::Under, PaceState::On, PaceState::Over];
    let glyphs: Vec<_> = states.iter().map(|s| glyph(*s)).collect();
    let labels: Vec<_> = states.iter().map(|s| label(*s)).collect();
    assert_eq!(glyphs, ["▼", "●", "▲"]);
    assert_eq!(labels, ["under", "on pace", "over"]);
    for (g, l) in glyphs.iter().zip(&labels) {
        assert!(
            !g.is_empty() && !l.is_empty(),
            "colour is never the only carrier"
        );
    }
}

#[test]
fn req_016_later_parts_have_not_available_defaults() {
    let vm = view(&[record(NOW_S, sample(60.0, RESET_5H), None)], NOW_MS, None);
    assert_eq!(vm.binding, None);
    assert_eq!(vm.weekly_text, None);
    assert_eq!(vm.estimate_text, None);
    assert!(!vm.stale);
    assert!(vm.previous.is_empty());
}

#[test]
fn req_030_chart_contains_target_line_endpoints() {
    let vm = view(&[record(NOW_S, sample(60.0, RESET_5H), None)], NOW_MS, None);
    let chart = &data(&vm.five_hour).chart;
    let start = (RESET_5H - 18_000) as f64;
    assert_eq!(chart.target, [[start, 0.0], [RESET_5H as f64, 100.0]]);
    assert_eq!(chart.now_s, NOW_S as f64);
    // The seven-day window has its own line over seven days.
    let seven = view(
        &[record(NOW_S, None, sample(10.0, RESET_5H + 100_000))],
        NOW_MS,
        None,
    );
    let chart7 = &data(&seven.seven_day).chart;
    assert_eq!(
        chart7.target[0],
        [(RESET_5H + 100_000 - 604_800) as f64, 0.0]
    );
    assert_eq!(chart7.target[1], [(RESET_5H + 100_000) as f64, 100.0]);
}

#[test]
fn req_030_chart_samples_are_those_of_the_current_period_only() {
    let old_reset = RESET_5H - 2 * 18_000;
    let records = [
        record(old_reset - 600, sample(90.0, old_reset), None),
        record(NOW_S - 1_800, sample(10.0, RESET_5H), None),
        record(NOW_S - 900, sample(120.0, RESET_5H), None),
        record(NOW_S, sample(30.0, RESET_5H), None),
    ];
    let vm = view(&records, NOW_MS, None);
    let samples = &data(&vm.five_hour).chart.samples;
    assert_eq!(
        samples,
        &vec![
            [(NOW_S - 1_800) as f64, 10.0],
            [(NOW_S - 900) as f64, 100.0],
            [NOW_S as f64, 30.0]
        ],
        "the older period is left out, 120 % is drawn as 100 %"
    );
}
