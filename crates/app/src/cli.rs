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
    fn req_032_unknown_input_is_rejected() {
        assert!(parse(&["frobnicate"]).is_err());
        assert!(parse(&["bridge", "--yes"]).is_err());
    }
}
