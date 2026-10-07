// SPDX-License-Identifier: Apache-2.0
//! The real bridge with hostile input, and the promise that credentials never reach the files
//! the bridge writes. `USAGE_COCKPIT_HOME` points the folders into a temporary folder.

use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;

const FAKE_KEY: &str = "sk-ant-FAKE-TEST-123";
const FAKE_TOKEN: &str = "FAKE-OAUTH-TOKEN-456";

fn bridge(home: &Path) -> Command {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("usage-cockpit"));
    cmd.arg("bridge").env("USAGE_COCKPIT_HOME", home);
    cmd
}

fn files_below(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(files_below(&path));
        } else {
            found.push(path);
        }
    }
    found
}

#[test]
fn req_108_bridge_survives_hostile_input_and_always_answers() {
    let home = tempfile::tempdir().unwrap();
    let huge = format!(r#"{{"padding":"{}"}}"#, "x".repeat(2 * 1024 * 1024));
    let deep = format!("{}1{}", "[".repeat(50_000), "]".repeat(50_000));
    let inputs: Vec<Vec<u8>> = vec![
        b"{\"rate_limits\":{\"five_hour\":{\"used_percentage\":".to_vec(),
        br#"{"rate_limits":{"five_hour":{"used_percentage":"NaN","resets_at":"x"}}}"#.to_vec(),
        br#"{"rate_limits":{"five_hour":{"used_percentage":1e999,"resets_at":1}}}"#.to_vec(),
        br#"{"rate_limits":{"five_hour":{"used_percentage":50,"resets_at":9223372036854775807}}}"#
            .to_vec(),
        vec![0xff, 0xfe, 0x00, 0x7b, 0xc3, 0x28, 0x80],
        (0..=255u8).collect(),
        huge.into_bytes(),
        deep.into_bytes(),
        Vec::new(),
    ];
    for input in inputs {
        bridge(home.path())
            .write_stdin(input)
            .assert()
            .success()
            .stdout(predicate::str::is_empty().not());
    }
}

#[test]
fn req_108_the_cockpit_data_files_stay_readable_after_hostile_input() {
    let home = tempfile::tempdir().unwrap();
    let good = r#"{"rate_limits":{"five_hour":{"used_percentage":12,"resets_at":1738425600}}}"#;
    bridge(home.path()).write_stdin(good).assert().success();
    bridge(home.path())
        .write_stdin("{ broken")
        .assert()
        .success();
    let data = home.path().join("data");
    let latest = cockpit_core::store::read_latest(&data).unwrap().unwrap();
    assert_eq!(
        latest.five_hour.unwrap().used_pct,
        12.0,
        "the good record stays"
    );
    assert!(cockpit_core::store::read_last_error(&data).is_some());
    assert_eq!(
        cockpit_core::store::read_history(&data)
            .unwrap()
            .records
            .len(),
        1
    );
}

#[test]
fn req_031_no_seeded_credentials_in_logs() {
    let home = tempfile::tempdir().unwrap();
    // A record with the fake credential in fields the bridge does not use, then broken inputs
    // that carry it too: whatever the bridge logs, it must not repeat any of it.
    let with_secret = format!(
        r#"{{"session_id":"s","version":"1.0.0","api_key":"{FAKE_KEY}","auth":{{"token":"{FAKE_TOKEN}"}},
            "rate_limits":{{"five_hour":{{"used_percentage":10,"resets_at":1738425600}}}}}}"#
    );
    let newer_version = with_secret.replace("1.0.0", "2.0.0");
    let broken = format!(r#"{{"api_key":"{FAKE_KEY}","token":"{FAKE_TOKEN}""#);
    for input in [with_secret, newer_version, broken] {
        bridge(home.path())
            .env("ANTHROPIC_API_KEY", FAKE_KEY)
            .env("CLAUDE_CODE_OAUTH_TOKEN", FAKE_TOKEN)
            .write_stdin(input)
            .assert()
            .success();
    }
    let files = files_below(home.path());
    // The broken input must have produced a log line about itself, so the check is not empty.
    let log: String = files
        .iter()
        .filter(|f| {
            f.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("log"))
        })
        .map(|f| fs::read_to_string(f).unwrap_or_default())
        .collect();
    assert!(
        log.contains("input could not be used"),
        "no log line for the broken input: {log:?}"
    );
    for file in files {
        let bytes = fs::read(&file).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        for secret in [FAKE_KEY, FAKE_TOKEN] {
            assert!(
                !text.contains(secret),
                "{} contains a seeded credential",
                file.display()
            );
        }
    }
}
