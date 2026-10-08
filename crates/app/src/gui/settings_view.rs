// SPDX-License-Identifier: Apache-2.0
//! The settings dialog (concept §9, §11.3): the values are edited as text, checked against the
//! ranges of the settings module, and only a valid set can be saved. The checking is plain
//! functions on values, so it can be tested without a window.

use std::ops::RangeInclusive;

use cockpit_core::settings::{
    RATE_PERIOD_RANGE, STALE_AFTER_RANGE, Settings, StartView, TOLERANCE_RANGE,
};
use eframe::egui::{self, Color32, Context, TextEdit, Ui};

use crate::autostart::{LABEL as AUTOSTART_LABEL, SWITCHED_OFF_TEXT, State as AutostartState};

/// What the person did in the dialog.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    /// Still open, nothing decided.
    Open,
    /// "Save" with a valid set of values.
    Save(Values),
    /// "Cancel" or the close button.
    Cancel,
    /// The start entry of the system was switched on (`true`) or off at once; unlike the other
    /// values it does not wait for "Save", because it is not part of the settings file.
    Autostart(bool),
}

/// A valid set of the values the dialog edits.
#[derive(Debug, Clone, PartialEq)]
pub struct Values {
    /// Tolerance band for the pace state, percentage points.
    pub tolerance_pp: f64,
    /// Seconds after which data counts as stale.
    pub stale_after_s: u32,
    /// Seconds over which the usage rate is computed.
    pub rate_period_s: u32,
    /// Keep the window on top of other windows.
    pub always_on_top: bool,
    /// View shown at start.
    pub start_view: StartView,
}

impl Values {
    /// Takes the values over into `settings`; everything else in `settings` stays.
    pub fn apply_to(&self, settings: &mut Settings) {
        settings.tolerance_pp = self.tolerance_pp;
        settings.stale_after_s = self.stale_after_s;
        settings.rate_period_s = self.rate_period_s;
        settings.always_on_top = self.always_on_top;
        settings.start_view = self.start_view;
    }
}

/// The text of the fields and the other choices while the dialog is open.
#[derive(Debug, Clone, PartialEq)]
pub struct Draft {
    /// Tolerance band as typed.
    pub tolerance: String,
    /// Stale threshold in seconds as typed.
    pub stale_after: String,
    /// Rate period in seconds as typed.
    pub rate_period: String,
    /// Always on top.
    pub always_on_top: bool,
    /// Start view.
    pub start_view: StartView,
    /// Why the last attempt to save failed, shown in the dialog until the next attempt.
    pub save_error: Option<String>,
    /// The real state of the start entry; `None` where the system has no such switch.
    pub autostart: Option<AutostartState>,
    /// Why the start entry could not be read or changed.
    pub autostart_error: Option<String>,
}

/// Which field has a problem and what it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldError {
    /// Name of the field as shown in the dialog.
    pub field: &'static str,
    /// Plain-language explanation, with the allowed range.
    pub message: String,
}

impl Draft {
    /// The draft that shows the current settings.
    pub fn from_settings(settings: &Settings) -> Self {
        Self {
            tolerance: settings.tolerance_pp.to_string(),
            stale_after: settings.stale_after_s.to_string(),
            rate_period: settings.rate_period_s.to_string(),
            always_on_top: settings.always_on_top,
            start_view: settings.start_view,
            save_error: None,
            autostart: None,
            autostart_error: None,
        }
    }

    /// All problems of the draft, or the valid values. Nothing is clamped or corrected: a value
    /// outside its range is an error the person sees.
    pub fn validate(&self) -> Result<Values, Vec<FieldError>> {
        let tolerance = parse_decimal(&self.tolerance, &TOLERANCE_RANGE, "Tolerance", "points");
        let stale = parse_whole(
            &self.stale_after,
            &STALE_AFTER_RANGE,
            "Stale after",
            "seconds",
        );
        let rate = parse_whole(
            &self.rate_period,
            &RATE_PERIOD_RANGE,
            "Rate period",
            "seconds",
        );
        match (tolerance, stale, rate) {
            (Ok(tolerance_pp), Ok(stale_after_s), Ok(rate_period_s)) => Ok(Values {
                tolerance_pp,
                stale_after_s,
                rate_period_s,
                always_on_top: self.always_on_top,
                start_view: self.start_view,
            }),
            (tolerance, stale, rate) => Err([tolerance.err(), stale.err(), rate.err()]
                .into_iter()
                .flatten()
                .collect()),
        }
    }
}

