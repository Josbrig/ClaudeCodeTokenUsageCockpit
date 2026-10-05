// SPDX-License-Identifier: Apache-2.0

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use cockpit_core::model::Record;
use cockpit_core::store::{
    HISTORY_FILE, HISTORY_KEEP_DAYS, HISTORY_MAX_BYTES, HISTORY_TARGET_BYTES, PruneStats,
    StoreError, append_history, lock_history, prune_history, prune_history_default,
    prune_history_with_wait, read_history,
};

const HOUR_MS: i64 = 3_600_000;
const DAY_MS: i64 = 24 * HOUR_MS;
const NOW_MS: i64 = 1_738_400_000_000;
const WAIT: Duration = Duration::from_secs(10);

fn record(received_at_ms: i64) -> Record {
    Record {
        received_at_ms,
        session_id: Some("s".to_owned()),
        cc_version: None,
        five_hour: None,
        seven_day: None,
        model: None,
        context_used_pct: None,
        cost_usd: None,
    }
}

/// Appends records in chronological order, `step_ms` apart, ending at `end_ms`.
fn fill(dir: &Path, count: i64, step_ms: i64, end_ms: i64) {
    for i in 0..count {
        let at = end_ms - (count - 1 - i) * step_ms;
        append_history(dir, &record(at), WAIT).unwrap();
    }
}

fn history_bytes(dir: &Path) -> Vec<u8> {
    fs::read(dir.join(HISTORY_FILE)).unwrap()
}

#[test]
fn req_025_defaults_are_35_days_and_50_and_45_mebibytes() {
    assert_eq!(HISTORY_KEEP_DAYS, 35);
    assert_eq!(HISTORY_MAX_BYTES, 50 * 1024 * 1024);
    assert_eq!(HISTORY_TARGET_BYTES, 45 * 1024 * 1024);
}

#[test]
fn req_025_prune_removes_older_than_35_days() {
    // 60 days, one record per hour; the newest one is "now".
    let dir = tempfile::tempdir().unwrap();
    fill(dir.path(), 60 * 24, HOUR_MS, NOW_MS);
    let stats = prune_history_default(dir.path(), NOW_MS).unwrap();
    // Records from exactly 35 days before now up to now are kept: 35 * 24 + 1.
    assert_eq!(
        stats,
        PruneStats {
            removed: 1440 - 841,
            kept: 841
        }
    );
    let read = read_history(dir.path()).unwrap();
    assert_eq!(read.records.len(), 841);
    assert_eq!(read.skipped, 0);
    let oldest = read.records.first().unwrap().received_at_ms;
    assert_eq!(oldest, NOW_MS - 35 * DAY_MS, "the record at the edge stays");
    assert_eq!(read.records.last().unwrap().received_at_ms, NOW_MS);
}

#[test]
fn req_025_prune_keeps_recent() {
    let dir = tempfile::tempdir().unwrap();
    fill(dir.path(), 100, HOUR_MS, NOW_MS);
    let before = history_bytes(dir.path());
    let stats = prune_history_default(dir.path(), NOW_MS).unwrap();
    assert_eq!(
        stats,
        PruneStats {
            removed: 0,
            kept: 100
        }
    );
    assert_eq!(
        history_bytes(dir.path()),
        before,
        "nothing to remove: not rewritten"
    );
}

#[test]
fn req_025_prune_enforces_size_limit() {
    let dir = tempfile::tempdir().unwrap();
    fill(dir.path(), 200, 60_000, NOW_MS);
    let size = history_bytes(dir.path()).len() as u64;
    let (max, target) = (size / 2, size / 4);
    let stats = prune_history(dir.path(), NOW_MS, 35, max, target).unwrap();
    let after = history_bytes(dir.path());
    assert!(after.len() as u64 <= target, "{} > {target}", after.len());
    assert!(!after.is_empty());
    assert_eq!(stats.removed + stats.kept, 200);
    // What is left are the newest records, in order, without a gap.
    let read = read_history(dir.path()).unwrap();
    assert_eq!(read.records.len(), stats.kept);
    assert_eq!(read.records.last().unwrap().received_at_ms, NOW_MS);
    let times: Vec<i64> = read.records.iter().map(|r| r.received_at_ms).collect();
    assert!(times.windows(2).all(|w| w[1] - w[0] == 60_000));
}

