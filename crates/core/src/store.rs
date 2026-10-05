// SPDX-License-Identifier: Apache-2.0
//! The data directory shared by the bridge and the cockpit (concept §5.2).
//!
//! - `latest.json`: the newest record, replaced atomically (unique temp file, then rename).
//! - `last_error.json`: time of the newest input that could not be parsed.
//! - `history-v1.jsonl`: every record, one JSON object per line, appended under `history.lock`.
//!
//! Every file carries a format version `v`. Readers need no lock: they skip every line they
//! cannot understand, wherever it is, and count it.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::Record;

/// Name of the file with the newest record.
pub const LATEST_FILE: &str = "latest.json";
/// Name of the file with the time of the newest malformed input.
pub const LAST_ERROR_FILE: &str = "last_error.json";
/// Name of the history file (format version 1).
pub const HISTORY_FILE: &str = "history-v1.jsonl";
/// Name of the lock file that guards appending to and pruning of the history.
pub const HISTORY_LOCK_FILE: &str = "history.lock";

const FORMAT_VERSION: u32 = 1;
const RENAME_RETRIES_WINDOWS: u32 = 5;
const RENAME_PAUSE: Duration = Duration::from_millis(20);
const LOCK_POLL: Duration = Duration::from_millis(10);

/// Errors of the data store.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// A file operation failed.
    #[error("file operation failed: {0}")]
    Io(#[from] io::Error),
    /// The history lock could not be taken in time.
    #[error("timed out waiting for the history lock")]
    Lock,
    /// A file exists but cannot be understood (unparsable, or an unknown format version).
    #[error("unreadable data file: {0}")]
    Format(String),
}

// ---- wire formats ---------------------------------------------------------------------

#[derive(Serialize)]
struct LatestOut<'a> {
    v: u32,
    record: &'a Record,
}

#[derive(Deserialize)]
struct LatestV1 {
    record: Record,
}

#[derive(Serialize)]
struct HistoryLineOut<'a> {
    v: u32,
    #[serde(flatten)]
    record: &'a Record,
}

#[derive(Deserialize)]
struct HistoryLineV1 {
    #[serde(flatten)]
    record: Record,
}

#[derive(Serialize, Deserialize)]
struct LastErrorV1 {
    v: u32,
    received_at_ms: i64,
    kind: String,
}

// ---- atomic writing -------------------------------------------------------------------

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_path_for(target: &Path) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    // The counter keeps names unique between threads even if the clock does not tick.
    let count = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}.{nanos}.{count}.tmp", std::process::id()));
    target.with_file_name(name)
}

/// `rename` replaces an existing target. On Windows a reader that has the target open for a
/// moment makes it fail with `PermissionDenied`; that is retried a few times.
fn rename_with_retry(from: &Path, to: &Path) -> io::Result<()> {
    // The first try plus up to five retries on Windows; a single try elsewhere.
    let attempts = if cfg!(windows) {
        1 + RENAME_RETRIES_WINDOWS
    } else {
        1
    };
    let mut attempt = 1;
    loop {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied && attempt < attempts => {
                attempt += 1;
                thread::sleep(RENAME_PAUSE);
            }
            Err(e) => return Err(e),
        }
    }
}

/// Writes `bytes` to a uniquely named temp file next to `target` and renames it onto
/// `target`, so a reader never sees a partial file. The temp file is removed on failure.
pub fn write_atomic(target: &Path, bytes: &[u8]) -> io::Result<()> {
    let temp = temp_path_for(target);
    let result = fs::write(&temp, bytes).and_then(|()| rename_with_retry(&temp, target));
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// Writes `latest.json` atomically, creating the directory if needed.
pub fn write_latest(dir: &Path, record: &Record) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let bytes = serde_json::to_vec(&LatestOut {
        v: FORMAT_VERSION,
        record,
    })
    .map_err(io::Error::other)?;
    write_atomic(&dir.join(LATEST_FILE), &bytes)
}

/// Writes `last_error.json` atomically: a malformed input arrived at `received_at_ms`.
pub fn write_last_error(dir: &Path, received_at_ms: i64) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let bytes = serde_json::to_vec(&LastErrorV1 {
        v: FORMAT_VERSION,
        received_at_ms,
        kind: "malformed".to_owned(),
    })
    .map_err(io::Error::other)?;
    write_atomic(&dir.join(LAST_ERROR_FILE), &bytes)
}

/// Removes leftover `*.tmp` files in `dir` that are at least `older_than` old; returns how
/// many were removed. Meant for the start of the cockpit.
pub fn cleanup_temp_files(dir: &Path, older_than: Duration) -> usize {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let is_temp = path.extension().is_some_and(|e| e == "tmp");
        let old_enough = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|m| m.elapsed().ok())
            .is_some_and(|age| age >= older_than);
        if is_temp && old_enough && fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    removed
}

