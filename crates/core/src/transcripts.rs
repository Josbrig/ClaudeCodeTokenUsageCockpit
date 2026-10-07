// SPDX-License-Identifier: Apache-2.0
//! Token statistics from the transcript files of Claude Code (concept §8, REQ-014).
//!
//! The format of the transcripts is not documented, so reading is tolerant: a line that cannot
//! be understood is skipped, a missing count is 0. This module only turns text into numbers;
//! finding the files and reading them incrementally is done elsewhere.

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use serde_json::Value;

/// Model name used when a line carries none.
pub const UNKNOWN_MODEL: &str = "unknown";

/// Token counts of one message or the sum of several.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    /// Input tokens that were not served from the cache.
    pub input: u64,
    /// Output tokens.
    pub output: u64,
    /// Input tokens written to the cache.
    pub cache_creation: u64,
    /// Input tokens read from the cache.
    pub cache_read: u64,
}

impl Usage {
    /// Adds `other` to `self`; counts saturate instead of overflowing.
    pub fn add(&mut self, other: &Usage) {
        self.input = self.input.saturating_add(other.input);
        self.output = self.output.saturating_add(other.output);
        self.cache_creation = self.cache_creation.saturating_add(other.cache_creation);
        self.cache_read = self.cache_read.saturating_add(other.cache_read);
    }

    /// (cache creation + cache read) ÷ (input + cache creation + cache read), between 0 and 1;
    /// `None` if there was no input at all.
    pub fn cache_share(&self) -> Option<f64> {
        let cached = self.cache_creation as f64 + self.cache_read as f64;
        let all = self.input as f64 + cached;
        (all > 0.0).then(|| cached / all)
    }
}

/// One assistant message with its token counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// When the message was written.
    pub timestamp: DateTime<Utc>,
    /// Model name, [`UNKNOWN_MODEL`] if the line had none.
    pub model: String,
    /// `message.id`, the first half of the duplicate key.
    pub message_id: Option<String>,
    /// `requestId`, the second half of the duplicate key.
    pub request_id: Option<String>,
    /// The token counts.
    pub usage: Usage,
}

impl Entry {
    /// Key under which duplicates are detected; `None` for a message without an id, which is
    /// never merged with another one.
    fn key(&self) -> Option<(&str, &str)> {
        let id = self.message_id.as_deref()?;
        Some((id, self.request_id.as_deref().unwrap_or("")))
    }
}

/// Totals over a set of entries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stats {
    /// Sums per model name.
    pub per_model: BTreeMap<String, Usage>,
    /// Sums per local calendar day.
    pub per_day: BTreeMap<NaiveDate, Usage>,
    /// Number of different messages that were understood. 0 means the format was not
    /// recognised and the statistics are not available.
    pub understood_lines: usize,
}

impl Stats {
    /// The sum over all models.
    pub fn total(&self) -> Usage {
        self.per_model
            .values()
            .fold(Usage::default(), |mut sum, u| {
                sum.add(u);
                sum
            })
    }
}

/// Reads the assistant messages with token counts from the text of a transcript file.
///
/// Only lines with `"type":"assistant"` and a `message.usage` object are used; a line that is
/// not JSON, has no valid `timestamp` or has another shape is skipped. Duplicates (same
/// `message.id` and `requestId`) are merged and the last occurrence wins; the merged entry keeps
/// the position of the first one.
pub fn parse_lines(text: &str) -> Vec<Entry> {
    dedupe(text.lines().filter_map(parse_line).collect())
}

/// Merges entries with the same (`message.id`, `requestId`); the last one wins. Use it on the
/// entries of several files together, because a resumed session can repeat messages.
pub fn dedupe(entries: Vec<Entry>) -> Vec<Entry> {
    let mut kept: Vec<Entry> = Vec::with_capacity(entries.len());
    let mut position: HashMap<(String, String), usize> = HashMap::new();
    for entry in entries {
        let Some((id, request)) = entry.key().map(|(i, r)| (i.to_owned(), r.to_owned())) else {
            kept.push(entry);
            continue;
        };
        match position.get(&(id.clone(), request.clone())) {
            Some(&index) => kept[index] = entry,
            None => {
                position.insert((id, request), kept.len());
                kept.push(entry);
            }
        }
    }
    kept
}

/// Sums the entries per model and per calendar day of the time zone `tz`.
///
/// Duplicates are merged first, so entries of several files can be passed together.
pub fn aggregate<Tz: TimeZone>(entries: &[Entry], tz: &Tz) -> Stats {
    let unique = dedupe(entries.to_vec());
    let mut stats = Stats {
        understood_lines: unique.len(),
        ..Stats::default()
    };
    for entry in &unique {
        stats
            .per_model
            .entry(entry.model.clone())
            .or_default()
            .add(&entry.usage);
        let day = entry.timestamp.with_timezone(tz).date_naive();
        stats.per_day.entry(day).or_default().add(&entry.usage);
    }
    stats
}

