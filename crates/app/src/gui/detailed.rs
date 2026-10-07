// SPDX-License-Identifier: Apache-2.0
//! The detailed view (concept §11.3): all values of both windows, binding limit and weekly plan,
//! session details, transcript statistics, previous periods and a footer. The history chart
//! follows in its own issue. Like the compact view it only lays out what the view model
//! provides.

use cockpit_core::format;
use cockpit_core::model::WindowKind;
use cockpit_core::viewmodel::{
    NO_DATA_GLYPH, PreviousPeriod, STALE_GLYPH, STALE_LABEL, TranscriptView, UsageText, ViewModel,
    WindowData, WindowView,
};
use eframe::egui::{self, Color32, RichText, ScrollArea, Sense, Ui, Vec2};

use super::{bars, theme};

/// Page that lists the licences of the components the program uses.
pub const LICENCE_NOTICES_URL: &str =
    "https://github.com/Josbrig/ClaudeCodeTokenUsageCockpit/blob/main/THIRD_PARTY_LICENSES.md";
/// Default size of the detailed view in logical pixels (concept §11.3).
pub const DEFAULT_SIZE: [f32; 2] = [520.0, 640.0];
/// Shown for the estimate when it cannot be computed; it always carries the word "estimate".
pub const ESTIMATE_NOT_AVAILABLE: &str = "not available (estimate)";
const ICON_SIZE: f32 = 10.0;

/// What the person did in the detailed view.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Action {
    /// The button "Compact view" or the key `D` was used.
    pub switch_view: bool,
    /// The button "Settings" was used.
    pub open_settings: bool,
    /// The button "Set up bridge" was used.
    pub setup_bridge: bool,
    /// The button "Remove bridge" was used.
    pub remove_bridge: bool,
    /// The button "Remove everything" was used.
    pub remove_everything: bool,
}

/// Version and commit as shown in the footer, for example `0.1.0 (a1b2c3d)`.
pub fn version_text() -> String {
    format!("{} ({})", env!("CARGO_PKG_VERSION"), env!("GIT_COMMIT"))
}

/// The name of the binding window for the section "Binding limit".
pub fn binding_text(binding: Option<WindowKind>) -> &'static str {
    match binding {
        Some(WindowKind::FiveHour) => "5-hour window",
        Some(WindowKind::SevenDay) => "7-day window",
        None => "none",
    }
}

/// One line of the list of previous periods.
pub fn previous_text(period: &PreviousPeriod) -> String {
    let window = match period.kind {
        WindowKind::FiveHour => "5h",
        WindowKind::SevenDay => "7d",
    };
    format!(
        "{window}  ended {}  final {}",
        period.reset_local_text, period.final_used_text
    )
}

/// Draws the detailed view into `ui`.
pub fn show(ui: &mut Ui, view: &ViewModel) -> Action {
    let mut action = Action::default();
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if let Some(banner) = &view.banner {
                ui.label(RichText::new(banner).strong());
                ui.separator();
            }
            if !view.age_text.is_empty() {
                ui.label(&view.age_text);
            }
            window_section(ui, "5-hour window", &view.five_hour, view.stale);
            window_section(ui, "7-day window", &view.seven_day, view.stale);
            planning_section(ui, view);
            session_section(ui, view);
            transcript_section(ui, view);
            previous_section(ui, view);
            footer(ui, &mut action);
        });
    if super::plain_d_pressed(ui.ctx()) {
        action.switch_view = true;
    }
    action
}

fn heading(ui: &mut Ui, text: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(text).strong().size(14.0));
    ui.separator();
}

fn row(ui: &mut Ui, name: &str, value: &str) {
    ui.label(name);
    ui.label(value);
    ui.end_row();
}

/// Section 1: all values of one window.
fn window_section(ui: &mut Ui, title: &str, window: &WindowView, stale: bool) {
    heading(ui, title);
    match window {
        WindowView::NoData { text } => {
            symbol_line(ui, NO_DATA_GLYPH, text, theme::GREY);
        }
        WindowView::Data(data) => data_grid(ui, title, data, stale),
    }
}

fn data_grid(ui: &mut Ui, id: &str, data: &WindowData, stale: bool) {
    let (glyph, label, color) = if stale {
        (STALE_GLYPH, STALE_LABEL, theme::GREY)
    } else {
        (data.glyph, data.label, theme::state_color(data.state))
    };
    symbol_line(ui, glyph, label, color);
    if data.binding {
        ui.label("This window's limit binds first.");
    }
    super::chart::show(ui, &format!("{id} chart"), &data.chart, color);
    egui::Grid::new(id)
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            row(ui, "Used", &data.used_text);
            row(ui, "Remaining", &data.remaining_text);
            row(
                ui,
                "Resets",
                &format!("{} (in {})", data.reset_local_text, data.countdown_text),
            );
            row(ui, "Target", &format::pct(data.target));
            row(ui, "Deviation", &data.deviation_text);
            row(ui, "Pace factor", &data.factor_text);
            row(ui, "Usage rate", &data.rate_text);
            row(ui, "Forecast", &data.forecast_text);
            row(ui, "Unused at reset", &data.unused_text);
            row(ui, "Recommended rate", &data.recommended_text);
        });
}

/// A symbol drawn as a shape, followed by a text.
fn symbol_line(ui: &mut Ui, glyph: &str, text: &str, color: Color32) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(ICON_SIZE + 2.0), Sense::hover());
        if let Some(icon) = theme::icon_for_glyph(glyph) {
            theme::paint_icon(ui.painter(), rect.center(), ICON_SIZE, icon, color);
        }
        ui.label(RichText::new(text).color(color));
    });
}

