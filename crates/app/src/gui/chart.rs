// SPDX-License-Identifier: Apache-2.0
//! The history chart of one window (concept §11.3, item 3): the samples of the current period,
//! the line of an even pace from the start of the period to the reset, and a vertical line at
//! now. Times on the horizontal axis are local times at whole hours or days. The data comes
//! from the view model.

use std::ops::RangeInclusive;

use chrono::{Local, TimeZone};
use cockpit_core::format;
use cockpit_core::viewmodel::ChartData;
use eframe::egui::{Color32, Ui};
use egui_plot::{GridInput, GridMark, Line, LineStyle, Plot, PlotPoints, Points, VLine};

use super::theme;

/// Height of a chart in logical pixels.
const HEIGHT: f32 = 150.0;
const HOUR_S: f64 = 3_600.0;
const DAY_S: f64 = 86_400.0;

/// Draws the chart; `id` must differ between the charts of one view, `color` is the colour of
/// the state of the window.
pub fn show(ui: &mut Ui, id: &str, chart: &ChartData, color: Color32) {
    let text_color = ui.visuals().text_color();
    let start = chart.target[0][0];
    let reset = chart.target[1][0];
    let samples: Vec<[f64; 2]> = chart.samples.clone();
    Plot::new(id)
        .height(HEIGHT)
        .allow_zoom(false)
        .allow_drag(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .allow_double_click_reset(false)
        .include_x(start)
        .include_x(reset)
        .include_y(0.0)
        .include_y(100.0)
        .default_y_bounds(0.0, 100.0)
        .x_grid_spacer(|input: GridInput| grid_marks(input.bounds, local_offset_s(input.bounds.0)))
        .y_grid_spacer(|_: GridInput| percent_marks())
        .x_axis_formatter(|mark: GridMark, _range: &RangeInclusive<f64>| axis_label(mark.value))
        .y_axis_formatter(|mark: GridMark, _range: &RangeInclusive<f64>| {
            format!("{:.0}%", mark.value)
        })
        .show(ui, |plot_ui| {
            plot_ui.line(
                Line::new("even pace", PlotPoints::from(chart.target.to_vec()))
                    .color(theme::GREY)
                    .width(1.5_f32)
                    .style(LineStyle::dashed_loose()),
            );
            plot_ui.vline(
                VLine::new("now", chart.now_s)
                    .color(text_color)
                    .width(1.0_f32),
            );
            if !samples.is_empty() {
                plot_ui.line(
                    Line::new("used", PlotPoints::from(samples.clone()))
                        .color(color)
                        .width(2.0_f32),
                );
                plot_ui.points(
                    Points::new("samples", PlotPoints::from(samples))
                        .color(color)
                        .radius(2.5_f32),
                );
            }
        });
}

/// Offset of the local time zone from UTC in seconds at the Unix time `at_s`.
fn local_offset_s(at_s: f64) -> f64 {
    Local
        .timestamp_opt(at_s as i64, 0)
        .single()
        .map_or(0.0, |time| f64::from(time.offset().local_minus_utc()))
}

/// Marks of the vertical axis: 0, 25, 50, 75 and 100 percent.
pub fn percent_marks() -> Vec<GridMark> {
    (0..=4)
        .map(|n| GridMark {
            value: f64::from(n) * 25.0,
            step_size: 25.0,
        })
        .collect()
}

/// The text on the time axis: weekday and local clock time, `Mon 14:00`.
pub fn axis_label(value_s: f64) -> String {
    format::local_time(value_s as i64, &Local)
}

/// Marks at whole local hours or days inside `bounds` (Unix seconds); `offset_s` is the offset of
/// the local time from UTC. The step is one hour for a span of up to 8 hours, six hours for up
/// to three days and a day beyond that, so a 5-hour window gets a mark every hour and a 7-day
/// window one a day.
pub fn grid_marks(bounds: (f64, f64), offset_s: f64) -> Vec<GridMark> {
    let (low, high) = bounds;
    let span = high - low;
    if !(low.is_finite() && high.is_finite()) || span <= 0.0 {
        return Vec::new();
    }
    let step = if span <= 8.0 * HOUR_S {
        HOUR_S
    } else if span <= 3.0 * DAY_S {
        6.0 * HOUR_S
    } else {
        DAY_S
    };
    // Whole steps of the local time, not of UTC.
    let first = ((low + offset_s) / step).ceil() * step - offset_s;
    let count = ((high - first) / step).floor();
    if !(0.0..=1_000.0).contains(&count) {
        return Vec::new();
    }
    (0..=count as u32)
        .map(|n| GridMark {
            value: first + f64::from(n) * step,
            step_size: step,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(marks: &[GridMark]) -> Vec<f64> {
        marks.iter().map(|m| m.value).collect()
    }

    #[test]
    fn req_030_a_five_hour_window_gets_a_mark_every_whole_hour() {
        // 10:30 to 15:30 UTC with UTC+1: marks at 12:00, 13:00, 14:00, 15:00, 16:00 local.
        let base = 1_738_368_000.0; // 2025-02-01 00:00 UTC
        let marks = grid_marks((base + 10.5 * 3_600.0, base + 15.5 * 3_600.0), 3_600.0);
        let local_hours: Vec<f64> = marks
            .iter()
            .map(|m| ((m.value + 3_600.0 - base) / 3_600.0) % 24.0)
            .collect();
        assert_eq!(local_hours, [12.0, 13.0, 14.0, 15.0, 16.0]);
        assert!(marks.iter().all(|m| m.step_size == 3_600.0));
    }

    #[test]
    fn req_030_a_seven_day_window_gets_a_mark_at_each_local_midnight() {
        let base = 1_738_368_000.0; // midnight UTC
        let marks = grid_marks((base + 3_600.0, base + 7.0 * 86_400.0 + 3_600.0), 7_200.0);
        assert_eq!(marks.len(), 7, "{:?}", values(&marks));
        for mark in &marks {
            let local = mark.value + 7_200.0;
            assert_eq!(local % 86_400.0, 0.0, "a mark is at local midnight");
            assert_eq!(mark.step_size, 86_400.0);
        }
    }

    #[test]
    fn req_030_the_step_follows_the_span() {
        let step = |hours: f64| grid_marks((0.0, hours * 3_600.0), 0.0)[0].step_size;
        assert_eq!(step(5.0), 3_600.0);
        assert_eq!(step(8.0), 3_600.0);
        assert_eq!(step(24.0), 6.0 * 3_600.0);
        assert_eq!(step(72.0), 6.0 * 3_600.0);
        assert_eq!(step(7.0 * 24.0), 86_400.0);
    }

    #[test]
    fn req_030_no_marks_for_an_empty_or_unusable_range() {
        assert!(grid_marks((5.0, 5.0), 0.0).is_empty());
        assert!(grid_marks((10.0, 5.0), 0.0).is_empty());
        assert!(grid_marks((f64::NAN, 5.0), 0.0).is_empty());
        assert!(grid_marks((0.0, f64::INFINITY), 0.0).is_empty());
        // Far too many marks would mean a broken range: nothing is drawn.
        assert!(grid_marks((0.0, 1.0e12), 0.0).is_empty());
    }

    #[test]
    fn req_030_the_vertical_axis_runs_from_0_to_100_percent() {
        let marks = percent_marks();
        assert_eq!(values(&marks), [0.0, 25.0, 50.0, 75.0, 100.0]);
    }

    #[test]
    fn req_030_the_axis_label_is_a_weekday_and_a_clock_time() {
        let label = axis_label(1_738_425_600.0);
        let parts: Vec<&str> = label.split(' ').collect();
        assert_eq!(parts.len(), 2, "{label}");
        assert_eq!(parts[0].len(), 3, "{label}");
        assert_eq!(parts[1].len(), 5, "{label}");
        assert_eq!(&parts[1][2..3], ":", "{label}");
    }
}
