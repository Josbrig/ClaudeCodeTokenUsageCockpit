// SPDX-License-Identifier: Apache-2.0
//! `setup-bridge` and `remove-bridge` through the real executable. `CLAUDE_CONFIG_DIR` and
//! `USAGE_COCKPIT_HOME` point the settings and the bridge state into a temporary folder.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::{Value, json};

fn cockpit(home: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("usage-cockpit"));
    cmd.args(args)
        .env("USAGE_COCKPIT_HOME", home)
        .env("CLAUDE_CONFIG_DIR", home.join("claude"));
    cmd
}

fn settings_path(home: &Path) -> std::path::PathBuf {
    home.join("claude").join("settings.json")
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn req_023_setup_and_remove_with_yes() {
    let home = tempfile::tempdir().unwrap();
    let old = json!({"model": "x", "statusLine": {"type": "command", "command": "ccusage"}});
    fs::create_dir_all(home.path().join("claude")).unwrap();
    fs::write(settings_path(home.path()), old.to_string()).unwrap();

    cockpit(home.path(), &["setup-bridge", "--yes"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Backup of the settings"));
    let new = read_json(&settings_path(home.path()));
    let command = new["statusLine"]["command"].as_str().unwrap();
    assert!(command.ends_with(" bridge"), "{command}");
    assert!(!command.contains('\\'), "forward slashes only: {command}");
    assert_eq!(new["model"], "x");

    cockpit(home.path(), &["remove-bridge", "--yes"])
        .assert()
        .success();
    assert_eq!(read_json(&settings_path(home.path())), old);
}

#[test]
fn req_023_setup_creates_a_missing_settings_file() {
    let home = tempfile::tempdir().unwrap();
    cockpit(home.path(), &["setup-bridge", "--yes"])
        .assert()
        .success();
    let settings = read_json(&settings_path(home.path()));
    assert_eq!(settings.as_object().unwrap().len(), 1);
    assert_eq!(settings["statusLine"]["type"], "command");
}

#[test]
fn req_023_remove_without_the_bridge_changes_nothing() {
    let home = tempfile::tempdir().unwrap();
    cockpit(home.path(), &["remove-bridge", "--yes"])
        .assert()
        .success()
        .stdout(predicate::str::contains("nothing changed"));
    assert!(!settings_path(home.path()).exists());
}

#[cfg(windows)]
#[test]
fn req_023_windows_needs_yes() {
    let home = tempfile::tempdir().unwrap();
    for command in ["setup-bridge", "remove-bridge"] {
        cockpit(home.path(), &[command])
            .assert()
            .code(1)
            .stderr(predicate::str::contains(
                "On Windows use --yes, or set up the bridge from the cockpit window.",
            ));
    }
    assert!(!settings_path(home.path()).exists());
}

#[cfg(not(windows))]
#[test]
fn req_023_declining_the_question_leaves_the_file_identical() {
    let home = tempfile::tempdir().unwrap();
    let original = "{ \"a\": 1 }\n";
    fs::create_dir_all(home.path().join("claude")).unwrap();
    fs::write(settings_path(home.path()), original).unwrap();
    for answer in ["n\n", "\n", ""] {
        cockpit(home.path(), &["setup-bridge"])
            .write_stdin(answer)
            .assert()
            .code(1)
            .stdout(predicate::str::contains(
                "Change Claude Code settings? [y/N]",
            ));
        assert_eq!(
            fs::read(settings_path(home.path())).unwrap(),
            original.as_bytes()
        );
    }
    cockpit(home.path(), &["setup-bridge"])
        .write_stdin("y\n")
        .assert()
        .success();
    assert!(
        read_json(&settings_path(home.path()))
            .get("statusLine")
            .is_some()
    );
}
