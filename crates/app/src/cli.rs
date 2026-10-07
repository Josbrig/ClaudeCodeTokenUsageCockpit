// SPDX-License-Identifier: Apache-2.0
//! The command line of `usage-cockpit` (concept §3).

use clap::{Parser, Subcommand};

/// Always-on-top cockpit for the Claude usage limits, including the status line bridge.
#[derive(Debug, Parser)]
#[command(
    name = "usage-cockpit",
    version = concat!(env!("CARGO_PKG_VERSION"), " (", env!("GIT_COMMIT"), ")"),
    about
)]
pub struct Cli {
    /// What to do; without a command the cockpit window opens.
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// The subcommands.
#[derive(Debug, PartialEq, Eq, Subcommand)]
pub enum Command {
    /// Status line bridge: reads one record from standard input (started by Claude Code).
    Bridge,
    /// Set up the bridge as the Claude Code status line command.
    SetupBridge {
        /// Do not ask for confirmation.
        #[arg(long)]
        yes: bool,
    },
    /// Remove the bridge from the Claude Code settings and restore the previous status line.
    RemoveBridge {
        /// Do not ask for confirmation.
        #[arg(long)]
        yes: bool,
    },
    /// Undo everything the cockpit created outside its own file (bridge, start entry and, with
    /// --remove-data, its data and configuration folders).
    Uninstall {
        /// Do not ask for confirmation.
        #[arg(long)]
        yes: bool,
        /// Also delete the history, logs and settings.
        #[arg(long)]
        remove_data: bool,
    },
    /// Started by the window after "Remove everything": waits until that process ended, then
    /// deletes the data and configuration folders.
    #[command(hide = true)]
    FinishUninstall {
        /// Process id to wait for.
        #[arg(long)]
        after: u32,
    },
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Cli, Command};

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("usage-cockpit").chain(args.iter().copied()))
    }

    #[test]
    fn req_032_no_subcommand_means_the_window() {
        assert_eq!(parse(&[]).unwrap().command, None);
    }

    #[test]
    fn req_032_bridge_subcommand() {
        assert_eq!(parse(&["bridge"]).unwrap().command, Some(Command::Bridge));
    }

    #[test]
    fn req_032_setup_and_remove_take_an_optional_yes() {
        assert_eq!(
            parse(&["setup-bridge"]).unwrap().command,
            Some(Command::SetupBridge { yes: false })
        );
        assert_eq!(
            parse(&["setup-bridge", "--yes"]).unwrap().command,
            Some(Command::SetupBridge { yes: true })
        );
        assert_eq!(
            parse(&["remove-bridge", "--yes"]).unwrap().command,
            Some(Command::RemoveBridge { yes: true })
        );
    }

    #[test]
    fn req_119_uninstall_takes_yes_and_remove_data() {
        assert_eq!(
            parse(&["uninstall"]).unwrap().command,
            Some(Command::Uninstall {
                yes: false,
                remove_data: false
            })
        );
        assert_eq!(
            parse(&["uninstall", "--yes", "--remove-data"])
                .unwrap()
                .command,
            Some(Command::Uninstall {
                yes: true,
                remove_data: true
            })
        );
        assert_eq!(
            parse(&["finish-uninstall", "--after", "42"])
                .unwrap()
                .command,
            Some(Command::FinishUninstall { after: 42 })
        );
        assert!(parse(&["finish-uninstall"]).is_err());
    }

    #[test]
    fn req_032_unknown_input_is_rejected() {
        assert!(parse(&["frobnicate"]).is_err());
        assert!(parse(&["bridge", "--yes"]).is_err());
    }
}
