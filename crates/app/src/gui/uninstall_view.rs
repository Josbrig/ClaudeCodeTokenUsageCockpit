// SPDX-License-Identifier: Apache-2.0
//! "Remove everything" in the window (REQ-119): the dialog lists what will be removed, offers the
//! choice about the data, and tells the result. The work is done by `crate::uninstall`, the same
//! code as the command line.
//!
//! The running window holds files of the data folder open, so a deletion of the data is not done
//! here: the report says `data_pending`, and when the dialog is closed the window starts a helper
//! (`usage-cockpit finish-uninstall`) and ends itself; the helper waits for that end.

use eframe::egui::{self, Context};

use crate::uninstall::{self, Data, Locations};

/// Where the dialog is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stage {
    /// The plan is shown; the person says yes or no.
    Ask,
    /// The result. `exit`: the window ends when the person presses OK.
    Done { text: String, exit: bool },
}

/// The open dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dialog {
    /// How far it is.
    pub stage: Stage,
    /// The box "also delete history, logs and settings".
    pub delete_data: bool,
    /// The plan as shown for the current choice.
    pub plan: String,
}

/// What the person decided when the dialog is closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Still open.
    Open,
    /// Closed. `exit`: end the window now.
    Closed { exit: bool },
}

impl Dialog {
    /// The dialog for the first question, with the data kept.
    pub fn new(loc: &Locations) -> Self {
        Self {
            stage: Stage::Ask,
            delete_data: false,
            plan: uninstall::describe(loc, Data::Keep),
        }
    }

    /// The person changed the box: the plan shows what would happen now.
    pub fn set_delete_data(&mut self, loc: &Locations, delete: bool) {
        self.delete_data = delete;
        self.plan = uninstall::describe(loc, data(delete));
    }

    /// The person said yes: does the steps and moves to the result.
    ///
    /// `start_helper` starts the process that deletes the data after the window ended; it is
    /// started here, so that a failure can still be told before the window closes.
    pub fn confirm(&mut self, loc: &Locations, start_helper: &dyn Fn() -> std::io::Result<()>) {
        let report = uninstall::run(loc, data(self.delete_data), false);
        let mut text = report.text();
        if report.data_pending {
            match start_helper() {
                Ok(()) => text
                    .push_str("\nThe history and settings are deleted when the cockpit has closed."),
                Err(error) => text.push_str(&format!(
                    "\nThe history and settings could not be deleted automatically ({error}). Delete the folders {} and {} by hand.",
                    loc.data_dir.display(),
                    loc.config_dir.display()
                )),
            }
        }
        if report.failed {
            text.push_str("\nSomething could not be done: the cockpit stays open.");
        } else {
            text.push_str(
                "\nThe cockpit closes when you press OK. You can then delete the program file by hand.",
            );
        }
        self.stage = Stage::Done {
            text,
            exit: !report.failed,
        };
    }
}

fn data(delete: bool) -> Data {
    if delete { Data::Delete } else { Data::Keep }
}

/// Draws the dialog and reports what the person decided.
pub fn show(ctx: &Context, dialog: &mut Dialog, loc: &Locations) -> Outcome {
    let mut open = true;
    let mut outcome = Outcome::Open;
    let mut delete_changed = None;
    let mut confirmed = false;
    egui::Window::new("Remove everything")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .max_width(460.0)
        .show(ctx, |ui| match &dialog.stage {
            Stage::Ask => {
                ui.label(&dialog.plan);
                ui.add_space(6.0);
                let mut delete = dialog.delete_data;
                if ui
                    .checkbox(&mut delete, "Also delete the history, logs and settings")
                    .changed()
                {
                    delete_changed = Some(delete);
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Remove everything").clicked() {
                        confirmed = true;
                    }
                    if ui.button("Cancel").clicked() {
                        outcome = Outcome::Closed { exit: false };
                    }
                });
            }
            Stage::Done { text, exit } => {
                ui.label(text);
                ui.add_space(6.0);
                if ui.button("OK").clicked() {
                    outcome = Outcome::Closed { exit: *exit };
                }
            }
        });
    if let Some(delete) = delete_changed {
        dialog.set_delete_data(loc, delete);
    }
    if confirmed {
        dialog.confirm(loc, &|| uninstall::start_finish_helper(&loc.exe));
    }
    if !open && outcome == Outcome::Open {
        // The close button counts as OK on the result and as Cancel on the question.
        outcome = match &dialog.stage {
            Stage::Ask => Outcome::Closed { exit: false },
            Stage::Done { exit, .. } => Outcome::Closed { exit: *exit },
        };
    }
    outcome
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn loc() -> (tempfile::TempDir, Locations) {
        let root = tempfile::tempdir().unwrap();
        let base = root.path();
        let loc = Locations {
            claude_settings: base.join("claude").join("settings.json"),
            bridge_state: base.join("config").join("bridge-state.json"),
            data_dir: base.join("data"),
            config_dir: base.join("config"),
            exe: base.join("usage-cockpit.exe"),
            autostart_name: format!("UsageCockpitTest{}", std::process::id()),
        };
        fs::create_dir_all(&loc.data_dir).unwrap();
        fs::create_dir_all(&loc.config_dir).unwrap();
        fs::write(loc.data_dir.join("latest.json"), b"x").unwrap();
        (root, loc)
    }

    #[test]
    fn req_119_the_plan_follows_the_box() {
        let (_root, loc) = loc();
        let mut dialog = Dialog::new(&loc);
        assert!(dialog.plan.contains("Keep the history"), "{}", dialog.plan);
        dialog.set_delete_data(&loc, true);
        assert!(
            dialog.plan.contains("Delete the history"),
            "{}",
            dialog.plan
        );
        dialog.set_delete_data(&loc, false);
        assert!(dialog.plan.contains("Keep the history"));
    }

    #[test]
    fn req_119_keeping_the_data_ends_the_window_without_a_helper() {
        let (_root, loc) = loc();
        let mut dialog = Dialog::new(&loc);
        dialog.confirm(&loc, &|| {
            panic!("no helper is needed when the data is kept")
        });
        assert!(matches!(dialog.stage, Stage::Done { exit: true, .. }));
        assert!(loc.data_dir.join("latest.json").exists());
    }

    #[test]
    fn req_119_a_helper_that_cannot_start_is_told_in_the_result() {
        let (_root, loc) = loc();
        let mut dialog = Dialog::new(&loc);
        dialog.set_delete_data(&loc, true);
        dialog.confirm(&loc, &|| Err(std::io::Error::other("no way")));
        let Stage::Done { text, .. } = &dialog.stage else {
            panic!("the result is shown");
        };
        assert!(text.contains("by hand"), "{text}");
        assert!(text.contains("no way"), "{text}");
        assert!(
            !text.contains("deleted when the cockpit has closed"),
            "{text}"
        );
    }

    #[test]
    fn req_119_deleting_the_data_waits_for_the_helper() {
        let (_root, loc) = loc();
        let mut dialog = Dialog::new(&loc);
        dialog.set_delete_data(&loc, true);
        let started = std::cell::Cell::new(false);
        dialog.confirm(&loc, &|| {
            started.set(true);
            Ok(())
        });
        assert!(
            started.get(),
            "the helper is started when the data is to be deleted"
        );
        let Stage::Done { text, exit } = &dialog.stage else {
            panic!("the result is shown");
        };
        assert!(*exit);
        assert!(text.contains("closed"), "{text}");
        assert!(
            loc.data_dir.join("latest.json").exists(),
            "the running window holds the files: nothing is deleted yet"
        );
    }
}