// ---- reading --------------------------------------------------------------------------

fn read_bytes_if_exists(path: &Path) -> Result<Option<Vec<u8>>, StoreError> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(StoreError::Io(e)),
    }
}

fn format_version(value: &Value) -> Option<u64> {
    value.get("v").and_then(Value::as_u64)
}

/// Reads `latest.json`. A missing file is `Ok(None)`; an unparsable file or an unknown format
/// version is a [`StoreError::Format`].
pub fn read_latest(dir: &Path) -> Result<Option<Record>, StoreError> {
    let Some(bytes) = read_bytes_if_exists(&dir.join(LATEST_FILE))? else {
        return Ok(None);
    };
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|e| StoreError::Format(format!("{LATEST_FILE}: {e}")))?;
    match format_version(&value) {
        Some(1) => {
            let latest: LatestV1 = serde_json::from_value(value)
                .map_err(|e| StoreError::Format(format!("{LATEST_FILE}: {e}")))?;
            Ok(Some(latest.record))
        }
        other => Err(StoreError::Format(format!(
            "{LATEST_FILE}: unknown format version {other:?}"
        ))),
    }
}

/// Reads the time of the newest malformed input from `last_error.json`; a missing or
/// unreadable file is `None`.
pub fn read_last_error(dir: &Path) -> Option<i64> {
    let bytes = fs::read(dir.join(LAST_ERROR_FILE)).ok()?;
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    match format_version(&value)? {
        1 => serde_json::from_value::<LastErrorV1>(value)
            .ok()
            .map(|e| e.received_at_ms),
        _ => None,
    }
}

/// Result of reading the history.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HistoryRead {
    /// All records that could be understood, in file order.
    pub records: Vec<Record>,
    /// Number of lines that were skipped (unparsable, partial, or of an unknown version).
    pub skipped: usize,
}

/// One history line; `None` if it cannot be understood. Dispatches on the format version so a
/// later version can be added as another arm.
fn parse_history_line(line: &str) -> Option<Record> {
    let value: Value = serde_json::from_str(line).ok()?;
    match format_version(&value)? {
        1 => serde_json::from_value::<HistoryLineV1>(value)
            .ok()
            .map(|l| l.record),
        _ => None,
    }
}

/// Reads the whole history. A missing file is an empty history.
pub fn read_history(dir: &Path) -> Result<HistoryRead, StoreError> {
    let Some(bytes) = read_bytes_if_exists(&dir.join(HISTORY_FILE))? else {
        return Ok(HistoryRead::default());
    };
    let text = String::from_utf8_lossy(&bytes);
    let mut result = HistoryRead::default();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        match parse_history_line(line) {
            Some(record) => result.records.push(record),
            None => result.skipped += 1,
        }
    }
    if result.skipped > 0 {
        log::warn!("history: skipped {} unreadable line(s)", result.skipped);
    }
    Ok(result)
}

// ---- appending ------------------------------------------------------------------------

/// Holds `history.lock` until dropped.
#[derive(Debug)]
pub struct HistoryLock {
    file: File,
}

impl Drop for HistoryLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

/// Takes `history.lock`, polling every 10 ms until `timeout` has passed.
pub fn lock_history(dir: &Path, timeout: Duration) -> Result<HistoryLock, StoreError> {
    fs::create_dir_all(dir)?;
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(dir.join(HISTORY_LOCK_FILE))?;
    let deadline = Instant::now() + timeout;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(HistoryLock { file }),
            Err(fs::TryLockError::WouldBlock) => {
                if Instant::now() >= deadline {
                    return Err(StoreError::Lock);
                }
                thread::sleep(LOCK_POLL);
            }
            Err(fs::TryLockError::Error(e)) => return Err(StoreError::Io(e)),
        }
    }
}

/// Appends one record to the history under `history.lock`. If the file does not end with a
/// newline (a writer was killed half-way), a newline is written first so the new line starts
/// cleanly.
pub fn append_history(
    dir: &Path,
    record: &Record,
    lock_timeout: Duration,
) -> Result<(), StoreError> {
    let _lock = lock_history(dir, lock_timeout)?;
    let mut file = OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .open(dir.join(HISTORY_FILE))?;
    let mut out = Vec::new();
    if file.metadata()?.len() > 0 {
        file.seek(SeekFrom::End(-1))?;
        let mut last = [0u8; 1];
        file.read_exact(&mut last)?;
        if last[0] != b'\n' {
            out.push(b'\n');
        }
    }
    serde_json::to_writer(
        &mut out,
        &HistoryLineOut {
            v: FORMAT_VERSION,
            record,
        },
    )
    .map_err(io::Error::other)?;
    out.push(b'\n');
    file.write_all(&out)?;
    Ok(())
}
