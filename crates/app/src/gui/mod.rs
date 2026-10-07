// SPDX-License-Identifier: Apache-2.0
//! The cockpit window (concept §11): the compact and the detailed view, switching between them,
//! and keeping position, size, view and level of the window across runs.
//!
//! Only `main` calls into this module; the bridge never does (see the test in `main.rs`), so the
//! bridge starts without any GUI code running.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use cockpit_core::paths;
use cockpit_core::settings::{self, Settings, StartView, WindowSettings};
use eframe::egui::{self, Vec2};

mod compact;
mod detailed;
mod settings_view;
mod state;
mod theme;
mod window_state;

use state::AppState;
use window_state::Debounce;

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
/// records, `settings_path` the file the window position, size and view are saved to.
pub fn run(settings: &Settings, settings_path: &Path, data_dir: &Path) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: viewport(settings),
        ..Default::default()
    };
    let settings = settings.clone();
    let settings_path = settings_path.to_path_buf();
    let data_dir = data_dir.to_path_buf();
    eframe::run_native(
        TITLE,
        options,
        Box::new(move |creation_context| {
            let context = creation_context.egui_ctx.clone();
            let state = AppState::new(
                &data_dir,
                settings.clone(),
                paths::claude_dir(),
                move || context.request_repaint(),
            );
            Ok(Box::new(Cockpit::new(state, settings, settings_path)))
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

/// The window at start: the saved position and size and, if the settings ask for it, on top.
fn viewport(settings: &Settings) -> egui::ViewportBuilder {
    let (position, size) = window_state::start_geometry(&settings.window);
    let mut builder = egui::ViewportBuilder::default()
        .with_title(TITLE)
        .with_inner_size(size);
    if let Some(position) = position {
        builder = builder.with_position(position);
    }
    if settings.always_on_top {
        builder = builder.with_always_on_top();
    }
    builder
}

/// `true` when a plain `D` was pressed in this frame: no Ctrl, Alt or Shift, no key repeat (a
/// held key switches once), and no text field has the keyboard.
fn plain_d_pressed(ctx: &egui::Context) -> bool {
    !ctx.wants_keyboard_input()
        && ctx.input(|input| {
            input.modifiers.is_none()
                && input.events.iter().any(|event| {
                    matches!(
                        event,
                        egui::Event::Key {
                            key: egui::Key::D,
                            pressed: true,
                            repeat: false,
                            ..
                        }
                    )
                })
        })
}

/// Which of the two views is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Compact,
    Detailed,
}

impl View {
    /// The other view.
    fn other(self) -> Self {
        match self {
            Self::Compact => Self::Detailed,
            Self::Detailed => Self::Compact,
        }
    }

    /// Default window size of the view in logical pixels.
    fn size(self) -> [f32; 2] {
        match self {
            Self::Compact => DEFAULT_SIZE,
            Self::Detailed => detailed::DEFAULT_SIZE,
        }
    }

    /// The setting that stands for this view.
    fn as_start_view(self) -> StartView {
        match self {
            Self::Compact => StartView::Compact,
            Self::Detailed => StartView::Detailed,
        }
    }
}

/// The window: the data and what the view model says about it, and what is kept of the window
/// itself.
struct Cockpit {
    state: AppState,
    view: View,
    settings: Settings,
    settings_path: PathBuf,
    /// The geometry the settings file holds.
    saved: WindowSettings,
    /// The newest geometry seen (also used when the window is closed).
    current: Option<WindowSettings>,
    debounce: Debounce,
    /// After a switch: the size the window is expected to take, and until when the geometry
    /// reported by the window system is ignored if it does not match yet.
    expected: Option<([f32; 2], Instant)>,
    /// The settings dialog, while it is open.
    dialog: Option<settings_view::Draft>,
    /// Whether the window is on top at the moment, as far as this program has set it.
    on_top: bool,
}

impl Cockpit {
    fn new(state: AppState, settings: Settings, settings_path: PathBuf) -> Self {
        let view = if window_state::start_view_is_detailed(&settings) {
            View::Detailed
        } else {
            View::Compact
        };
        Self {
            state,
            view,
            saved: settings.window.clone(),
            on_top: settings.always_on_top,
            settings,
            settings_path,
            current: None,
            debounce: Debounce::default(),
            expected: None,
            dialog: None,
        }
    }

    /// Follows position and size of the window and writes them after a quiet second.
    fn track_window(&mut self, ctx: &egui::Context) {
        let geometry = ctx.input(|input| {
            let viewport = input.viewport();
            window_state::geometry_from(
                viewport.outer_rect,
                viewport.inner_rect,
                viewport.minimized.unwrap_or(false),
            )
        });
        let Some(geometry) = geometry else {
            return;
        };
        // Right after a switch the window system still reports the size of the other view for a
        // frame or two; that geometry must neither be saved nor kept for the exit.
        if let Some((size, until)) = self.expected {
            if window_state::matches_size(&geometry, size) || Instant::now() >= until {
                self.expected = None;
            } else {
                return;
            }
        }
        self.current = Some(geometry.clone());
        if let Some(to_save) = self.debounce.update(Instant::now(), &geometry, &self.saved) {
            self.save_window(to_save);
        }
    }

    /// The geometry to write when the window closes: the last one seen, or, if the window was
    /// switched and has not reported its new size yet, the default size of the shown view at the
    /// saved position.
    fn geometry_for_exit(&self) -> WindowSettings {
        self.current.clone().unwrap_or_else(|| {
            let [width, height] = self.view.size();
            WindowSettings {
                x: self.saved.x,
                y: self.saved.y,
                width: f64::from(width),
                height: f64::from(height),
            }
        })
    }

    /// Writes the window geometry and the current view to the settings file. Everything else in
    /// the file is taken from the file as it is now, so a change made there meanwhile is kept.
    fn save_window(&mut self, geometry: WindowSettings) {
        let mut on_disk = settings::load(&self.settings_path);
        on_disk.window = geometry.clone();
        on_disk.start_view = self.view.as_start_view();
        match settings::save(&self.settings_path, &on_disk) {
            Ok(()) => {
                self.settings.window = geometry.clone();
                self.settings.start_view = on_disk.start_view;
                self.saved = geometry;
            }
            Err(error) => log::warn!("the window position cannot be saved: {error}"),
        }
    }

    /// Shows the other view and the default size of it.
    fn switch_view(&mut self, ctx: &egui::Context) {
        self.view = self.view.other();
        let size = self.view.size();
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::from(size)));
        // Forget the geometry of the other view; the new one is taken when it is reported.
        self.current = None;
        self.debounce = Debounce::default();
        self.expected = Some((size, Instant::now() + Duration::from_secs(2)));
    }

    /// Draws the settings dialog if it is open and takes its decision.
    fn show_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.dialog.take() else {
            return;
        };
        match settings_view::show(ctx, &mut draft) {
            settings_view::Outcome::Open => self.dialog = Some(draft),
            settings_view::Outcome::Cancel => {}
            settings_view::Outcome::Save(values) => match self.apply_values(ctx, &values) {
                Ok(()) => {}
                Err(message) => {
                    // The dialog stays open and says why nothing was saved.
                    draft.save_error = Some(message);
                    self.dialog = Some(draft);
                }
            },
        }
    }

    /// Writes the dialog values and makes them count at once: the file keeps everything else as
    /// it is, the window level and the view model follow, and the window shows the chosen start
    /// view now, so that what is saved at exit is what was chosen.
    fn apply_values(
        &mut self,
        ctx: &egui::Context,
        values: &settings_view::Values,
    ) -> Result<(), String> {
        let mut on_disk = settings::load(&self.settings_path);
        values.apply_to(&mut on_disk);
        if let Err(error) = settings::save(&self.settings_path, &on_disk) {
            log::warn!("the settings cannot be saved: {error}");
            return Err(format!("The settings could not be saved: {error}"));
        }
        values.apply_to(&mut self.settings);
        self.state.set_settings(self.settings.clone());
        let wanted = match values.start_view {
            StartView::Compact => View::Compact,
            StartView::Detailed => View::Detailed,
        };
        if wanted != self.view {
            self.switch_view(ctx);
        }
        Ok(())
    }

    /// Applies the setting "always on top" when it differs from what the window has.
    fn apply_level(&mut self, ctx: &egui::Context) {
        if let Some(level) = window_state::level_command(self.on_top, self.settings.always_on_top) {
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
            self.on_top = self.settings.always_on_top;
        }
    }
}

