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

/// Git for Windows bash, the shell Claude Code uses on Windows; `None` if it is not installed.
#[cfg(windows)]
fn git_bash() -> Option<std::path::PathBuf> {
    let machine = ["ProgramFiles", "ProgramFiles(x86)"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|base| Path::new(&base).join("Git").join("bin").join("bash.exe"));
    let user = std::env::var_os("LOCALAPPDATA").map(|base| {
        Path::new(&base)
            .join("Programs")
            .join("Git")
            .join("bin")
            .join("bash.exe")
    });
    machine.chain(user).find(|path| path.is_file())
}

/// Runs `command` through Git Bash with a record on standard input; returns what it printed.
#[cfg(windows)]
fn record_text() -> String {
    format!(
        "{{\"session_id\":\"t\",\"rate_limits\":{{\"five_hour\":{{\"used_percentage\":23.5,\"resets_at\":{}}},\"seven_day\":{{\"used_percentage\":41.2,\"resets_at\":{}}}}}}}",
        4_102_444_800_u64, 4_102_444_800_u64
    )
}

#[cfg(windows)]
fn run_in_bash(bash: &Path, command: &str, home: &Path) -> String {
    let record = record_text();
    let output = Command::new(bash)
        .args(["-c", command])
        .env("USAGE_COCKPIT_HOME", home)
        .write_stdin(record)
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[cfg(windows)]
#[test]
fn req_117_the_bridge_runs_from_a_folder_with_a_space() {
    let Some(bash) = git_bash() else {
        eprintln!("Git for Windows bash not found: the shell part of this test is skipped");
        return;
    };
    let home = tempfile::tempdir().unwrap();
    let folder = home.path().join("my tools");
    fs::create_dir_all(&folder).unwrap();
    let exe = folder.join("usage-cockpit.exe");
    fs::copy(assert_cmd::cargo::cargo_bin!("usage-cockpit"), &exe).unwrap();

    Command::new(&exe)
        .args(["setup-bridge", "--yes"])
        .env("USAGE_COCKPIT_HOME", home.path())
        .env("CLAUDE_CONFIG_DIR", home.path().join("claude"))
        .assert()
        .success();
    let settings = read_json(&settings_path(home.path()));
    let command = settings["statusLine"]["command"].as_str().unwrap();
    assert!(command.ends_with(" bridge"), "{command}");
    assert_eq!(
        run_in_bash(&bash, command, home.path()).trim(),
        "5h 23.5% \u{b7} 7d 41.2%",
        "command: {command}"
    );

    // the short form (no quote) also runs in Windows PowerShell
    if !command.starts_with('"') {
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", command])
            .env("USAGE_COCKPIT_HOME", home.path())
            .write_stdin(record_text())
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "5h 23.5% \u{b7} 7d 41.2%",
            "powershell, command: {command}"
        );
    }

    // the quoted form, used when a volume has no 8.3 names, runs in bash as well
    let quoted = format!("\"{}\" bridge", exe.to_string_lossy().replace('\\', "/"));
    assert_eq!(
        run_in_bash(&bash, &quoted, home.path()).trim(),
        "5h 23.5% \u{b7} 7d 41.2%",
        "command: {quoted}"
    );

    Command::new(&exe)
        .args(["remove-bridge", "--yes"])
        .env("USAGE_COCKPIT_HOME", home.path())
        .env("CLAUDE_CONFIG_DIR", home.path().join("claude"))
        .assert()
        .success()
        .stdout(predicate::str::contains("removed"));
    assert!(
        read_json(&settings_path(home.path()))
            .get("statusLine")
            .is_none()
    );
}

// The quoting of the command for `sh -c` (Linux and macOS) is checked against a real POSIX shell:
// Git for Windows bash reads single quotes the way `sh` does. The module is the same file that the
// program uses.
#[allow(dead_code)]
#[path = "../src/quoting.rs"]
mod quoting;

