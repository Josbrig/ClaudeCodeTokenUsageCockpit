// SPDX-License-Identifier: Apache-2.0
//! Token counts from the transcripts in a window row that has no percentage (REQ-120).

use chrono_tz::Europe::Berlin;
use cockpit_core::model::{Record, WindowSample};
use cockpit_core::settings::Settings;
use cockpit_core::transcripts::Stats;
use cockpit_core::viewmodel::{
    Inputs, ViewModel, WINDOW_NO_DATA, WINDOW_RESET_PASSED, WindowView, build,
};

const NOW_MS: i64 = 1_738_425_600_000;
const HOUR_MS: i64 = 3_600_000;

fn stats(points: &[(i64, u64)]) -> Stats {
    Stats {
        understood_lines: points.len(),
        token_points: points.to_vec(),
        ..Stats::default()
    }
}

fn expired_record() -> Record {
    let sample = Some(WindowSample {
        used_pct: 60.0,
        resets_at: NOW_MS / 1000 - 600,
    });
    Record {
        received_at_ms: NOW_MS - 13 * HOUR_MS,
        session_id: None,
        cc_version: None,
        five_hour: sample.clone(),
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

fn text(window: &WindowView) -> &str {
    match window {
        WindowView::NoData { text } => text,
        WindowView::Data(_) => panic!("expected no data"),
    }
}

#[test]
fn req_120_expired_window_names_the_tokens_of_the_last_five_hours() {
    let stats = stats(&[
        (NOW_MS - 6 * HOUR_MS, 9_000),
        (NOW_MS - 4 * HOUR_MS, 1_000),
        (NOW_MS - HOUR_MS, 234),
    ]);
    let vm = view(&[expired_record()], Some(&stats));
    let row = text(&vm.five_hour);
    assert_eq!(row, "1,234 tokens in 5 h");
}

#[test]
fn req_120_no_record_at_all_names_the_tokens_too() {
    let stats = stats(&[(NOW_MS - HOUR_MS, 500)]);
    let vm = view(&[], Some(&stats));
    assert_ne!(text(&vm.five_hour), WINDOW_NO_DATA);
    assert_eq!(text(&vm.five_hour), "500 tokens in 5 h");
    assert_eq!(text(&vm.seven_day), "500 tokens in 7 d");
}

#[test]
fn req_120_without_tokens_in_range_the_text_stays() {
    let old = stats(&[(NOW_MS - 30 * HOUR_MS, 500)]);
    assert_eq!(
        text(&view(&[expired_record()], Some(&old)).five_hour),
        WINDOW_RESET_PASSED
    );
    assert_eq!(
        text(&view(&[expired_record()], None).five_hour),
        WINDOW_RESET_PASSED
    );
    let not_understood = Stats::default();
    assert_eq!(
        text(&view(&[expired_record()], Some(&not_understood)).five_hour),
        WINDOW_RESET_PASSED
    );
}
