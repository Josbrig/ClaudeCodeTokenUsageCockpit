// SPDX-License-Identifier: Apache-2.0
//! The commands `setup-bridge` and `remove-bridge` on the command line: ask the person, call
//! [`crate::setup`] and report. Exit codes: 0 done, 1 declined or not allowed, 2 error.

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use cockpit_core::paths;

use crate::instance;
use crate::setup::{self, RemoveOutcome, SetupError, SetupOutcome};
use crate::uninstall::{self, Data, Locations};

const QUESTION: &str = "Change Claude Code settings? [y/N] ";
/// Printed instead of asking on Windows, where a console prompt is not reliable for a program
/// built for the GUI subsystem.
pub const WINDOWS_NEEDS_YES: &str =
    "On Windows use --yes, or set up the bridge from the cockpit window.";

/// `usage-cockpit setup-bridge [--yes]`.
pub fn setup_bridge(yes: bool) -> ExitCode {
    if let Some(code) = needs_yes(yes) {
        return code;
    }
    let Some((settings, state)) = locations() else {
        return ExitCode::from(2);
    };
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(error) => return fail(&format!("cannot determine the executable path: {error}")),
    };
    match setup::setup(&settings, &exe, &state, &mut ask(yes)) {
        Ok(SetupOutcome::Changed { backup }) => {
            if let Some(backup) = backup {
                println!("Backup of the settings: {}", backup.display());
            }
            println!("The bridge is now the Claude Code status line command.");
            ExitCode::SUCCESS
        }
        Ok(SetupOutcome::AlreadySetUp) => {
            println!("The bridge is already set up; nothing changed.");
            ExitCode::SUCCESS
        }
        Ok(SetupOutcome::Declined) => declined(),
        Err(error) => report(&error),
    }
}

/// `usage-cockpit remove-bridge [--yes]`.
pub fn remove_bridge(yes: bool) -> ExitCode {
    if let Some(code) = needs_yes(yes) {
        return code;
    }
    let Some((settings, state)) = locations() else {
        return ExitCode::from(2);
    };
    match setup::remove(&settings, &state, &mut ask(yes)) {
        Ok(RemoveOutcome::Removed { backup }) => {
            println!("Backup of the settings: {}", backup.display());
            println!("The bridge is removed from the Claude Code settings.");
            ExitCode::SUCCESS
        }
        Ok(RemoveOutcome::NotTheBridge) => {
            println!("The Claude Code status line is not the bridge; nothing changed.");
            ExitCode::SUCCESS
        }
        Ok(RemoveOutcome::Declined) => declined(),
        Err(error) => report(&error),
    }
}

/// `usage-cockpit uninstall [--yes] [--remove-data]`.
pub fn uninstall(yes: bool, remove_data: bool) -> ExitCode {
    if let Some(code) = needs_yes(yes) {
        return code;
    }
    let Some(loc) = uninstall_locations() else {
        return ExitCode::from(2);
    };
    let mut data = if remove_data {
        Data::Delete
    } else {
        Data::Keep
    };
    println!("{}", uninstall::describe(&loc, data));
    if !yes && !ask_plainly("Remove everything listed? [y/N] ") {
        return declined();
    }
    // The data can only be deleted while no cockpit window runs: it holds files open. The lock
    // is released again before the deletion, because the lock file is one of the files.
    if data == Data::Delete {
        match instance::acquire(&loc.data_dir) {
            Ok(guard) => drop(guard),
            Err(instance::AlreadyRunning) => {
                println!(
                    "The cockpit window is open: its history and settings are kept. Close it and run the command again to delete them."
                );
                data = Data::Keep;
            }
        }
    }
    let report = uninstall::run(&loc, data, true);
    println!("{}", report.text());
    if report.failed {
        return ExitCode::from(2);
    }
    println!("Done. You can delete the program file by hand now.");
    ExitCode::SUCCESS
}

/// `usage-cockpit finish-uninstall --after <pid>` (started by the window).
pub fn finish_uninstall(after: u32) -> ExitCode {
    let Some(loc) = uninstall_locations() else {
        return ExitCode::from(2);
    };
    uninstall::finish_data(&loc, || uninstall::wait_for_exit(after));
    ExitCode::SUCCESS
}

/// The places "remove everything" works on.
fn uninstall_locations() -> Option<Locations> {
    let (claude_settings, bridge_state) = locations()?;
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(error) => {
            eprintln!("usage-cockpit: cannot determine the executable path: {error}");
            return None;
        }
    };
    match (paths::data_dir(), paths::config_dir()) {
        (Ok(data_dir), Ok(config_dir)) => Some(Locations {
            claude_settings,
            bridge_state,
            data_dir,
            config_dir,
            exe,
            autostart_name: crate::autostart::default_name().to_owned(),
        }),
        _ => {
            eprintln!("usage-cockpit: cannot determine the data and configuration folders");
            None
        }
    }
}

/// The settings file of Claude Code and `bridge-state.json`.
fn locations() -> Option<(PathBuf, PathBuf)> {
    let Some(claude) = paths::claude_dir() else {
        eprintln!("usage-cockpit: cannot determine the Claude Code settings directory");
        return None;
    };
    match paths::config_dir() {
        Ok(config) => Some((
            claude.join("settings.json"),
            config.join("bridge-state.json"),
        )),
        Err(error) => {
            eprintln!("usage-cockpit: {error}");
            None
        }
    }
}

fn needs_yes(yes: bool) -> Option<ExitCode> {
    if cfg!(windows) && !yes {
        eprintln!("{WINDOWS_NEEDS_YES}");
        return Some(ExitCode::from(1));
    }
    None
}

/// Shows the plan and asks; `--yes` answers for the person.
fn ask(yes: bool) -> impl FnMut(&str) -> bool {
    move |plan| {
        println!("{plan}");
        yes || ask_plainly(QUESTION)
    }
}

/// Asks `question` on the console; only "y" or "yes" is a yes.
fn ask_plainly(question: &str) -> bool {
    print!("{question}");
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    if std::io::stdin().lock().read_line(&mut answer).is_err() {
        return false;
    }
    matches!(answer.trim().to_lowercase().as_str(), "y" | "yes")
}

fn declined() -> ExitCode {
    println!("Nothing changed.");
    ExitCode::from(1)
}

fn report(error: &SetupError) -> ExitCode {
    fail(&error.to_string())
}

fn fail(message: &str) -> ExitCode {
    eprintln!("usage-cockpit: {message}");
    ExitCode::from(2)
}