fn parse_line(line: &str) -> Option<Entry> {
    let value: Value = serde_json::from_str(line.trim()).ok()?;
    if value.get("type")?.as_str()? != "assistant" {
        return None;
    }
    let message = value.get("message")?;
    let usage = message.get("usage")?.as_object()?;
    let timestamp = DateTime::parse_from_rfc3339(value.get("timestamp")?.as_str()?)
        .ok()?
        .with_timezone(&Utc);
    let count = |name: &str| usage.get(name).and_then(Value::as_u64).unwrap_or(0);
    let text = |source: &Value, name: &str| {
        source
            .get(name)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    Some(Entry {
        timestamp,
        model: text(message, "model").unwrap_or_else(|| UNKNOWN_MODEL.to_owned()),
        message_id: text(message, "id"),
        request_id: text(&value, "requestId"),
        usage: Usage {
            input: count("input_tokens"),
            output: count("output_tokens"),
            cache_creation: count("cache_creation_input_tokens"),
            cache_read: count("cache_read_input_tokens"),
        },
    })
}

/// What the scanner knows about one transcript file.
#[derive(Debug, Default)]
struct FileState {
    /// Bytes up to and including the last complete line that was read.
    offset: u64,
    /// Modification time when the file was last read.
    modified: Option<SystemTime>,
    /// The entries of the file.
    entries: Vec<Entry>,
}

/// Reads the transcript files incrementally (concept §8): a file is read again only from the
/// place where the last scan stopped, and only up to its last complete line.
#[derive(Debug, Default)]
pub struct Scanner {
    files: HashMap<PathBuf, FileState>,
}

impl Scanner {
    /// A scanner that has seen nothing yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Scans `<claude_dir>/projects/**/*.jsonl` for files modified after `since`.
    ///
    /// A file is read from its stored offset to its last complete line, so a line that is still
    /// being written is picked up by the next scan. A file that became shorter than the stored
    /// offset is read again from the start and its old entries are dropped. Files that no longer
    /// exist are forgotten. A missing `projects` folder is not an error; a file or folder that
    /// cannot be read is logged and skipped, only the failure to list the `projects` folder
    /// itself is returned.
    pub fn scan(&mut self, claude_dir: &Path, since: SystemTime) -> Result<(), io::Error> {
        let root = claude_dir.join("projects");
        let mut found = Vec::new();
        match collect_files(&root, since, &mut found, true) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        for (path, modified, len) in found {
            let state = self.files.entry(path.clone()).or_default();
            if state.modified == Some(modified) && state.offset == len {
                continue;
            }
            if let Err(error) = read_new_lines(&path, state, len) {
                log::warn!("transcript {} cannot be read: {error}", path.display());
            } else {
                state.modified = Some(modified);
            }
        }
        self.files.retain(|path, _| path.exists());
        Ok(())
    }

    /// Totals over everything seen so far; a message that appears in several files counts once.
    ///
    /// The files are taken in the order of their paths, so if two files hold the same message
    /// with different counts the one in the later path wins, the same way every time.
    pub fn stats<Tz: TimeZone>(&self, tz: &Tz) -> Stats {
        let mut paths: Vec<&PathBuf> = self.files.keys().collect();
        paths.sort();
        let all: Vec<Entry> = paths
            .into_iter()
            .flat_map(|path| self.files[path].entries.iter().cloned())
            .collect();
        aggregate(&all, tz)
    }

    /// Number of files the scanner keeps track of.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }
}

/// Lists the `.jsonl` files below `dir` that were modified after `since` with their
/// modification time and size. `top` marks the folder whose listing errors are returned.
fn collect_files(
    dir: &Path,
    since: SystemTime,
    found: &mut Vec<(PathBuf, SystemTime, u64)>,
    top: bool,
) -> io::Result<()> {
    let listing = match fs::read_dir(dir) {
        Ok(listing) => listing,
        Err(error) if top => return Err(error),
        Err(error) => {
            log::warn!("folder {} cannot be listed: {error}", dir.display());
            return Ok(());
        }
    };
    for item in listing.flatten() {
        let path = item.path();
        let Ok(kind) = item.file_type() else { continue };
        if kind.is_dir() {
            collect_files(&path, since, found, false)?;
        } else if kind.is_file() && path.extension().is_some_and(|e| e == "jsonl") {
            match item.metadata().and_then(|m| Ok((m.modified()?, m.len()))) {
                Ok((modified, len)) if modified > since => found.push((path, modified, len)),
                Ok(_) => {}
                Err(error) => log::warn!("{} cannot be examined: {error}", path.display()),
            }
        }
    }
    Ok(())
}

/// Reads the complete lines after `state.offset` (or all of them if the file shrank).
fn read_new_lines(path: &Path, state: &mut FileState, len: u64) -> io::Result<()> {
    if len < state.offset {
        *state = FileState::default();
    }
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(state.offset))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    // Only up to the last line break: the rest is a line that is still being written.
    let Some(end) = bytes.iter().rposition(|&b| b == b'\n') else {
        return Ok(());
    };
    let complete = &bytes[..=end];
    state
        .entries
        .extend(parse_lines(&String::from_utf8_lossy(complete)));
    state.offset += complete.len() as u64;
    Ok(())
}
