// SPDX-License-Identifier: Apache-2.0
//! The view model (concept §11.4): every string, number and state that the views draw.
//!
//! The window code only lays these out; all decisions are made and tested here. This part holds
//! the types and the per-window values with the no-data texts; binding limit, weekly plan, data
//! age, session, transcripts and previous periods are filled by later parts and carry their
//! "not available" defaults until then. No GUI types appear here.

use std::fmt::Display;

use chrono::TimeZone;

use crate::format;
use crate::metrics::{self, Forecast, PaceState};
use crate::model::{Record, WindowKind};
use crate::periods::{self, Period};
use crate::planning;
use crate::settings::Settings;
use crate::transcripts::Stats;

/// Banner when there is no record yet (concept §11.7).
pub const BANNER_NO_DATA_YET: &str = "No data yet. Set up the bridge and use Claude Code once.";
/// Banner when the latest record holds neither window.
pub const BANNER_NO_LIMITS: &str = "Claude Code sent no usage limits. They appear only for Pro and Max subscriptions, after the first response of a session.";
/// Start of the banner when the data folder cannot be read; the path follows.
pub const BANNER_LOAD_ERROR: &str = "Cannot read the data folder: ";
/// Text of a window that has no value in the latest record.
pub const WINDOW_NO_DATA: &str = "no data";
/// Text of a window whose reset has passed without a newer record.
pub const WINDOW_RESET_PASSED: &str = "Window reset. Waiting for new data from Claude Code.";

/// Everything the view model is built from.
pub struct Inputs<'a, Tz: TimeZone> {
    /// Now, Unix milliseconds.
    pub now_ms: i64,
    /// The full history in received order.
    pub records: &'a [Record],
    /// Time of the newest malformed input (`last_error.json`), if any.
    pub last_error_ms: Option<i64>,
    /// Why the data could not be loaded (for example the data folder), if that happened.
    pub load_error: Option<&'a str>,
    /// The user settings.
    pub settings: &'a Settings,
    /// Transcript statistics, if there are any.
    pub stats: Option<&'a Stats>,
    /// The time zone for local times.
    pub tz: &'a Tz,
}

/// What the views draw.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewModel {
    /// A message for the whole application instead of values, if there is one.
    pub banner: Option<String>,
    /// The 5-hour window.
    pub five_hour: WindowView,
    /// The 7-day window.
    pub seven_day: WindowView,
    /// The window whose limit binds first.
    pub binding: Option<WindowKind>,
    /// Weekly plan text.
    pub weekly_text: Option<String>,
    /// Data age or stale marker text.
    pub age_text: String,
    /// Whether the data counts as stale.
    pub stale: bool,
    /// Details of the session (part 3).
    pub session: SessionView,
    /// Transcript statistics (part 3).
    pub transcripts: TranscriptView,
    /// Tokens per percentage point, labelled as an estimate (part 3).
    pub estimate_text: Option<String>,
    /// The last finished periods, per window newest first, five-hour window before seven-day.
    pub previous: Vec<PreviousPeriod>,
}

/// One window: values or the reason why there are none.
// The shape `Data(WindowData)` is fixed by the concept; two of these are built per redraw.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum WindowView {
    /// There is nothing to show; `text` says why.
    NoData {
        /// Plain-language reason.
        text: String,
    },
    /// The values of the window.
    Data(WindowData),
}

/// The values and texts of one window.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowData {
    /// Used share in percent, 0 to 100 (for the bar).
    pub used: f64,
    /// Target share in percent, 0 to 100 (for the marker on the bar).
    pub target: f64,
    /// Under, on or over pace.
    pub state: PaceState,
    /// Glyph of the state (concept §11.1).
    pub glyph: &'static str,
    /// Text of the state (concept §11.1).
    pub label: &'static str,
    /// Whether this window's limit binds first.
    pub binding: bool,
    /// `62.0%`.
    pub used_text: String,
    /// `38.0%`.
    pub remaining_text: String,
    /// Reset time in the local time zone, `Mon 14:30`.
    pub reset_local_text: String,
    /// Time until the reset, `1h 12m`.
    pub countdown_text: String,
    /// `+20.0 pp`.
    pub deviation_text: String,
    /// `1.50×` or `–`.
    pub factor_text: String,
    /// `20.0 %/h` or `not available`.
    pub rate_text: String,
    /// `limit in 3h 0m, before reset` and the other forecast texts.
    pub forecast_text: String,
    /// `40.0% unused at reset` or `not available`.
    pub unused_text: String,
    /// The rate that uses up the quota exactly at the reset, `10.0 %/h`.
    pub recommended_text: String,
}

