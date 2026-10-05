// SPDX-License-Identifier: Apache-2.0
#![cfg_attr(windows, windows_subsystem = "windows")]

mod bridge;
mod cli;
mod console;

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
        Some(Command::SetupBridge { .. }) => {
            console::attach_to_parent();
            not_implemented("setup-bridge")
        }
        Some(Command::RemoveBridge { .. }) => {
            console::attach_to_parent();
            not_implemented("remove-bridge")
        }
    }
}

/// Runs the status line bridge. Whatever happens, the exit code is 0 and Claude Code gets a text.
fn run_bridge() -> ExitCode {
    let (data_dir, config_dir) = match (paths::data_dir(), paths::config_dir()) {
        (Ok(data), Ok(config)) => (data, config),
        _ => {
            // No usable home directory: nothing can be stored, but the answer must still come.
            println!("{}", bridge::NO_DATA_TEXT);
            return ExitCode::SUCCESS;
        }
    };
    // Logging is best effort; the bridge works without it.
    let _ = logging::init(&data_dir, "bridge", logging::DEFAULT_MAX_BYTES);
    let code = bridge::run(
        std::io::stdin().lock(),
        std::io::stdout().lock(),
        &data_dir,
        &config_dir,
    );
    ExitCode::from(u8::try_from(code).unwrap_or(0))
}

fn not_implemented(command: &str) -> ExitCode {
    eprintln!("usage-cockpit {command}: not implemented yet");
    ExitCode::from(2)
}