/// A decimal number, a comma or a point as separator, inside `range`.
fn parse_decimal(
    text: &str,
    range: &RangeInclusive<f64>,
    field: &'static str,
    unit: &str,
) -> Result<f64, FieldError> {
    let error = || FieldError {
        field,
        message: format!(
            "{field} must be a number from {} to {} {unit}.",
            range.start(),
            range.end()
        ),
    };
    let value: f64 = text.trim().replace(',', ".").parse().map_err(|_| error())?;
    (value.is_finite() && range.contains(&value))
        .then_some(value)
        .ok_or_else(error)
}

/// A whole number inside `range`.
fn parse_whole(
    text: &str,
    range: &RangeInclusive<u32>,
    field: &'static str,
    unit: &str,
) -> Result<u32, FieldError> {
    let error = || FieldError {
        field,
        message: format!(
            "{field} must be a whole number from {} to {} {unit}.",
            range.start(),
            range.end()
        ),
    };
    let value: u32 = text.trim().parse().map_err(|_| error())?;
    range.contains(&value).then_some(value).ok_or_else(error)
}

/// Draws the dialog as a window and reports what the person decided.
pub fn show(ctx: &Context, draft: &mut Draft) -> Outcome {
    let mut outcome = Outcome::Open;
    let mut open = true;
    let validation = draft.validate();
    egui::Window::new("Settings")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            egui::Grid::new("settings_grid")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("Tolerance (percentage points)");
                    text_field(ui, &mut draft.tolerance);
                    ui.end_row();
                    ui.label("Stale after (seconds)");
                    text_field(ui, &mut draft.stale_after);
                    ui.end_row();
                    ui.label("Rate period (seconds)");
                    text_field(ui, &mut draft.rate_period);
                    ui.end_row();
                    ui.label("Window");
                    ui.checkbox(&mut draft.always_on_top, "Always on top");
                    ui.end_row();
                    ui.label("Start view");
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut draft.start_view, StartView::Compact, "Compact");
                        ui.radio_value(&mut draft.start_view, StartView::Detailed, "Detailed");
                    });
                    ui.end_row();
                });
            autostart_row(ui, draft, &mut outcome);
            if let Err(errors) = &validation {
                ui.add_space(4.0);
                for error in errors {
                    ui.colored_label(Color32::from_rgb(0xD5, 0x5E, 0x00), &error.message);
                }
            }
            if let Some(message) = &draft.save_error {
                ui.add_space(4.0);
                ui.colored_label(Color32::from_rgb(0xD5, 0x5E, 0x00), message);
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let can_save = validation.is_ok();
                if ui
                    .add_enabled(can_save, egui::Button::new("Save"))
                    .clicked()
                    && let Ok(values) = validation.clone()
                {
                    outcome = Outcome::Save(values);
                }
                if ui.button("Cancel").clicked() {
                    outcome = Outcome::Cancel;
                }
            });
        });
    if !open && outcome == Outcome::Open {
        outcome = Outcome::Cancel;
    }
    outcome
}

/// The switch for the start entry of the system, with what is wrong with it, if anything.
fn autostart_row(ui: &mut Ui, draft: &Draft, outcome: &mut Outcome) {
    let warning = Color32::from_rgb(0xD5, 0x5E, 0x00);
    if let Some(message) = &draft.autostart_error {
        ui.add_space(4.0);
        ui.colored_label(warning, message);
    }
    let Some(state) = &draft.autostart else {
        return;
    };
    ui.add_space(4.0);
    let mut on = matches!(state, AutostartState::On | AutostartState::Stale { .. });
    if ui.checkbox(&mut on, AUTOSTART_LABEL).changed() {
        *outcome = Outcome::Autostart(on);
    }
    match state {
        AutostartState::Stale { found } => {
            ui.colored_label(warning, format!("The entry starts another file: {found}"));
            if ui.button("Use this file").clicked() {
                *outcome = Outcome::Autostart(true);
            }
        }
        AutostartState::SwitchedOff => {
            ui.colored_label(warning, SWITCHED_OFF_TEXT);
        }
        AutostartState::On | AutostartState::Off => {}
    }
}