impl eframe::App for Cockpit {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.state.drain();
        self.apply_level(ctx);
        let model = self.state.view_model_now();
        let mut switch = false;
        let mut open_settings = false;
        egui::CentralPanel::default().show(ctx, |ui| match self.view {
            View::Compact => switch = compact::show(ui, model).switch_view,
            View::Detailed => {
                let action = detailed::show(ui, model);
                switch = action.switch_view;
                open_settings = action.open_settings;
            }
        });
        if open_settings && self.dialog.is_none() {
            self.dialog = Some(settings_view::Draft::from_settings(&self.settings));
        }
        // While the settings dialog is open the views behind it do not react.
        if switch && self.dialog.is_none() {
            self.switch_view(ctx);
        }
        self.show_dialog(ctx);
        self.track_window(ctx);
        ctx.request_repaint_after(REPAINT_EVERY);
    }

    /// The last geometry and the view are written when the window is closed.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let geometry = self.geometry_for_exit();
        let view_changed = self.settings.start_view != self.view.as_start_view();
        if geometry != self.saved || view_changed {
            self.save_window(geometry);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn cockpit(settings_path: PathBuf) -> (Cockpit, tempfile::TempDir) {
        let data = tempfile::tempdir().unwrap();
        let settings = Settings::default();
        let state = AppState::new(data.path(), settings.clone(), None, || {});
        (Cockpit::new(state, settings, settings_path), data)
    }

    fn values() -> settings_view::Values {
        settings_view::Values {
            tolerance_pp: 12.0,
            stale_after_s: 120,
            rate_period_s: 900,
            always_on_top: false,
            start_view: StartView::Compact,
        }
    }

    #[test]
    fn req_024_a_failed_save_applies_nothing_and_says_why() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a-file");
        std::fs::write(&file, b"x").unwrap();
        // The settings folder would have to lie below a regular file.
        let (mut cockpit, _data) = cockpit(file.join("config").join("settings.toml"));
        let before = cockpit.settings.clone();
        let result = cockpit.apply_values(&egui::Context::default(), &values());
        let message = result.unwrap_err();
        assert!(
            message.starts_with("The settings could not be saved"),
            "{message}"
        );
        assert_eq!(cockpit.settings, before, "nothing is applied");
    }

    #[test]
    fn req_024_a_successful_save_applies_the_values_and_keeps_the_rest_of_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        std::fs::write(
            &path,
            "version = 1\nstale_after_s = 700\n[window]\nx = 321.0\n",
        )
        .unwrap();
        let (mut cockpit, _data) = cockpit(path.clone());
        cockpit
            .apply_values(&egui::Context::default(), &values())
            .unwrap();
        assert_eq!(cockpit.settings.tolerance_pp, 12.0);
        assert!(!cockpit.settings.always_on_top);
        let on_disk = settings::load(&path);
        assert_eq!(on_disk.tolerance_pp, 12.0);
        assert_eq!(on_disk.stale_after_s, 120, "the dialog value wins");
        assert_eq!(
            on_disk.window.x, 321.0,
            "the window geometry in the file stays"
        );
    }
}
