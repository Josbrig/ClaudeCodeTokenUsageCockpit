// SPDX-License-Identifier: Apache-2.0
//! The cockpit window (concept §11). This is the skeleton that proves the choice of ADR 0002:
//! a small window that stays on top of other windows. Real content follows in later issues.
//!
//! Only `main` calls into this module; the bridge never does (see the test in `main.rs`), so the
//! bridge starts without any GUI code running.

use std::time::Duration;

use cockpit_core::settings::Settings;
use eframe::egui;

/// Default size of the compact view in logical pixels (concept §11.2).
pub const DEFAULT_SIZE: [f32; 2] = [320.0, 120.0];
/// Title of the window.
pub const TITLE: &str = "usage-cockpit";
/// How often the window is drawn again without any input.
pub const REPAINT_EVERY: Duration = Duration::from_secs(1);

/// Opens the window and runs until it is closed.
pub fn run(settings: &Settings) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: viewport(settings),
        ..Default::default()
    };
    eframe::run_native(
        TITLE,
        options,
        Box::new(|_creation_context| Ok(Box::new(Cockpit))),
    )
}

/// The window settings: size and, if the settings ask for it, always on top.
fn viewport(settings: &Settings) -> egui::ViewportBuilder {
    let builder = egui::ViewportBuilder::default()
        .with_title(TITLE)
        .with_inner_size(DEFAULT_SIZE);
    if settings.always_on_top {
        builder.with_always_on_top()
    } else {
        builder
    }
}

/// The application state; empty until the window gets content.
struct Cockpit;

impl eframe::App for Cockpit {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label(TITLE);
            ui.add(egui::ProgressBar::new(0.5).desired_width(ui.available_width()));
        });
        ctx.request_repaint_after(REPAINT_EVERY);
    }
}
