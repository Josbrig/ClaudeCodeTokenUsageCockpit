// SPDX-License-Identifier: Apache-2.0
#![cfg_attr(windows, windows_subsystem = "windows")]

mod bridge;
mod cli;
mod commands;
mod console;
mod setup;
mod shell;

use std::process::ExitCode;

use clap::Parser;
use cockpit_core::{logging, paths};

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
        // The window follows in a later issue.
        None => ExitCode::SUCCESS,
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
