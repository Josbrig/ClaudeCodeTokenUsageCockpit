// SPDX-License-Identifier: Apache-2.0
//! `uninstall` through the real executable (REQ-119). `CLAUDE_CONFIG_DIR` and `USAGE_COCKPIT_HOME`
//! point everything into a temporary folder; with `USAGE_COCKPIT_HOME` set the program uses a start
//! entry name of its own, so the person's real entry is never touched.

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

/// A bridge run, so that the data folder holds what the program really writes there.
fn feed_the_bridge(home: &Path) {
    let record = json!({
        "session_id": "t",
        "rate_limits": {
            "five_hour": {"used_percentage": 23.5, "resets_at": 4_102_444_800_u64},
            "seven_day": {"used_percentage": 41.2, "resets_at": 4_102_444_800_u64}
        }
    });
    cockpit(home, &["bridge"])
        .write_stdin(record.to_string())
        .assert()
        .success();
}

fn old_settings() -> Value {
    json!({"model": "x", "statusLine": {"type": "command", "command": "ccusage"}})
}

fn prepare(home: &Path) {
    fs::create_dir_all(home.join("claude")).unwrap();
    fs::write(settings_path(home), old_settings().to_string()).unwrap();
    cockpit(home, &["setup-bridge", "--yes"]).assert().success();
    feed_the_bridge(home);
    assert!(home.join("data").join("latest.json").exists());
    assert!(home.join("config").join("bridge-state.json").exists());
}

#[test]
fn req_119_uninstall_with_data_leaves_no_file_of_the_cockpit() {
    let home = tempfile::tempdir().unwrap();
    prepare(home.path());

    cockpit(home.path(), &["uninstall", "--yes", "--remove-data"])
        .assert()
        .success()
        .stdout(predicate::str::contains("delete the program file by hand"));

    assert!(!home.path().join("data").exists(), "data folder is gone");
    assert!(
        !home.path().join("config").exists(),
        "config folder is gone"
    );
    assert_eq!(read_json(&settings_path(home.path())), old_settings());
    let backups = fs::read_dir(home.path().join("claude"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .contains("usage-cockpit-backup")
        })
        .count();
    assert!(backups >= 1, "the backups of the settings stay");
}

#[test]
fn req_119_uninstall_without_remove_data_keeps_history_and_settings() {
    let home = tempfile::tempdir().unwrap();
    prepare(home.path());

    cockpit(home.path(), &["uninstall", "--yes"])
        .assert()
        .success()
        .stdout(predicate::str::contains("kept"));

    assert!(home.path().join("data").join("latest.json").exists());
    assert_eq!(read_json(&settings_path(home.path())), old_settings());
}

#[test]
fn req_119_uninstall_on_a_clean_system_changes_nothing_and_succeeds() {
    let home = tempfile::tempdir().unwrap();
    cockpit(home.path(), &["uninstall", "--yes", "--remove-data"])
        .assert()
        .success();
    assert!(!settings_path(home.path()).exists());
}

#[cfg(windows)]
#[test]
fn req_119_windows_needs_yes() {
    let home = tempfile::tempdir().unwrap();
    cockpit(home.path(), &["uninstall"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("--yes"));
}