/// Session details from the latest record (filled in part 3).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionView {}

/// Transcript statistics for the detailed view (filled in part 3).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TranscriptView {}

/// A finished period of a window.
#[derive(Debug, Clone, PartialEq)]
pub struct PreviousPeriod {
    /// Which window the period belongs to.
    pub kind: WindowKind,
    /// Reset time that ended the period, in the local time zone, `Mon 14:30`.
    pub reset_local_text: String,
    /// Used share at the end of the period (last sample), `83.0%`.
    pub final_used_text: String,
}

/// How many finished periods are listed per window.
pub const PREVIOUS_PERIODS_PER_WINDOW: usize = 3;

/// Glyph of a pace state (concept §11.1).
pub fn glyph(state: PaceState) -> &'static str {
    match state {
        PaceState::Under => "▼",
        PaceState::On => "●",
        PaceState::Over => "▲",
    }
}

/// Text of a pace state (concept §11.1).
pub fn label(state: PaceState) -> &'static str {
    match state {
        PaceState::Under => "under",
        PaceState::On => "on pace",
        PaceState::Over => "over",
    }
}

/// Builds the view model.
pub fn build<Tz: TimeZone>(inputs: &Inputs<'_, Tz>) -> ViewModel
where
    Tz::Offset: Display,
{
    let (mut five_hour, five_forecast) = window_view(inputs, WindowKind::FiveHour);
    let (mut seven_day, seven_forecast) = window_view(inputs, WindowKind::SevenDay);
    let binding = metrics::binding(five_forecast.as_ref(), seven_forecast.as_ref());
    mark_binding(&mut five_hour, binding == Some(WindowKind::FiveHour));
    mark_binding(&mut seven_day, binding == Some(WindowKind::SevenDay));
    let (age_text, stale) = age(inputs);
    ViewModel {
        banner: banner(inputs),
        weekly_text: weekly_text(inputs, &five_hour, &seven_day),
        five_hour,
        seven_day,
        binding,
        age_text,
        stale,
        session: SessionView::default(),
        transcripts: TranscriptView::default(),
        estimate_text: None,
        previous: previous_periods(inputs),
    }
}

fn mark_binding(window: &mut WindowView, binding: bool) {
    if let WindowView::Data(data) = window {
        data.binding = binding;
    }
}

/// Age of the newest record as text, and whether the data counts as stale (concept §7.9);
/// without any record the text is empty.
fn age<Tz: TimeZone>(inputs: &Inputs<'_, Tz>) -> (String, bool) {
    let Some(latest) = metrics::latest(inputs.records) else {
        return (String::new(), false);
    };
    let age_s = metrics::data_age_s(latest, inputs.now_ms);
    let stale = metrics::is_stale(
        age_s,
        inputs.settings.stale_after_s,
        inputs.last_error_ms,
        latest.received_at_ms,
    );
    let text = if stale {
        format::stale(age_s)
    } else {
        format::age(age_s)
    };
    (text, stale)
}

/// The weekly plan text; only with values for both windows (concept §7.8).
fn weekly_text<Tz: TimeZone>(
    inputs: &Inputs<'_, Tz>,
    five_hour: &WindowView,
    seven_day: &WindowView,
) -> Option<String> {
    if !matches!(
        (five_hour, seven_day),
        (WindowView::Data(_), WindowView::Data(_))
    ) {
        return None;
    }
    let sample = metrics::latest(inputs.records)?.seven_day.as_ref()?;
    let plan = planning::weekly(
        sample.used_pct,
        sample.resets_at,
        inputs.now_ms.div_euclid(1000),
    )?;
    Some(format::weekly(plan.windows_left, plan.share_per_window_pct))
}

/// The last finished periods of each window, newest first, five-hour window before seven-day.
fn previous_periods<Tz: TimeZone>(inputs: &Inputs<'_, Tz>) -> Vec<PreviousPeriod>
where
    Tz::Offset: Display,
{
    let now_s = inputs.now_ms.div_euclid(1000);
    [WindowKind::FiveHour, WindowKind::SevenDay]
        .into_iter()
        .flat_map(|kind| {
            let finished: Vec<Period> = periods::split(inputs.records, kind)
                .into_iter()
                .filter(|p| p.resets_at <= now_s)
                .collect();
            finished
                .into_iter()
                .rev()
                .take(PREVIOUS_PERIODS_PER_WINDOW)
                .filter_map(|period| {
                    let final_used = period.samples.last()?.1.clamp(0.0, 100.0);
                    Some(PreviousPeriod {
                        kind,
                        reset_local_text: format::local_time(period.resets_at, inputs.tz),
                        final_used_text: format::pct(final_used),
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The message for the whole application, in the order of concept §11.7: a data folder that
/// cannot be read, no record yet, a latest record without any window.
fn banner<Tz: TimeZone>(inputs: &Inputs<'_, Tz>) -> Option<String> {
    if let Some(error) = inputs.load_error {
        return Some(format!("{BANNER_LOAD_ERROR}{error}"));
    }
    let Some(latest) = metrics::latest(inputs.records) else {
        return Some(BANNER_NO_DATA_YET.to_owned());
    };
    (latest.five_hour.is_none() && latest.seven_day.is_none()).then(|| BANNER_NO_LIMITS.to_owned())
}

fn no_data(text: &str) -> WindowView {
    WindowView::NoData {
        text: text.to_owned(),
    }
}

/// One window from the latest record and the current period of that window.
///
/// Also returns the exhaustion forecast, which decides the binding limit.
fn window_view<Tz: TimeZone>(
    inputs: &Inputs<'_, Tz>,
    kind: WindowKind,
) -> (WindowView, Option<Forecast>)
where
    Tz::Offset: Display,
{
    let Some(sample) = metrics::latest(inputs.records).and_then(|r| r.window(kind)) else {
        return (no_data(WINDOW_NO_DATA), None);
    };
    let now_s = inputs.now_ms.div_euclid(1000);
    if sample.resets_at <= now_s {
        return (no_data(WINDOW_RESET_PASSED), None);
    }
    let settings = inputs.settings;
    let basic = metrics::basic(
        sample.used_pct,
        sample.resets_at,
        metrics::window_len_s(kind),
        now_s,
        settings.tolerance_pp,
    );
    let all_periods = periods::split(inputs.records, kind);
    let rate = periods::current(&all_periods, now_s)
        .and_then(|period| metrics::rate(period, now_s, i64::from(settings.rate_period_s)));
    let forecast = metrics::forecast(basic.used, sample.resets_at, rate, now_s);
    let unused = metrics::unused_at_reset(basic.used, sample.resets_at, rate, now_s);
    let recommended = metrics::recommended_rate(basic.used, sample.resets_at, now_s);
    let view = WindowView::Data(WindowData {
        used: basic.used,
        target: basic.target,
        state: basic.state,
        glyph: glyph(basic.state),
        label: label(basic.state),
        binding: false,
        used_text: format::pct(basic.used),
        remaining_text: format::pct(basic.remaining),
        reset_local_text: format::local_time(sample.resets_at, inputs.tz),
        countdown_text: format::duration(basic.secs_to_reset),
        deviation_text: format::deviation(basic.deviation_pp),
        factor_text: format::factor(basic.pace_factor),
        rate_text: format::rate(rate),
        forecast_text: format::forecast(&forecast, now_s),
        unused_text: format::unused(unused),
        recommended_text: format::rate(recommended),
    });
    (view, Some(forecast))
}
