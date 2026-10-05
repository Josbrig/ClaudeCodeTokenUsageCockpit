// SPDX-License-Identifier: Apache-2.0
//! Runs the real executable as Claude Code would: record on standard input, status text on
//! standard output. `USAGE_COCKPIT_HOME` points the data directory into a temporary folder.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use cockpit_core::store::{read_history, read_last_error, read_latest};
use predicates::prelude::*;

fn bridge(home: &Path) -> Command {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("usage-cockpit"));
    cmd.arg("bridge").env("USAGE_COCKPIT_HOME", home);
    cmd
}

fn record(version: &str) -> String {
    format!(
        r#"{{"session_id":"test","version":"{version}","rate_limits":{{
            "five_hour":{{"used_percentage":23.5,"resets_at":1738425600}},
            "seven_day":{{"used_percentage":41.2,"resets_at":1738857600}}}}}}"#
    )
}

fn data_dir(home: &Path) -> std::path::PathBuf {
    home.join("data")
}

#[test]
fn req_020_bridge_stores_record() {
    let home = tempfile::tempdir().unwrap();
    bridge(home.path())
        .write_stdin(record("2.1.90"))
        .assert()
        .success();
    let latest = read_latest(&data_dir(home.path())).unwrap().unwrap();
    assert_eq!(latest.seven_day.unwrap().used_pct, 41.2);
    assert_eq!(
        read_history(&data_dir(home.path())).unwrap().records.len(),
        1
    );
}

#[test]
fn req_020_bridge_prints_usage_text() {
    let home = tempfile::tempdir().unwrap();
    bridge(home.path())
        .write_stdin(record("2.1.90"))
        .assert()
        .success()
        .stdout("5h 23.5% · 7d 41.2%\n");
}

#[test]
fn req_109_bridge_exit_zero_on_malformed_input() {
    let home = tempfile::tempdir().unwrap();
    bridge(home.path())
        .write_stdin("{ this is not json")
        .assert()
        .success()
        .stdout("usage-cockpit: no data\n");
    assert!(read_last_error(&data_dir(home.path())).is_some());
    assert_eq!(read_latest(&data_dir(home.path())).unwrap(), None);
}

#[test]
fn req_109_bridge_exit_zero_on_unwritable_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("a-file");
    fs::write(&file, b"x").unwrap();
    // The home directory would have to lie below a regular file.
    bridge(&file.join("home"))
        .write_stdin(record("2.1.90"))
        .assert()
        .success()
        .stdout("5h 23.5% · 7d 41.2%\n");
}

#[test]
fn req_109_bridge_exit_zero_without_any_input() {
    let home = tempfile::tempdir().unwrap();
    bridge(home.path())
        .assert()
        .success()
        .stdout("usage-cockpit: no data\n");
}

#[test]
fn req_031_cc_version_change_logged_once() {
    let home = tempfile::tempdir().unwrap();
    for version in ["2.1.90", "2.1.90", "2.1.91", "2.1.91"] {
        bridge(home.path())
            .write_stdin(record(version))
            .assert()
            .success();
    }
    let log = fs::read_to_string(data_dir(home.path()).join("log.txt")).unwrap();
    let changes: Vec<_> = log
        .lines()
        .filter(|l| l.contains("Claude Code version changed"))
        .collect();
    assert_eq!(changes.len(), 1, "{log}");
    assert!(changes[0].contains("2.1.90") && changes[0].contains("2.1.91"));
    assert!(changes[0].contains(" INFO bridge: "), "{}", changes[0]);
}

#[test]
fn req_031_bridge_log_never_contains_the_input() {
    let home = tempfile::tempdir().unwrap();
    let secret = "sk-ant-FAKE-TEST-VALUE-123";
    let input = format!(r#"{{"session_id":"x","unused_secret_field":"{secret}"}}"#);
    bridge(home.path())
        .env("FAKE_TOKEN_FOR_TEST", secret)
        .write_stdin(input)
        .assert()
        .success();
    bridge(home.path())
        .write_stdin(format!("{{ broken {secret}"))
        .assert()
        .success();
    let dir = data_dir(home.path());
    for entry in fs::read_dir(&dir).unwrap().flatten() {
        let text = fs::read(entry.path()).unwrap();
        let text = String::from_utf8_lossy(&text);
        assert!(
            !text.contains(secret),
            "{:?} contains the input",
            entry.path()
        );
    }
}

#[test]
fn req_109_bridge_does_not_wait_for_a_window_or_a_console() {
    // Stdout and stderr stay quiet except for the status text: no GUI start-up output.
    let home = tempfile::tempdir().unwrap();
    bridge(home.path())
        .write_stdin(record("2.1.90"))
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}