/// Section 2: binding limit and weekly plan.
fn planning_section(ui: &mut Ui, view: &ViewModel) {
    heading(ui, "Binding limit and weekly plan");
    egui::Grid::new("planning").num_columns(2).show(ui, |ui| {
        row(ui, "Binding limit", binding_text(view.binding));
        row(
            ui,
            "Weekly plan",
            view.weekly_text.as_deref().unwrap_or("not available"),
        );
    });
}

/// Section 4: details of the session, from the latest record.
fn session_section(ui: &mut Ui, view: &ViewModel) {
    heading(ui, view.session.heading);
    egui::Grid::new("session").num_columns(2).show(ui, |ui| {
        row(ui, "Model", &view.session.model_text);
        row(ui, "Context used", &view.session.context_text);
        row(ui, "Session cost", &view.session.cost_text);
    });
}

/// Section 5: transcript statistics and the token estimate.
fn transcript_section(ui: &mut Ui, view: &ViewModel) {
    heading(ui, "Transcript statistics");
    let t: &TranscriptView = &view.transcripts;
    if !t.available {
        ui.label(&t.cache_share_text);
    } else {
        ui.label("Per model");
        if t.per_model.is_empty() {
            ui.label("no data");
        } else {
            usage_grid(ui, "per_model", "Model", &t.per_model);
        }
        ui.add_space(4.0);
        if !t.per_day_numbers.is_empty() {
            let kind = bars::choice(ui);
            bars::days(ui, "per_day_chart", &t.per_day_numbers, kind);
            ui.add_space(4.0);
        }
        ui.label("Per day (newest first)");
        // At most 35 rows; the whole view scrolls, so the table needs no scroll area of its own.
        if t.per_day.is_empty() {
            ui.label("no data");
        } else {
            usage_grid(ui, "per_day", "Day", &t.per_day);
        }
        ui.label(&t.cache_share_text);
    }
    let estimate = view
        .estimate_text
        .as_deref()
        .unwrap_or(ESTIMATE_NOT_AVAILABLE);
    ui.label(format!("Tokens per percentage point: {estimate}"));
}

fn usage_grid(ui: &mut Ui, id: &str, first: &str, rows: &[(String, UsageText)]) {
    egui::Grid::new(id)
        .num_columns(5)
        .striped(true)
        .show(ui, |ui| {
            for head in [first, "Input", "Output", "Cache write", "Cache read"] {
                ui.label(RichText::new(head).strong());
            }
            ui.end_row();
            for (name, usage) in rows {
                ui.label(name);
                for value in [
                    &usage.input,
                    &usage.output,
                    &usage.cache_creation,
                    &usage.cache_read,
                ] {
                    ui.label(value);
                }
                ui.end_row();
            }
        });
}

/// Section 6: the last finished periods.
fn previous_section(ui: &mut Ui, view: &ViewModel) {
    heading(ui, "Previous periods");
    if view.previous.is_empty() {
        ui.label("none yet");
    }
    for period in &view.previous {
        ui.label(previous_text(period));
    }
}

/// Section 7: version, licence notices and the way back to the compact view.
fn footer(ui: &mut Ui, action: &mut Action) {
    ui.add_space(8.0);
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(format!("usage-cockpit {}", version_text()));
        ui.hyperlink_to("Licence notices", LICENCE_NOTICES_URL);
    });
    ui.horizontal_wrapped(|ui| {
        if ui.button("Settings").clicked() {
            action.open_settings = true;
        }
        if ui.button("Set up bridge").clicked() {
            action.setup_bridge = true;
        }
        if ui.button("Remove bridge").clicked() {
            action.remove_bridge = true;
        }
        if ui.button("Remove everything").clicked() {
            action.remove_everything = true;
        }
        if ui.button("Compact view (D)").clicked() {
            action.switch_view = true;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_032_version_text_has_version_and_commit() {
        let text = version_text();
        assert!(text.starts_with(env!("CARGO_PKG_VERSION")), "{text}");
        assert!(text.contains(env!("GIT_COMMIT")), "{text}");
        assert!(text.ends_with(')'));
    }

    #[test]
    fn req_032_licence_notices_link_points_to_the_licence_file() {
        assert!(LICENCE_NOTICES_URL.starts_with("https://"));
        assert!(LICENCE_NOTICES_URL.ends_with("THIRD_PARTY_LICENSES.md"));
    }

    #[test]
    fn req_008_binding_text_names_the_window() {
        assert_eq!(binding_text(Some(WindowKind::FiveHour)), "5-hour window");
        assert_eq!(binding_text(Some(WindowKind::SevenDay)), "7-day window");
        assert_eq!(binding_text(None), "none");
    }

    #[test]
    fn req_013_previous_text_names_window_end_and_final_value() {
        let period = PreviousPeriod {
            kind: WindowKind::FiveHour,
            reset_local_text: "Sat 09:00".to_owned(),
            final_used_text: "44.0%".to_owned(),
        };
        assert_eq!(previous_text(&period), "5h  ended Sat 09:00  final 44.0%");
    }

    #[test]
    fn req_015_the_missing_estimate_still_says_estimate() {
        assert!(ESTIMATE_NOT_AVAILABLE.contains("estimate"));
        assert!(ESTIMATE_NOT_AVAILABLE.contains("not available"));
    }
}
