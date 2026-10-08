// SPDX-License-Identifier: Apache-2.0
//! The bridge in front of a kept user status line command: the real executable, with
//! `USAGE_COCKPIT_HOME` in a temporary folder and `config/bridge-state.json` naming the command.
//! The commands use POSIX syntax, which `sh` and Git for Windows bash both understand.

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use assert_cmd::Command;
use cockpit_core::store::read_latest;

const RECORD: &str = r#"{"session_id":"chain","rate_limits":{
    "five_hour":{"used_percentage":23.5,"resets_at":1738425600},
    "seven_day":{"used_percentage":41.2,"resets_at":1738857600}}}"#;
const OWN_TEXT: &str = "5h 23.5% · 7d 41.2%\n";

fn bridge_with_kept(home: &Path, command: &str) -> Command {
    let config = home.join("config");
    fs::create_dir_all(&config).unwrap();
    let state = serde_json::json!({
        "v": 1,
        "previous_status_line": {"type": "command", "command": command, "padding": 0}
    });
    fs::write(config.join("bridge-state.json"), state.to_string()).unwrap();
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("usage-cockpit"));
    cmd.arg("bridge").env("USAGE_COCKPIT_HOME", home);
    cmd
}

#[test]
fn req_012_kept_command_output_is_printed() {
    let home = tempfile::tempdir().unwrap();
    bridge_with_kept(home.path(), "echo kept-ok")
        .write_stdin(RECORD)
        .assert()
        .success()
        .stdout("kept-ok\n");
    assert!(
        read_latest(&home.path().join("data")).unwrap().is_some(),
        "the record is stored as well"
    );
}

#[test]
fn req_012_kept_command_receives_the_original_input() {
    let home = tempfile::tempdir().unwrap();
    bridge_with_kept(home.path(), "cat")
        .write_stdin(RECORD)
        .assert()
        .success()
        .stdout(RECORD);
}

#[test]
fn req_109_kept_command_timeout_falls_back() {
    let home = tempfile::tempdir().unwrap();
    let started = Instant::now();
    bridge_with_kept(home.path(), "sleep 5")
        .write_stdin(RECORD)
        .assert()
        .success()
        .stdout(OWN_TEXT);
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    assert!(read_latest(&home.path().join("data")).unwrap().is_some());
}

#[test]
fn req_012_failing_kept_command_falls_back() {
    let home = tempfile::tempdir().unwrap();
    bridge_with_kept(home.path(), "echo not-wanted; exit 3")
        .write_stdin(RECORD)
        .assert()
        .success()
        .stdout(OWN_TEXT);
}

#[test]
fn req_012_empty_output_falls_back() {
    let home = tempfile::tempdir().unwrap();
    bridge_with_kept(home.path(), "true")
        .write_stdin(RECORD)
        .assert()
        .success()
        .stdout(OWN_TEXT);
}

#[test]
fn req_012_kept_command_not_reading_stdin() {
    let home = tempfile::tempdir().unwrap();
    // More than a pipe buffer holds, so the write to the command fails with a broken pipe.
    let big = format!(r#"{{"padding":"{}"}}"#, "x".repeat(900 * 1024));
    bridge_with_kept(home.path(), "echo kept-ok")
        .write_stdin(big)
        .assert()
        .success()
        .stdout("kept-ok\n");
}

#[test]
fn req_012_kept_command_is_used_for_unusable_input_too() {
    let home = tempfile::tempdir().unwrap();
    bridge_with_kept(home.path(), "echo kept-ok")
        .write_stdin("{ nope")
        .assert()
        .success()
        .stdout("kept-ok\n");
}

#[test]
fn req_012_without_a_state_file_the_own_text_is_printed() {
    let home = tempfile::tempdir().unwrap();
    Command::new(assert_cmd::cargo::cargo_bin!("usage-cockpit"))
        .arg("bridge")
        .env("USAGE_COCKPIT_HOME", home.path())
        .write_stdin(RECORD)
        .assert()
        .success()
        .stdout(OWN_TEXT);
}
