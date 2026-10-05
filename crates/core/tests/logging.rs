// SPDX-License-Identifier: Apache-2.0

use std::fs;

use chrono::{TimeZone, Utc};
use cockpit_core::logging::{DEFAULT_MAX_BYTES, FileLogger, init};
use log::Level;

fn fixed_time() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 5, 14, 3, 22).unwrap()
}

#[test]
fn req_031_log_line_has_time_and_level() {
    let dir = tempfile::tempdir().unwrap();
    let logger = FileLogger::new(dir.path(), "bridge", DEFAULT_MAX_BYTES);
    logger.write_at(fixed_time(), Level::Warn, "malformed input");
    let text = fs::read_to_string(dir.path().join("log.txt")).unwrap();
    assert_eq!(text, "2026-10-05T14:03:22Z WARN bridge: malformed input\n");
}

#[test]
fn req_031_default_limit_is_five_megabytes() {
    assert_eq!(DEFAULT_MAX_BYTES, 5 * 1024 * 1024);
}

#[test]
fn req_031_log_rotates_at_limit() {
    let dir = tempfile::tempdir().unwrap();
    let logger = FileLogger::new(dir.path(), "bridge", 1_000);
    // About 1,500 bytes in total, in lines of about 60 bytes.
    for i in 0..25 {
        logger.write_at(
            fixed_time(),
            Level::Info,
            &format!("line number {i:02} padding"),
        );
    }
    let current = fs::metadata(dir.path().join("log.txt")).unwrap().len();
    assert!(current < 1_000, "log.txt has {current} bytes");
    assert!(dir.path().join("log.1.txt").exists());
    // Nothing is lost between the two files except what a second rotation replaced.
    let old = fs::read_to_string(dir.path().join("log.1.txt")).unwrap();
    assert!(old.lines().all(|l| l.contains("bridge: line number")));
}

#[test]
fn req_031_second_rotation_replaces_the_old_file() {
    let dir = tempfile::tempdir().unwrap();
    let logger = FileLogger::new(dir.path(), "bridge", 300);
    for i in 0..40 {
        logger.write_at(
            fixed_time(),
            Level::Info,
            &format!("entry {i:02} with some padding"),
        );
    }
    let old = fs::read_to_string(dir.path().join("log.1.txt")).unwrap();
    let current = fs::read_to_string(dir.path().join("log.txt")).unwrap();
    assert!(old.len() <= 300 && current.len() <= 300);
    assert!(current.contains("entry 39"));
}

#[test]
fn req_031_multi_line_message_stays_one_line() {
    let dir = tempfile::tempdir().unwrap();
    let logger = FileLogger::new(dir.path(), "cockpit", DEFAULT_MAX_BYTES);
    logger.write_at(fixed_time(), Level::Error, "first\nsecond\r\nthird");
    let text = fs::read_to_string(dir.path().join("log.txt")).unwrap();
    assert_eq!(text.lines().count(), 1, "{text:?}");
    assert!(text.contains("first second  third"));
}

#[test]
fn req_031_logging_never_fails_on_an_unusable_directory() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a-file");
    fs::write(&file, b"x").unwrap();
    // The "directory" lies below a regular file, so it can never be created.
    let logger = FileLogger::new(&file.join("logs"), "bridge", DEFAULT_MAX_BYTES);
    logger.write(Level::Error, "this goes nowhere and must not panic");
}

#[test]
fn req_031_init_installs_the_global_logger_once() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path(), "bridge", DEFAULT_MAX_BYTES).unwrap();
    log::warn!("through the facade");
    log::debug!("below the level, not written");
    let text = fs::read_to_string(dir.path().join("log.txt")).unwrap();
    assert!(text.contains(" WARN bridge: through the facade"), "{text}");
    assert!(!text.contains("below the level"), "{text}");
    assert!(init(dir.path(), "bridge", DEFAULT_MAX_BYTES).is_err());
}
