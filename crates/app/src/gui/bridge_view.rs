// SPDX-License-Identifier: Apache-2.0
//! "Set up bridge" and "Remove bridge" in the window (concept §5.4, §11.7). The window uses the
//! same code as the command line (`crate::setup`); only the way consent is given differs.
//!
//! `setup` and `remove` ask for consent through a callback that gets the planned change as text.
//! A window cannot wait inside such a callback, so the call is made twice: the first time the
//! callback only keeps the text and says no (nothing is changed), the dialog shows the text, and
//! after a yes the call is made again with a callback that says yes.

use std::path::PathBuf;

use eframe::egui::{self, Context};

use crate::setup::{self, RemoveOutcome, SetupOutcome};

/// What the buttons act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Make the bridge the status line command.
    Setup,
    /// Take the bridge out and put the previous status line back.
    Remove,
}

/// The files and the program involved.
#[derive(Debug, Clone)]
pub struct Paths {
    /// Claude Code settings file.
    pub settings: PathBuf,
    /// `bridge-state.json` in the configuration folder.
    pub state: PathBuf,
    /// This program.
    pub exe: PathBuf,
}

/// Where the dialog is: asking for consent, or telling the result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stage {
    /// The planned change is shown; the person says yes or no.
    Ask(String),
    /// Nothing more to decide; this is the result or the reason why nothing happens.
    Done(String),
}

/// The open dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dialog {
    /// What it is about.
    pub kind: Kind,
    /// How far it is.
    pub stage: Stage,
}

/// Looks at what would change, without changing anything.
pub fn prepare(kind: Kind, paths: &Paths) -> Dialog {
    let mut plan = None;
    let mut keep_plan = |text: &str| {
        plan = Some(text.to_owned());
        false
    };
    let stage = match kind {
        Kind::Setup => {
            match setup::setup(&paths.settings, &paths.exe, &paths.state, &mut keep_plan) {
                Ok(SetupOutcome::AlreadySetUp) => {
                    Stage::Done("The bridge is already set up; nothing to change.".to_owned())
                }
                Ok(_) => ask_or_nothing(plan),
                Err(error) => Stage::Done(error.to_string()),
            }
        }
        Kind::Remove => match setup::remove(&paths.settings, &paths.state, &mut keep_plan) {
            Ok(RemoveOutcome::NotTheBridge) => Stage::Done(
                "The Claude Code status line is not the bridge; nothing to change.".to_owned(),
            ),
            Ok(_) => ask_or_nothing(plan),
            Err(error) => Stage::Done(error.to_string()),
        },
    };
    Dialog { kind, stage }
}

fn ask_or_nothing(plan: Option<String>) -> Stage {
    match plan {
        Some(text) => Stage::Ask(text),
        None => Stage::Done("There is nothing to change.".to_owned()),
    }
}

/// Makes the change after the person agreed and returns the result in plain words.
pub fn perform(kind: Kind, paths: &Paths) -> String {
    let mut yes = |_: &str| true;
    match kind {
        Kind::Setup => match setup::setup(&paths.settings, &paths.exe, &paths.state, &mut yes) {
            Ok(SetupOutcome::Changed { backup }) => {
                let mut text = "The bridge is set up.".to_owned();
                if let Some(backup) = backup {
                    text.push_str(&format!(
                        " A copy of the old settings is {}.",
                        backup.display()
                    ));
                }
                text
            }
            Ok(SetupOutcome::AlreadySetUp) => "The bridge is already set up.".to_owned(),
            Ok(SetupOutcome::Declined) => "Nothing was changed.".to_owned(),
            Err(error) => format!("The bridge could not be set up: {error}"),
        },
        Kind::Remove => match setup::remove(&paths.settings, &paths.state, &mut yes) {
            Ok(RemoveOutcome::Removed { backup }) => format!(
                "The bridge is removed. A copy of the old settings is {}.",
                backup.display()
            ),
            Ok(RemoveOutcome::NotTheBridge) => {
                "The Claude Code status line is not the bridge; nothing was changed.".to_owned()
            }
            Ok(RemoveOutcome::Declined) => "Nothing was changed.".to_owned(),
            Err(error) => format!("The bridge could not be removed: {error}"),
        },
    }
}

