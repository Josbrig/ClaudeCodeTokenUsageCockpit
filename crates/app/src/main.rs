// SPDX-License-Identifier: Apache-2.0
#![cfg_attr(windows, windows_subsystem = "windows")]

mod autostart;
#[cfg_attr(windows, allow(dead_code))]
mod autostart_files;
mod bridge;
mod cli;
mod commands;
mod console;
mod gui;
mod instance;
mod quoting;
mod setup;
mod shell;
mod uninstall;

use std::process::ExitCode;

use clap::Parser;
use cockpit_core::{logging, paths, settings};

use cli::{Cli, Command};

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            // Help, version and usage errors are printed for a person.
            console::attach_to_parent();
            error.exit()
        }
    };
    match cli.command {
        None => run_window(),
        // The bridge never attaches to a console: its output goes to Claude Code.
        Some(Command::Bridge) => run_bridge(),
        Some(Command::SetupBridge { yes }) => {
            console::attach_to_parent();
            commands::setup_bridge(yes)
        }
        Some(Command::RemoveBridge { yes }) => {
            console::attach_to_parent();
            commands::remove_bridge(yes)
        }
        Some(Command::Uninstall { yes, remove_data }) => {
            console::attach_to_parent();
            commands::uninstall(yes, remove_data)
        }
        // Started by the window, without a console and without output.
        Some(Command::FinishUninstall { after }) => commands::finish_uninstall(after),
    }
}

/// Opens the cockpit window. Errors go to the log; the window program has no console.
fn run_window() -> ExitCode {
    let (data_dir, config_dir) = match (paths::data_dir(), paths::config_dir()) {
        (Ok(data), Ok(config)) => (data, config),
        _ => return ExitCode::FAILURE,
    };
    let _ = logging::init(&data_dir, "cockpit", logging::DEFAULT_MAX_BYTES);
    // Held until the window is closed; a second cockpit only tells the person and ends.
    let _guard = match instance::acquire(&data_dir) {
        Ok(guard) => guard,
        Err(instance::AlreadyRunning) => {
            log::info!("another cockpit is already running; this start ends");
            return match gui::show_already_running() {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    log::error!("the notice window stopped with an error: {error}");
                    ExitCode::SUCCESS
                }
            };
        }
    };
    let settings_path = config_dir.join("settings.toml");
    let settings = settings::load(&settings_path);
    match gui::run(&settings, &settings_path, &data_dir) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            log::error!("the window stopped with an error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Runs the status line bridge. Whatever happens, the exit code is 0 and Claude Code gets a text.
fn run_bridge() -> ExitCode {
    let (data_dir, config_dir) = match (paths::data_dir(), paths::config_dir()) {
        (Ok(data), Ok(config)) => (data, config),
        _ => {
            // No usable home directory: nothing can be stored, but the answer must still come.
            print_no_data();
            return ExitCode::SUCCESS;
        }
    };
    // Logging is best effort; the bridge works without it.
    let _ = logging::init(&data_dir, "bridge", logging::DEFAULT_MAX_BYTES);
    let finished = guarded(|| {
        bridge::run(
            std::io::stdin().lock(),
            std::io::stdout().lock(),
            &data_dir,
            &config_dir,
        )
    });
    if finished.is_none() {
        // An unexpected panic inside the bridge: still answer, still exit with 0.
        print_no_data();
    }
    ExitCode::SUCCESS
}

/// Runs `f`; `None` if it panicked.
fn guarded(f: impl FnOnce() -> i32) -> Option<i32> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).ok()
}

/// Writes the no-data text; a closed output is ignored (a plain `println!` would panic).
fn print_no_data() {
    use std::io::Write;
    let _ = writeln!(std::io::stdout(), "{}", bridge::NO_DATA_TEXT);
}

#[cfg(test)]
mod tests {
    use super::guarded;

    /// The bridge must start without any GUI code (REQ-109: under 100 ms, no GUI initialisation).
    #[test]
    fn req_018_gui_module_not_used_by_bridge() {
        for (name, source) in [
            ("bridge.rs", include_str!("bridge.rs")),
            ("shell.rs", include_str!("shell.rs")),
        ] {
            for word in ["gui::", "eframe", "egui"] {
                assert!(!source.contains(word), "{name} mentions {word}");
            }
        }
    }

    #[test]
    fn req_109_guarded_returns_the_result_of_a_normal_run() {
        assert_eq!(guarded(|| 0), Some(0));
    }

    #[test]
    fn req_109_guarded_turns_a_panic_into_none() {
        assert_eq!(
            guarded(|| panic!("simulated failure inside the bridge")),
            None
        );
    }
}
