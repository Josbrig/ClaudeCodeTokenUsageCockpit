// SPDX-License-Identifier: Apache-2.0
//! Small rotating file logger (concept §10).
//!
//! One line per message: `<UTC RFC 3339 seconds> <LEVEL> <component>: <message>`. When the
//! next line would push `log.txt` over the size limit, `log.txt` becomes `log.1.txt`
//! (replacing an older one) and a new `log.txt` starts. Rotation happens only while holding
//! `log.lock`, so the bridge and the cockpit never rotate at the same time. Logging never
//! panics and never reports a failure to its caller.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use log::{Level, LevelFilter, Log, Metadata, Record};

/// Default size limit of `log.txt`: 5 MB.
pub const DEFAULT_MAX_BYTES: u64 = 5 * 1024 * 1024;

const LOG_FILE: &str = "log.txt";
const OLD_LOG_FILE: &str = "log.1.txt";
const LOCK_FILE: &str = "log.lock";

/// Errors of [`init`].
#[derive(Debug, thiserror::Error)]
pub enum LogError {
    /// The log directory could not be created.
    #[error("cannot create the log directory: {0}")]
    Io(#[from] std::io::Error),
    /// A logger is already installed in this process.
    #[error("a logger is already installed")]
    AlreadyInitialised,
}

/// A logger that writes to `log.txt` in a directory.
#[derive(Debug)]
pub struct FileLogger {
    dir: PathBuf,
    component: String,
    max_bytes: u64,
    guard: Mutex<()>,
}

impl FileLogger {
    /// Creates a logger for `component` writing into `dir`. Nothing is created until the
    /// first line is written.
    pub fn new(dir: &Path, component: &str, max_bytes: u64) -> Self {
        Self {
            dir: dir.to_path_buf(),
            component: component.to_owned(),
            max_bytes,
            guard: Mutex::new(()),
        }
    }

    /// Writes one line stamped with the current time.
    pub fn write(&self, level: Level, message: &str) {
        self.write_at(Utc::now(), level, message);
    }

    /// Writes one line stamped with the given time. Failures are ignored.
    pub fn write_at(&self, time: DateTime<Utc>, level: Level, message: &str) {
        let _serial = self.guard.lock().unwrap_or_else(|e| e.into_inner());
        let line = format!(
            "{} {} {}: {}\n",
            time.format("%Y-%m-%dT%H:%M:%SZ"),
            level,
            self.component,
            message.replace(['\r', '\n'], " ")
        );
        let _ = self.append(&line);
    }

    fn append(&self, line: &str) -> std::io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let path = self.dir.join(LOG_FILE);
        let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        if size > 0 && size + line.len() as u64 > self.max_bytes {
            self.rotate(&path, line.len() as u64);
        }
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        file.write_all(line.as_bytes())
    }

    /// Renames `log.txt` to `log.1.txt` while holding `log.lock`; skips if another process
    /// holds the lock or another process already rotated (the next line fits again).
    fn rotate(&self, path: &Path, next_line_len: u64) {
        let Ok(lock) = File::create(self.dir.join(LOCK_FILE)) else {
            return;
        };
        if lock.try_lock().is_err() {
            return;
        }
        let still_full = fs::metadata(path)
            .map(|m| m.len() > 0 && m.len() + next_line_len > self.max_bytes)
            .unwrap_or(false);
        if still_full {
            let old = self.dir.join(OLD_LOG_FILE);
            let _ = fs::remove_file(&old);
            let _ = fs::rename(path, &old);
        }
        let _ = lock.unlock();
    }
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            self.write(record.level(), &record.args().to_string());
        }
    }

    fn flush(&self) {}
}

/// Installs a [`FileLogger`] as the process-wide logger (level `Info` and above).
pub fn init(dir: &Path, component: &str, max_bytes: u64) -> Result<(), LogError> {
    fs::create_dir_all(dir)?;
    let logger = FileLogger::new(dir, component, max_bytes);
    log::set_boxed_logger(Box::new(logger)).map_err(|_| LogError::AlreadyInitialised)?;
    log::set_max_level(LevelFilter::Info);
    Ok(())
}
