// SPDX-License-Identifier: Apache-2.0
//! Bar charts of the transcript statistics (REQ-014 shown as a picture): the tokens per day.
//! The numbers come from the view model; the choice of what the bars show (the four token
//! kinds differ by orders of magnitude) is kept while the window is open.

use std::ops::RangeInclusive;

use cockpit_core::viewmodel::TOKEN_KINDS;
use eframe::egui::{Id, RichText, Ui};
use egui_plot::{Bar, BarChart, GridInput, GridMark, Plot};

use super::theme;

/// Height of a chart in logical pixels.
const HEIGHT: f32 = 150.0;
/// About this many day labels are shown, so that they do not overlap.
const LABELS: usize = 7;

/// What the bars show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Output tokens (the default).
    Output,
    /// Input tokens that were not served from the cache.
    Input,
    /// Input tokens written to the cache.
    CacheWrite,
    /// Input tokens read from the cache.
    CacheRead,
}

impl Kind {
    /// The order of the choice in the dialog.
    pub const ALL: [Kind; 4] = [Kind::Output, Kind::Input, Kind::CacheWrite, Kind::CacheRead];

    /// Position in the numbers of the view model ([`TOKEN_KINDS`]).
    pub fn index(self) -> usize {
        match self {
            Kind::Input => 0,
            Kind::Output => 1,
            Kind::CacheWrite => 2,
            Kind::CacheRead => 3,
        }
    }

    /// The name as shown, the same as the column of the table.
    pub fn label(self) -> &'static str {
        TOKEN_KINDS[self.index()]
    }
}

/// The row of choices; returns the kind chosen now. The choice lives in the memory of the
/// window, one for all charts.
pub fn choice(ui: &mut Ui) -> Kind {
    let id = Id::new("token_chart_kind");
    let mut kind = ui
        .data(|data| data.get_temp::<Kind>(id))
        .unwrap_or(Kind::Output);
    ui.horizontal_wrapped(|ui| {
        ui.label("Chart shows:");
        for option in Kind::ALL {
            ui.radio_value(&mut kind, option, option.label());
        }
    });
    ui.data_mut(|data| data.insert_temp(id, kind));
    kind
}

/// Draws the bars of the tokens per day. `days` is newest first as in the table; the chart has
/// the oldest day on the left.
pub fn days(ui: &mut Ui, id: &str, days: &[(String, [u64; 4])], kind: Kind) {
    let count = days.len();
    let ordered: Vec<&(String, [u64; 4])> = days.iter().rev().collect();
    let bars: Vec<Bar> = ordered
        .iter()
        .enumerate()
        .map(|(position, (day, tokens))| {
            Bar::new(position as f64, tokens[kind.index()] as f64)
                .width(0.7)
                .name(day.as_str())
        })
        .collect();
    let labels: Vec<String> = ordered.iter().map(|(day, _)| day_label(day)).collect();
    let step = label_step(count);
    ui.label(RichText::new(format!("{} tokens per day", kind.label())).strong());
    Plot::new(id)
        .height(HEIGHT)
        .allow_zoom(false)
        .allow_drag(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .allow_double_click_reset(false)
        .include_x(-0.6)
        .include_x(count as f64 - 0.4)
        .include_y(0.0)
        .x_grid_spacer(move |_: GridInput| day_marks(count, step))
        .x_axis_formatter(move |mark: GridMark, _range: &RangeInclusive<f64>| {
            let position = mark.value.round();
            if (mark.value - position).abs() < 1e-6 && position >= 0.0 {
                labels.get(position as usize).cloned().unwrap_or_default()
            } else {
                String::new()
            }
        })
        .y_axis_formatter(|mark: GridMark, _range: &RangeInclusive<f64>| short_count(mark.value))
        .show(ui, |plot_ui| {
            plot_ui.bar_chart(BarChart::new(kind.label(), bars).color(theme::UNDER));
        });
}

/// `2026-10-08` as `10-08`; text that is not a date is shown as it is.
pub fn day_label(day: &str) -> String {
    match day.split_once('-') {
        Some((year, rest)) if year.len() == 4 && year.chars().all(|c| c.is_ascii_digit()) => {
            rest.to_owned()
        }
        _ => day.to_owned(),
    }
}

/// Every how many days a label is shown, for `count` days.
pub fn label_step(count: usize) -> usize {
    count.div_ceil(LABELS).max(1)
}

/// Marks at the positions that get a label: counted from the newest day, so that the newest
/// bar is always labelled.
pub fn day_marks(count: usize, step: usize) -> Vec<GridMark> {
    if count == 0 || step == 0 {
        return Vec::new();
    }
    (0..count)
        .filter(|position| (count - 1 - position).checked_rem(step) == Some(0))
        .map(|position| GridMark {
            value: position as f64,
            step_size: step as f64,
        })
        .collect()
}

/// A large number in short form: `950`, `1.2 K`, `3.4 M`, `5.6 B`.
pub fn short_count(value: f64) -> String {
    let magnitude = value.abs();
    let (scaled, suffix) = if magnitude >= 1e12 {
        (value / 1e12, " T")
    } else if magnitude >= 1e9 {
        (value / 1e9, " B")
    } else if magnitude >= 1e6 {
        (value / 1e6, " M")
    } else if magnitude >= 1e3 {
        (value / 1e3, " K")
    } else {
        return format!("{value:.0}");
    };
    // Whole numbers without a decimal, otherwise one: `3 M`, `1.2 K`.
    if (scaled - scaled.round()).abs() < 0.05 {
        format!("{:.0}{suffix}", scaled)
    } else {
        format!("{scaled:.1}{suffix}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_014_short_count_names_thousands_millions_and_billions() {
        assert_eq!(short_count(0.0), "0");
        assert_eq!(short_count(950.0), "950");
        assert_eq!(short_count(1_000.0), "1 K");
        assert_eq!(short_count(1_234.0), "1.2 K");
        assert_eq!(short_count(3_400_000.0), "3.4 M");
        assert_eq!(short_count(2_000_000.0), "2 M");
        assert_eq!(short_count(8_627_482_138.0), "8.6 B");
        assert_eq!(short_count(2.5e12), "2.5 T");
        assert_eq!(short_count(-1_500.0), "-1.5 K");
    }

    #[test]
    fn req_014_day_label_drops_the_year() {
        assert_eq!(day_label("2026-10-08"), "10-08");
        assert_eq!(day_label("whatever"), "whatever");
        assert_eq!(day_label("x-y"), "x-y");
    }

    #[test]
    fn req_014_the_newest_day_is_always_labelled() {
        assert_eq!(label_step(0), 1);
        assert_eq!(label_step(7), 1);
        assert_eq!(label_step(8), 2);
        assert_eq!(label_step(35), 5);
        let positions = |count, step| -> Vec<usize> {
            day_marks(count, step)
                .iter()
                .map(|mark| mark.value as usize)
                .collect()
        };
        assert_eq!(positions(35, 5), [4, 9, 14, 19, 24, 29, 34]);
        assert_eq!(positions(3, 1), [0, 1, 2]);
        assert_eq!(positions(1, 1), [0]);
        assert!(day_marks(0, 1).is_empty());
    }

    #[test]
    fn req_014_the_kinds_follow_the_order_of_the_view_model() {
        assert_eq!(Kind::Input.label(), "Input");
        assert_eq!(Kind::Output.label(), "Output");
        assert_eq!(Kind::CacheWrite.label(), "Cache write");
        assert_eq!(Kind::CacheRead.label(), "Cache read");
        assert_eq!(Kind::ALL[0], Kind::Output, "the default comes first");
    }
}