fn text_field(ui: &mut Ui, text: &mut String) {
    ui.add(TextEdit::singleline(text).desired_width(90.0));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(tolerance: &str, stale: &str, rate: &str) -> Draft {
        Draft {
            tolerance: tolerance.to_owned(),
            stale_after: stale.to_owned(),
            rate_period: rate.to_owned(),
            always_on_top: true,
            start_view: StartView::Compact,
            save_error: None,
            autostart: None,
            autostart_error: None,
        }
    }

    fn fields(result: Result<Values, Vec<FieldError>>) -> Vec<&'static str> {
        result.unwrap_err().iter().map(|e| e.field).collect()
    }

    #[test]
    fn req_024_the_draft_of_the_current_settings_is_valid() {
        let settings = Settings::default();
        let values = Draft::from_settings(&settings).validate().unwrap();
        assert_eq!(values.tolerance_pp, settings.tolerance_pp);
        assert_eq!(values.stale_after_s, settings.stale_after_s);
        assert_eq!(values.rate_period_s, settings.rate_period_s);
        assert_eq!(values.always_on_top, settings.always_on_top);
        assert_eq!(values.start_view, settings.start_view);
    }

    #[test]
    fn req_024_dialog_validation_rejects_out_of_range() {
        assert_eq!(
            fields(draft("-0.1", "600", "1800").validate()),
            ["Tolerance"]
        );
        assert_eq!(
            fields(draft("50.1", "600", "1800").validate()),
            ["Tolerance"]
        );
        assert_eq!(fields(draft("5", "59", "1800").validate()), ["Stale after"]);
        assert_eq!(
            fields(draft("5", "86401", "1800").validate()),
            ["Stale after"]
        );
        assert_eq!(fields(draft("5", "600", "299").validate()), ["Rate period"]);
        assert_eq!(
            fields(draft("5", "600", "7201").validate()),
            ["Rate period"]
        );
        assert_eq!(
            fields(draft("-1", "1", "1").validate()),
            ["Tolerance", "Stale after", "Rate period"]
        );
    }

    #[test]
    fn req_024_the_limits_of_the_ranges_are_valid() {
        let low = draft("0", "60", "300").validate().unwrap();
        assert_eq!(
            (low.tolerance_pp, low.stale_after_s, low.rate_period_s),
            (0.0, 60, 300)
        );
        let high = draft("50", "86400", "7200").validate().unwrap();
        assert_eq!(
            (high.tolerance_pp, high.stale_after_s, high.rate_period_s),
            (50.0, 86_400, 7_200)
        );
    }

    #[test]
    fn req_024_text_that_is_not_a_number_is_rejected() {
        for bad in [
            "", " ", "abc", "NaN", "inf", "-inf", "1e999", "5 points", "--5",
        ] {
            assert!(
                draft(bad, "600", "1800").validate().is_err(),
                "tolerance {bad:?}"
            );
        }
        for bad in [
            "",
            "6.5",
            "1e3",
            "-60",
            "600s",
            "NaN",
            "99999999999999999999",
        ] {
            assert!(draft("5", bad, "1800").validate().is_err(), "stale {bad:?}");
            assert!(draft("5", "600", bad).validate().is_err(), "rate {bad:?}");
        }
    }

    #[test]
    fn req_024_spaces_and_a_decimal_comma_are_accepted() {
        let values = draft(" 7,5 ", " 900 ", "\t1200\n").validate().unwrap();
        assert_eq!(values.tolerance_pp, 7.5);
        assert_eq!(values.stale_after_s, 900);
        assert_eq!(values.rate_period_s, 1_200);
    }

    #[test]
    fn req_024_the_message_names_the_range() {
        let errors = draft("99", "600", "1800").validate().unwrap_err();
        assert_eq!(
            errors[0].message,
            "Tolerance must be a number from 0 to 50 points."
        );
        let errors = draft("5", "x", "1800").validate().unwrap_err();
        assert_eq!(
            errors[0].message,
            "Stale after must be a whole number from 60 to 86400 seconds."
        );
    }

    #[test]
    fn req_024_applying_changes_only_the_edited_fields() {
        let mut settings = Settings::default();
        settings.window.x = 321.0;
        let before = settings.clone();
        let values = Values {
            tolerance_pp: 12.0,
            stale_after_s: 120,
            rate_period_s: 900,
            always_on_top: false,
            start_view: StartView::Detailed,
        };
        values.apply_to(&mut settings);
        assert_eq!(settings.tolerance_pp, 12.0);
        assert_eq!(settings.stale_after_s, 120);
        assert_eq!(settings.rate_period_s, 900);
        assert!(!settings.always_on_top);
        assert_eq!(settings.start_view, StartView::Detailed);
        assert_eq!(settings.window, before.window, "the window geometry stays");
    }
}