/// Draws the dialog; returns `true` when it is to be closed.
pub fn show(ctx: &Context, dialog: &mut Dialog, paths: Option<&Paths>) -> bool {
    let title = match dialog.kind {
        Kind::Setup => "Set up bridge",
        Kind::Remove => "Remove bridge",
    };
    let mut open = true;
    let mut close = false;
    let mut next = None;
    egui::Window::new(title)
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .max_width(460.0)
        .show(ctx, |ui| match &dialog.stage {
            Stage::Ask(plan) => {
                ui.label(plan);
                ui.add_space(6.0);
                ui.label("Change the Claude Code settings?");
                ui.horizontal(|ui| {
                    if ui.button("Yes").clicked() {
                        next = Some(Stage::Done(match paths {
                            Some(paths) => perform(dialog.kind, paths),
                            None => "The settings folder cannot be found.".to_owned(),
                        }));
                    }
                    if ui.button("No").clicked() {
                        close = true;
                    }
                });
            }
            Stage::Done(text) => {
                ui.label(text);
                ui.add_space(6.0);
                if ui.button("OK").clicked() {
                    close = true;
                }
            }
        });
    if let Some(stage) = next {
        dialog.stage = stage;
    }
    close || !open
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::{Value, json};

    use super::*;

    struct Fixture {
        _dir: tempfile::TempDir,
        paths: Paths,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths {
            settings: dir.path().join("claude").join("settings.json"),
            state: dir.path().join("config").join("bridge-state.json"),
            exe: dir.path().join("bin").join("usage-cockpit"),
        };
        Fixture { _dir: dir, paths }
    }

    fn put(paths: &Paths, value: &Value) {
        fs::create_dir_all(paths.settings.parent().unwrap()).unwrap();
        fs::write(&paths.settings, value.to_string()).unwrap();
    }

    fn json_of(paths: &Paths) -> Value {
        serde_json::from_slice(&fs::read(&paths.settings).unwrap()).unwrap()
    }

    #[test]
    fn req_023_the_plan_is_shown_and_declining_changes_nothing() {
        let f = fixture();
        put(
            &f.paths,
            &json!({"statusLine": {"type": "command", "command": "ccusage"}}),
        );
        let before = fs::read(&f.paths.settings).unwrap();
        let dialog = prepare(Kind::Setup, &f.paths);
        let Stage::Ask(plan) = &dialog.stage else {
            panic!("expected a question, got {:?}", dialog.stage);
        };
        assert!(plan.contains("statusLine"), "{plan}");
        assert!(plan.contains("ccusage"), "the old command is named: {plan}");
        assert_eq!(
            fs::read(&f.paths.settings).unwrap(),
            before,
            "byte-identical"
        );
        assert!(!f.paths.state.exists());
    }

    #[test]
    fn req_023_yes_sets_up_the_bridge_and_a_second_look_says_it_is_done() {
        let f = fixture();
        put(&f.paths, &json!({"model": "x"}));
        let message = perform(Kind::Setup, &f.paths);
        assert!(message.starts_with("The bridge is set up."), "{message}");
        assert!(message.contains("A copy of the old settings"), "{message}");
        let settings = json_of(&f.paths);
        assert!(
            settings["statusLine"]["command"]
                .as_str()
                .unwrap()
                .ends_with(" bridge")
        );
        assert_eq!(settings["model"], "x");
        let again = prepare(Kind::Setup, &f.paths);
        assert_eq!(
            again.stage,
            Stage::Done("The bridge is already set up; nothing to change.".to_owned())
        );
    }

    #[test]
    fn req_023_removal_asks_and_then_restores_the_previous_status_line() {
        let f = fixture();
        let old = json!({"statusLine": {"type": "command", "command": "ccusage"}});
        put(&f.paths, &old);
        perform(Kind::Setup, &f.paths);
        let dialog = prepare(Kind::Remove, &f.paths);
        assert!(matches!(dialog.stage, Stage::Ask(_)), "{:?}", dialog.stage);
        let message = perform(Kind::Remove, &f.paths);
        assert!(message.starts_with("The bridge is removed."), "{message}");
        assert_eq!(json_of(&f.paths), old);
    }

    #[test]
    fn req_023_removal_without_the_bridge_says_so() {
        let f = fixture();
        put(
            &f.paths,
            &json!({"statusLine": {"type": "command", "command": "ccusage"}}),
        );
        let dialog = prepare(Kind::Remove, &f.paths);
        assert_eq!(
            dialog.stage,
            Stage::Done(
                "The Claude Code status line is not the bridge; nothing to change.".to_owned()
            )
        );
        // No settings file at all is the same case.
        let g = fixture();
        assert!(matches!(
            prepare(Kind::Remove, &g.paths).stage,
            Stage::Done(_)
        ));
    }

    #[test]
    fn req_023_a_path_with_a_space_is_explained_and_nothing_is_changed() {
        let mut f = fixture();
        put(&f.paths, &json!({"a": 1}));
        f.paths.exe = f
            .paths
            .exe
            .parent()
            .unwrap()
            .join("with space")
            .join("usage-cockpit");
        let before = fs::read(&f.paths.settings).unwrap();
        let Stage::Done(text) = prepare(Kind::Setup, &f.paths).stage else {
            panic!("a path with a space must stop at once");
        };
        assert!(text.contains("without spaces"), "{text}");
        let message = perform(Kind::Setup, &f.paths);
        assert!(
            message.starts_with("The bridge could not be set up:"),
            "{message}"
        );
        assert_eq!(fs::read(&f.paths.settings).unwrap(), before);
    }

    #[test]
    fn req_023_unusable_settings_are_reported_in_words() {
        let f = fixture();
        fs::create_dir_all(f.paths.settings.parent().unwrap()).unwrap();
        fs::write(&f.paths.settings, "{ not json").unwrap();
        let Stage::Done(text) = prepare(Kind::Setup, &f.paths).stage else {
            panic!("broken settings must stop at once");
        };
        assert!(text.contains("cannot be parsed"), "{text}");
        assert_eq!(fs::read_to_string(&f.paths.settings).unwrap(), "{ not json");
    }
}