#[test]
fn req_025_below_the_maximum_nothing_is_cut_for_size() {
    let dir = tempfile::tempdir().unwrap();
    fill(dir.path(), 50, 60_000, NOW_MS);
    let size = history_bytes(dir.path()).len() as u64;
    // Larger than the target, but not larger than the maximum: untouched.
    let stats = prune_history(dir.path(), NOW_MS, 35, size, size / 10).unwrap();
    assert_eq!(
        stats,
        PruneStats {
            removed: 0,
            kept: 50
        }
    );
}

#[test]
fn req_025_exactly_at_the_maximum_is_not_cut() {
    let dir = tempfile::tempdir().unwrap();
    fill(dir.path(), 10, 60_000, NOW_MS);
    let size = history_bytes(dir.path()).len() as u64;
    let stats = prune_history(dir.path(), NOW_MS, 35, size, 0).unwrap();
    assert_eq!(stats.removed, 0);
}

#[test]
fn req_025_age_and_size_limits_work_together() {
    let dir = tempfile::tempdir().unwrap();
    fill(dir.path(), 10 * 24, HOUR_MS, NOW_MS - 40 * DAY_MS); // all too old
    fill(dir.path(), 200, 60_000, NOW_MS);
    let size = history_bytes(dir.path()).len() as u64;
    let stats = prune_history(dir.path(), NOW_MS, 35, size / 4, size / 8).unwrap();
    assert!(stats.removed >= 240);
    assert_eq!(read_history(dir.path()).unwrap().records.len(), stats.kept);
}

#[test]
fn req_025_unreadable_lines_are_dropped() {
    let dir = tempfile::tempdir().unwrap();
    fill(dir.path(), 3, HOUR_MS, NOW_MS);
    let mut text = String::from_utf8(history_bytes(dir.path())).unwrap();
    text.push_str("garbage line\n{\"v\":2,\"received_at_ms\":1}\n\n");
    fs::write(dir.path().join(HISTORY_FILE), text).unwrap();
    let stats = prune_history_default(dir.path(), NOW_MS).unwrap();
    assert_eq!(
        stats,
        PruneStats {
            removed: 2,
            kept: 3
        }
    );
    let read = read_history(dir.path()).unwrap();
    assert_eq!(read.skipped, 0);
    assert_eq!(read.records.len(), 3);
}

#[test]
fn req_025_missing_history_is_nothing_to_do() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        prune_history_default(dir.path(), NOW_MS).unwrap(),
        PruneStats::default()
    );
    assert!(
        !dir.path().join(HISTORY_FILE).exists(),
        "no file is created"
    );
}

#[test]
fn req_025_pruning_leaves_no_temp_file_behind() {
    let dir = tempfile::tempdir().unwrap();
    fill(dir.path(), 5, DAY_MS, NOW_MS);
    prune_history(
        dir.path(),
        NOW_MS,
        1,
        HISTORY_MAX_BYTES,
        HISTORY_TARGET_BYTES,
    )
    .unwrap();
    let leftovers: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty());
}

#[test]
fn req_025_pruning_takes_the_history_lock() {
    let dir = tempfile::tempdir().unwrap();
    fill(dir.path(), 3, HOUR_MS, NOW_MS);
    let held = lock_history(dir.path(), WAIT).unwrap();
    let err = prune_history_with_wait(
        dir.path(),
        NOW_MS,
        35,
        HISTORY_MAX_BYTES,
        HISTORY_TARGET_BYTES,
        Duration::from_millis(60),
    )
    .unwrap_err();
    assert!(matches!(err, StoreError::Lock), "{err}");
    drop(held);
    prune_history_default(dir.path(), NOW_MS).unwrap();
}

