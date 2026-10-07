// SPDX-License-Identifier: Apache-2.0
//! Token statistics from the transcript files of Claude Code (concept §8, REQ-014).
//!
//! The format of the transcripts is not documented, so reading is tolerant: a line that cannot
//! be understood is skipped, a missing count is 0. This module only turns text into numbers;
//! finding the files and reading them incrementally is done elsewhere.

use std::collections::{BTreeMap, HashMap};

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