#[cfg(windows)]
#[test]
fn req_117_a_sh_quoted_command_runs_from_a_folder_with_a_space_a_quote_and_a_dollar_sign() {
    let Some(bash) = git_bash() else {
        eprintln!("Git for Windows bash not found: this test is skipped");
        return;
    };
    let home = tempfile::tempdir().unwrap();
    let folder = home.path().join("it's my $tools");
    fs::create_dir_all(&folder).unwrap();
    let exe = folder.join("usage-cockpit.exe");
    fs::copy(assert_cmd::cargo::cargo_bin!("usage-cockpit"), &exe).unwrap();
    let path = exe.to_string_lossy().replace('\\', "/");
    let command = format!("{} bridge", quoting::sh_quote(&path));
    assert!(command.starts_with('\''), "{command}");
    assert_eq!(
        run_in_bash(&bash, &command, home.path()).trim(),
        "5h 23.5% \u{b7} 7d 41.2%",
        "command: {command}"
    );
}

/// Runs `setup-bridge --yes` from a copy of the program in a folder with a space, an apostrophe and
/// a dollar sign, then returns the command it wrote into the settings.
fn command_written_from_a_difficult_folder(home: &Path) -> (std::path::PathBuf, String) {
    let folder = home.join("it's my $tools");
    fs::create_dir_all(&folder).unwrap();
    let exe = folder.join(if cfg!(windows) {
        "usage-cockpit.exe"
    } else {
        "usage-cockpit"
    });
    fs::copy(assert_cmd::cargo::cargo_bin!("usage-cockpit"), &exe).unwrap();
    Command::new(&exe)
        .args(["setup-bridge", "--yes"])
        .env("USAGE_COCKPIT_HOME", home)
        .env("CLAUDE_CONFIG_DIR", home.join("claude"))
        .assert()
        .success();
    let settings = read_json(&settings_path(home));
    let command = settings["statusLine"]["command"]
        .as_str()
        .unwrap()
        .to_owned();
    (exe, command)
}

#[cfg(windows)]
#[test]
fn req_117_the_command_that_setup_writes_runs_from_a_folder_with_a_quote_and_a_dollar_sign() {
    let Some(bash) = git_bash() else {
        eprintln!("Git for Windows bash not found: this test is skipped");
        return;
    };
    let home = tempfile::tempdir().unwrap();
    let (exe, command) = command_written_from_a_difficult_folder(home.path());
    assert_eq!(
        run_in_bash(&bash, &command, home.path()).trim(),
        "5h 23.5% \u{b7} 7d 41.2%",
        "command: {command}"
    );
    // setting up again from the same place is recognised, and the removal finds the command
    Command::new(&exe)
        .args(["setup-bridge", "--yes"])
        .env("USAGE_COCKPIT_HOME", home.path())
        .env("CLAUDE_CONFIG_DIR", home.path().join("claude"))
        .assert()
        .success()
        .stdout(predicate::str::contains("already set up"));
    Command::new(&exe)
        .args(["remove-bridge", "--yes"])
        .env("USAGE_COCKPIT_HOME", home.path())
        .env("CLAUDE_CONFIG_DIR", home.path().join("claude"))
        .assert()
        .success()
        .stdout(predicate::str::contains("removed"));
    assert!(
        read_json(&settings_path(home.path()))
            .get("statusLine")
            .is_none()
    );
}

/// The same on Linux and macOS with the system shell. NOT RUN on the development machine (Windows);
/// it is the check that the human issues for those systems ask for.
#[cfg(unix)]
#[test]
fn req_117_the_command_that_setup_writes_runs_from_a_folder_with_a_quote_and_a_dollar_sign() {
    let home = tempfile::tempdir().unwrap();
    let (exe, command) = command_written_from_a_difficult_folder(home.path());
    let record = format!(
        "{{\"session_id\":\"t\",\"rate_limits\":{{\"five_hour\":{{\"used_percentage\":23.5,\"resets_at\":{}}},\"seven_day\":{{\"used_percentage\":41.2,\"resets_at\":{}}}}}}}",
        4_102_444_800_u64, 4_102_444_800_u64
    );
    let output = Command::new("sh")
        .args(["-c", &command])
        .env("USAGE_COCKPIT_HOME", home.path())
        .write_stdin(record)
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "5h 23.5% \u{b7} 7d 41.2%",
        "command: {command}"
    );
    Command::new(&exe)
        .args(["remove-bridge", "--yes"])
        .env("USAGE_COCKPIT_HOME", home.path())
        .env("CLAUDE_CONFIG_DIR", home.path().join("claude"))
        .assert()
        .success();
    assert!(
        read_json(&settings_path(home.path()))
            .get("statusLine")
            .is_none()
    );
}
