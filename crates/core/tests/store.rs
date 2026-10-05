// SPDX-License-Identifier: Apache-2.0

use std::fs::{self, File};
use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

use cockpit_core::model::{Record, WindowSample};
use cockpit_core::store::{
    HISTORY_FILE, LAST_ERROR_FILE, LATEST_FILE, StoreError, append_history, cleanup_temp_files,
    lock_history, read_history, read_last_error, read_latest, write_last_error, write_latest,
};

const TIMEOUT: Duration = Duration::from_secs(30);

fn record(n: i64) -> Record {
    Record {
        received_at_ms: n,
        session_id: Some(format!("session-{n}")),
        cc_version: Some("2.1.90".to_owned()),
        five_hour: Some(WindowSample {
            used_pct: 23.5,
            resets_at: 1_738_425_600,
        }),
        seven_day: None,
        model: Some("Test Model".to_owned()),
        context_used_pct: None,
        cost_usd: Some(0.5),
    }
}

fn history_path(dir: &Path) -> std::path::PathBuf {
    dir.join(HISTORY_FILE)
}

fn tmp_files(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp"))
        .collect()
}

// ---- latest.json ----------------------------------------------------------------------

#[test]
fn req_020_latest_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    write_latest(dir.path(), &record(1)).unwrap();
    assert_eq!(read_latest(dir.path()).unwrap(), Some(record(1)));
}

#[test]
fn req_020_latest_file_has_version_and_record() {
    let dir = tempfile::tempdir().unwrap();
    write_latest(dir.path(), &record(7)).unwrap();
    let text = fs::read_to_string(dir.path().join(LATEST_FILE)).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(json["v"], 1);
    assert_eq!(json["record"]["received_at_ms"], 7);
    assert!(tmp_files(dir.path()).is_empty());
}

#[test]
fn req_020_latest_replaces_existing() {
    let dir = tempfile::tempdir().unwrap();
    write_latest(dir.path(), &record(1)).unwrap();
    write_latest(dir.path(), &record(2)).unwrap();
    assert_eq!(read_latest(dir.path()).unwrap(), Some(record(2)));
    assert!(tmp_files(dir.path()).is_empty());
}

#[test]
fn req_020_read_latest_missing_is_none() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(read_latest(dir.path()).unwrap(), None);
}

#[test]
fn req_115_latest_with_unknown_version_is_a_format_error() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(LATEST_FILE),
        r#"{"v":2,"record":{"received_at_ms":1}}"#,
    )
    .unwrap();
    assert!(matches!(
        read_latest(dir.path()),
        Err(StoreError::Format(_))
    ));
}

#[test]
fn req_108_unparsable_latest_is_a_format_error() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(LATEST_FILE), "{ broken").unwrap();
    assert!(matches!(
        read_latest(dir.path()),
        Err(StoreError::Format(_))
    ));
}

#[test]
fn req_020_unique_temp_names_for_concurrent_latest_writes() {
    let dir = Arc::new(tempfile::tempdir().unwrap());
    let handles: Vec<_> = (0..8)
        .map(|t| {
            let dir = Arc::clone(&dir);
            thread::spawn(move || {
                (0..25)
                    .filter(|i| write_latest(dir.path(), &record(t * 100 + i)).is_ok())
                    .count()
            })
        })
        .collect();
    let succeeded: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();
    assert!(succeeded > 0);
    // The final file is one complete record, and no temp file is left behind.
    assert!(read_latest(dir.path()).unwrap().is_some());
    assert!(
        tmp_files(dir.path()).is_empty(),
        "{:?}",
        tmp_files(dir.path())
    );
}

// ---- last_error.json ------------------------------------------------------------------

#[test]
fn req_108_last_error_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(read_last_error(dir.path()), None);
    write_last_error(dir.path(), 1_738_400_123_456).unwrap();
    assert_eq!(read_last_error(dir.path()), Some(1_738_400_123_456));
    let text = fs::read_to_string(dir.path().join(LAST_ERROR_FILE)).unwrap();
    assert!(text.contains(r#""kind":"malformed""#), "{text}");
}

#[test]
fn req_108_unreadable_last_error_is_none() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(LAST_ERROR_FILE), "not json").unwrap();
    assert_eq!(read_last_error(dir.path()), None);
    fs::write(
        dir.path().join(LAST_ERROR_FILE),
        r#"{"v":9,"received_at_ms":5,"kind":"malformed"}"#,
    )
    .unwrap();
    assert_eq!(read_last_error(dir.path()), None);
}

// ---- history --------------------------------------------------------------------------

#[test]
fn req_013_history_append_and_read() {
    let dir = tempfile::tempdir().unwrap();
    for n in 1..=3 {
        append_history(dir.path(), &record(n), TIMEOUT).unwrap();
    }
    let read = read_history(dir.path()).unwrap();
    assert_eq!(read.records, vec![record(1), record(2), record(3)]);
    assert_eq!(read.skipped, 0);
}

#[test]
fn req_013_missing_history_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let read = read_history(dir.path()).unwrap();
    assert!(read.records.is_empty());
    assert_eq!(read.skipped, 0);
}