/// Appends one record, repeating a failed attempt a limited number of times (see the
/// remark on the concurrent writers test of the data store).
fn append_with_retries(dir: &Path, record: &Record) {
    for _ in 0..200 {
        if append_history(dir, record, Duration::from_secs(5)).is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("append failed after 200 retries");
}

#[test]
fn req_025_appends_during_pruning_are_not_lost() {
    let dir = Arc::new(tempfile::tempdir().unwrap());
    // 100 records that are too old and 100 recent ones to begin with.
    fill(dir.path(), 100, HOUR_MS, NOW_MS - 60 * DAY_MS);
    fill(dir.path(), 100, HOUR_MS, NOW_MS - 10 * DAY_MS);
    let done = Arc::new(AtomicBool::new(false));
    let pruner = {
        let (dir, done) = (Arc::clone(&dir), Arc::clone(&done));
        thread::spawn(move || {
            let mut runs = 0u32;
            while !done.load(Ordering::Relaxed) {
                prune_history_with_wait(
                    dir.path(),
                    NOW_MS,
                    35,
                    HISTORY_MAX_BYTES,
                    HISTORY_TARGET_BYTES,
                    Duration::from_secs(5),
                )
                .expect("pruning failed");
                runs += 1;
                thread::sleep(Duration::from_millis(3));
            }
            runs
        })
    };
    // Every sixth appended record is too old: the pruner has to rewrite the file again and
    // again while the writer appends, which is the situation that could lose a record.
    for i in 0..240 {
        let at = if i % 6 == 5 {
            NOW_MS - 60 * DAY_MS + i
        } else {
            NOW_MS - 5 * DAY_MS + i
        };
        append_with_retries(dir.path(), &record(at));
    }
    done.store(true, Ordering::Relaxed);
    let runs = pruner.join().unwrap();
    eprintln!("pruning ran {runs} times while 240 records were appended");
    // One last run removes the old records that were appended after the last rewrite.
    prune_history_default(dir.path(), NOW_MS).unwrap();
    let read = read_history(dir.path()).unwrap();
    assert_eq!(read.skipped, 0);
    // The 100 old records are gone, the 100 recent and the 200 appended ones are all there.
    assert_eq!(read.records.len(), 300);
    let mut times: Vec<i64> = read.records.iter().map(|r| r.received_at_ms).collect();
    times.sort_unstable();
    times.dedup();
    assert_eq!(times.len(), 300, "a record was lost or written twice");
    assert!(times[0] >= NOW_MS - 35 * DAY_MS);
}

#[test]
fn req_115_pruning_keeps_lines_of_a_newer_format_version() {
    // Lines written by a newer version are not destroyed just because they are not understood:
    // they only go when their receive time is known and too old.
    let dir = tempfile::tempdir().unwrap();
    let recent = NOW_MS - DAY_MS;
    let old = NOW_MS - 60 * DAY_MS;
    let text = format!(
        "{{\"v\":2,\"received_at_ms\":{recent},\"future\":true}}\n\
         {{\"v\":2,\"received_at_ms\":{old}}}\n\
         {{\"v\":2,\"no_time\":1}}\n\
         {{\"v\":1,\"received_at_ms\":{recent}}}\n\
         {{\"received_at_ms\":{recent}}}\n"
    );
    fs::write(dir.path().join(HISTORY_FILE), text).unwrap();
    let stats = prune_history_default(dir.path(), NOW_MS).unwrap();
    // Gone: the old v2 line and the line without a version. Kept: both recent kinds and the
    // v2 line without a time.
    assert_eq!(
        stats,
        PruneStats {
            removed: 2,
            kept: 3
        }
    );
    let left = String::from_utf8(history_bytes(dir.path())).unwrap();
    assert!(left.contains("\"future\":true"));
    assert!(left.contains("\"no_time\":1"));
    assert!(!left.contains(&old.to_string()));
}
