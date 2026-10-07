// SPDX-License-Identifier: Apache-2.0
//! The cockpit window (concept §11). This is the skeleton that proves the choice of ADR 0002:
//! a small window that stays on top of other windows. Real content follows in later issues.
//!
//! Only `main` calls into this module; the bridge never does (see the test in `main.rs`), so the
//! bridge starts without any GUI code running.

use std::path::Path;
use std::time::Duration;

use cockpit_core::paths;
use cockpit_core::settings::Settings;
use cockpit_core::viewmodel::WindowView;
use eframe::egui;

mod state;

use state::AppState;

/// Default size of the compact view in logical pixels (concept §11.2).
pub const DEFAULT_SIZE: [f32; 2] = [320.0, 120.0];
/// Size of the "already running" window in logical pixels.
pub const ALREADY_RUNNING_SIZE: [f32; 2] = [280.0, 90.0];
/// Text of the "already running" window (concept §11.6).
pub const ALREADY_RUNNING_TEXT: &str = "usage-cockpit is already running.";
/// Title of the window.
pub const TITLE: &str = "usage-cockpit";
/// How often the window is drawn again without any input.
pub const REPAINT_EVERY: Duration = Duration::from_secs(1);

/// Opens the window and runs until it is closed. `data_dir` is where the bridge stores the
/// records.
pub fn run(settings: &Settings, data_dir: &Path) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: viewport(settings),
        ..Default::default()
    };
    let settings = settings.clone();
    let data_dir = data_dir.to_path_buf();
    eframe::run_native(
        TITLE,
        options,
        Box::new(move |creation_context| {
            let context = creation_context.egui_ctx.clone();
            let state = AppState::new(&data_dir, settings, paths::claude_dir(), move || {
                context.request_repaint()
            });
            Ok(Box::new(Cockpit { state }))
        }),
    )
}

/// Opens a small window that says another cockpit is running, and returns when it is closed.
pub fn show_already_running() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(TITLE)
            .with_inner_size(ALREADY_RUNNING_SIZE)
            .with_resizable(false)
            .with_always_on_top(),
        ..Default::default()
    };
    eframe::run_native(
        TITLE,
        options,
        Box::new(|_creation_context| Ok(Box::new(AlreadyRunning))),
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

/// The window: the data and what the view model says about it.
struct Cockpit {
    state: AppState,
}

impl eframe::App for Cockpit {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.state.drain();
        let view = self.state.view_model_now();
        egui::CentralPanel::default().show(ctx, |ui| {
            // Placeholder content: the real views follow in later issues.
            ui.label(TITLE);
            let text = match (&view.banner, &view.five_hour) {
                (Some(banner), _) => banner.clone(),
                (None, WindowView::Data(window)) => format!("5h {}", window.used_text),
                (None, WindowView::NoData { text }) => text.clone(),
            };
            ui.label(text);
            ui.label(&view.age_text);
        });
        ctx.request_repaint_after(REPAINT_EVERY);
    }
}

/// The small window of a second start.
struct AlreadyRunning;

impl eframe::App for AlreadyRunning {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label(ALREADY_RUNNING_TEXT);
            ui.add_space(8.0);
            if ui.button("OK").clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }
}