#[test]
fn req_020_history_line_is_flat_with_version() {
    let dir = tempfile::tempdir().unwrap();
    append_history(dir.path(), &record(5), TIMEOUT).unwrap();
    let text = fs::read_to_string(history_path(dir.path())).unwrap();
    assert!(text.ends_with('\n'));
    let json: serde_json::Value = serde_json::from_str(text.trim_end()).unwrap();
    assert_eq!(json["v"], 1);
    assert_eq!(json["received_at_ms"], 5);
    assert_eq!(json["five_hour"]["used_pct"], 23.5);
}

#[test]
fn req_020_append_creates_the_directory() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("a").join("b");
    append_history(&nested, &record(1), TIMEOUT).unwrap();
    assert_eq!(read_history(&nested).unwrap().records.len(), 1);
}

#[test]
fn req_020_reader_ignores_partial_last_line() {
    let dir = tempfile::tempdir().unwrap();
    append_history(dir.path(), &record(1), TIMEOUT).unwrap();
    let mut text = fs::read_to_string(history_path(dir.path())).unwrap();
    text.push_str(r#"{"v":1,"received_at_ms":2,"sess"#);
    fs::write(history_path(dir.path()), text).unwrap();
    let read = read_history(dir.path()).unwrap();
    assert_eq!(read.records, vec![record(1)]);
    assert_eq!(read.skipped, 1);
}

#[test]
fn req_020_append_repairs_missing_newline() {
    let dir = tempfile::tempdir().unwrap();
    append_history(dir.path(), &record(1), TIMEOUT).unwrap();
    let mut text = fs::read_to_string(history_path(dir.path())).unwrap();
    text.push_str(r#"{"v":1,"received_at_ms":2,"sess"#);
    fs::write(history_path(dir.path()), text).unwrap();
    append_history(dir.path(), &record(3), TIMEOUT).unwrap();
    let read = read_history(dir.path()).unwrap();
    // The old partial line is skipped, and the new record is read intact.
    assert_eq!(read.records, vec![record(1), record(3)]);
    assert_eq!(read.skipped, 1);
}

#[test]
fn req_115_unknown_version_line_skipped() {
    let dir = tempfile::tempdir().unwrap();
    append_history(dir.path(), &record(1), TIMEOUT).unwrap();
    let mut text = fs::read_to_string(history_path(dir.path())).unwrap();
    text.push_str("{\"v\":2,\"received_at_ms\":2}\n");
    fs::write(history_path(dir.path()), text).unwrap();
    append_history(dir.path(), &record(3), TIMEOUT).unwrap();
    let read = read_history(dir.path()).unwrap();
    assert_eq!(read.records, vec![record(1), record(3)]);
    assert_eq!(read.skipped, 1);
}

#[test]
fn req_108_malformed_history_lines_are_skipped_wherever_they_are() {
    let dir = tempfile::tempdir().unwrap();
    let good = serde_json::to_string(&serde_json::json!({"v":1,"received_at_ms":1})).unwrap();
    let text = format!("garbage\n{good}\n\n[1,2]\n{{\"received_at_ms\":4}}\n{good}\n");
    fs::write(history_path(dir.path()), text).unwrap();
    let read = read_history(dir.path()).unwrap();
    assert_eq!(read.records.len(), 2);
    // "garbage", the array and the line without a version; the empty line does not count.
    assert_eq!(read.skipped, 3);
}

#[test]
fn req_020_concurrent_appends_keep_lines_intact() {
    let dir = Arc::new(tempfile::tempdir().unwrap());
    let handles: Vec<_> = (0..8)
        .map(|t| {
            let dir = Arc::clone(&dir);
            thread::spawn(move || {
                for i in 0..200 {
                    append_history(dir.path(), &record(t * 1000 + i), TIMEOUT).unwrap();
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    let read = read_history(dir.path()).unwrap();
    assert_eq!(read.skipped, 0);
    assert_eq!(read.records.len(), 1_600);
}

#[test]
fn req_020_lock_timeout_returns_lock_error() {
    let dir = tempfile::tempdir().unwrap();
    let _held = lock_history(dir.path(), TIMEOUT).unwrap();
    let err = append_history(dir.path(), &record(1), Duration::from_millis(60)).unwrap_err();
    assert!(matches!(err, StoreError::Lock), "{err}");
}

#[test]
fn req_020_lock_is_released_when_dropped() {
    let dir = tempfile::tempdir().unwrap();
    drop(lock_history(dir.path(), TIMEOUT).unwrap());
    append_history(dir.path(), &record(1), Duration::from_millis(60)).unwrap();
}

// ---- temp file cleanup ----------------------------------------------------------------

#[test]
fn req_020_cleanup_removes_only_old_temp_files() {
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("latest.json.1.2.3.tmp");
    let fresh = dir.path().join("latest.json.4.5.6.tmp");
    let other = dir.path().join("settings.toml");
    for p in [&old, &fresh, &other] {
        fs::write(p, b"x").unwrap();
    }
    let two_hours_ago = SystemTime::now() - Duration::from_secs(2 * 3600);
    File::options()
        .write(true)
        .open(&old)
        .unwrap()
        .set_modified(two_hours_ago)
        .unwrap();
    let removed = cleanup_temp_files(dir.path(), Duration::from_secs(60));
    assert_eq!(removed, 1);
    assert!(!old.exists());
    assert!(fresh.exists());
    assert!(other.exists());
}
