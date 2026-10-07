// SPDX-License-Identifier: Apache-2.0

use assert_cmd::Command;
use predicates::prelude::*;

fn cockpit() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("usage-cockpit"))
}

#[test]
fn req_032_version_flag_prints_version_and_commit() {
    cockpit()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"^usage-cockpit \d+\.\d+\.\d+ \(\S+\)\n$").unwrap());
}

#[test]
fn req_032_commit_is_a_short_hash_or_unknown() {
    let out = cockpit().arg("--version").output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    let commit = text.split(['(', ')']).nth(1).unwrap();
    let is_hash = commit.len() >= 7 && commit.chars().all(|c| c.is_ascii_hexdigit());
    assert!(is_hash || commit == "unknown", "{commit:?}");
}

#[test]
fn req_032_version_matches_the_package_version() {
    cockpit()
        .arg("--version")
        .assert()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn req_032_no_arguments_exit_quietly_until_the_window_exists() {
    cockpit()
        .assert()
        .success()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::is_empty());
}

#[test]
fn req_032_help_lists_the_subcommands() {
    cockpit()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("bridge"))
        .stdout(predicate::str::contains("setup-bridge"))
        .stdout(predicate::str::contains("remove-bridge"));
}

#[test]
fn req_032_unknown_subcommand_is_a_usage_error() {
    cockpit()
        .arg("frobnicate")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("frobnicate"));
}
