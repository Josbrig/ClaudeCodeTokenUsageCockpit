// SPDX-License-Identifier: Apache-2.0
#![cfg_attr(windows, windows_subsystem = "windows")]

mod cli;
mod console;

use std::process::ExitCode;

use clap::Parser;

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
        Some(Command::Bridge) => not_implemented("bridge"),
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

fn not_implemented(command: &str) -> ExitCode {
    eprintln!("usage-cockpit {command}: not implemented yet");
    ExitCode::from(2)
}
