// SPDX-License-Identifier: Apache-2.0
//! What the window knows and how it learns more (concept §11.6): the history loaded at start,
//! new records picked up by a poller thread, transcript statistics from a scanner thread.
//!
//! Threads are plain `std` threads that send over channels and wake the window with a callback;
//! there is no async runtime. The window drains the channels at the start of each frame.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime};

use chrono::{Local, Utc};
use cockpit_core::model::Record;
use cockpit_core::settings::Settings;
use cockpit_core::store;
use cockpit_core::transcripts::{Scanner, Stats};
use cockpit_core::viewmodel::{Inputs, ViewModel, build};

/// How often `latest.json` is looked at.
pub const POLL_EVERY: Duration = Duration::from_millis(500);
/// How often the transcript files are scanned.
pub const SCAN_EVERY: Duration = Duration::from_secs(60);
/// Temp files older than this are removed at start.
pub const TEMP_FILE_AGE: Duration = Duration::from_secs(60);
/// How many of the newest records are compared to avoid taking one record in twice.
const RECENT_FOR_DUPLICATES: usize = 5;

/// Something the poller noticed.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A record that differs from the last one seen.
    Record(Record),
    /// The time (Unix milliseconds) of the newest malformed input changed.
    LastError(i64),
}

/// Looks at `latest.json` and `last_error.json` and reports changes.
#[derive(Debug)]
pub struct Poller {
    dir: PathBuf,
    last_record: Option<Record>,
    last_error_ms: Option<i64>,
}

impl Poller {
    /// A poller that treats what is in the folder now as already known.
    pub fn new(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
            last_record: store::read_latest(dir).ok().flatten(),
            last_error_ms: store::read_last_error(dir),
        }
    }

    /// Reads both files and returns what changed since the last call.
    ///
    /// The small file is read on every call and compared by content, so a change is found even
    /// if the file system's time stamp did not move. A file that cannot be parsed (for example
    /// while it is replaced) counts as unchanged.
    pub fn poll(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        match store::read_latest(&self.dir) {
            Ok(Some(record)) if self.last_record.as_ref() != Some(&record) => {
                self.last_record = Some(record.clone());
                events.push(Event::Record(record));
            }
            Ok(_) => {}
            Err(error) => log::debug!("latest.json cannot be read: {error}"),
        }
        let error_ms = store::read_last_error(&self.dir);
        if let Some(ms) = error_ms.filter(|ms| self.last_error_ms != Some(*ms)) {
            events.push(Event::LastError(ms));
        }
        self.last_error_ms = error_ms.or(self.last_error_ms);
        events
    }
}

/// Runs a [`Poller`] every [`POLL_EVERY`] on its own thread. `on_new` is called after events were
/// sent. The thread ends when the receiver is gone.
pub fn spawn_poller(
    dir: &Path,
    events: Sender<Event>,
    on_new: impl Fn() + Send + 'static,
) -> JoinHandle<()> {
    let mut poller = Poller::new(dir);
    thread::spawn(move || {
        loop {
            thread::sleep(POLL_EVERY);
            let found = poller.poll();
            if found.is_empty() {
                continue;
            }
            for event in found {
                if events.send(event).is_err() {
                    return;
                }
            }
            on_new();
        }
    })
}

/// Scans the transcripts at once and then every [`SCAN_EVERY`]; sends the totals each time.
pub fn spawn_scanner(
    claude_dir: PathBuf,
    stats: Sender<Stats>,
    on_new: impl Fn() + Send + 'static,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut scanner = Scanner::new();
        loop {
            let keep = Duration::from_secs(86_400 * u64::from(store::HISTORY_KEEP_DAYS));
            let since = SystemTime::now()
                .checked_sub(keep)
                .unwrap_or(SystemTime::UNIX_EPOCH);
            if let Err(error) = scanner.scan(&claude_dir, since) {
                log::warn!("transcripts cannot be scanned: {error}");
            }
            if stats.send(scanner.stats(&Local)).is_err() {
                return;
            }
            on_new();
            thread::sleep(SCAN_EVERY);
        }
    })
}

/// The data of the window: records and the time of the last malformed input.
#[derive(Debug, Default)]
pub struct Model {
    /// The history in received order, plus the records that arrived since the start.
    pub records: Vec<Record>,
    /// Time of the newest malformed input.
    pub last_error_ms: Option<i64>,
}

impl Model {
    /// Takes in an event; returns whether the data changed. A record that is already among the
    /// newest ones (it can be both in the history and in `latest.json`) is not added twice.
    pub fn apply(&mut self, event: Event) -> bool {
        match event {
            Event::Record(record) => {
                let known = self
                    .records
                    .iter()
                    .rev()
                    .take(RECENT_FOR_DUPLICATES)
                    .any(|r| *r == record);
                if known {
                    return false;
                }
                self.records.push(record);
                true
            }
            Event::LastError(ms) => {
                let changed = self.last_error_ms != Some(ms);
                self.last_error_ms = Some(ms);
                changed
            }
        }
    }
}

/// Everything the window needs: the data, the settings, the channels and the cached view model.
pub struct AppState {
    settings: Settings,
    model: Model,
    stats: Option<Stats>,
    load_error: Option<String>,
    events: Receiver<Event>,
    stats_in: Receiver<Stats>,
    /// Counts changes of the data; part of the key of the cached view model.
    version: u64,
    cache: Option<(i64, u64, ViewModel)>,
}

impl AppState {
    /// Loads the history once, removes old temp files and starts the two threads. `wake` is
    /// called from those threads when there is something new to draw.
    pub fn new(
        data_dir: &Path,
        settings: Settings,
        claude_dir: Option<PathBuf>,
        wake: impl Fn() + Clone + Send + 'static,
    ) -> Self {
        let removed = store::cleanup_temp_files(data_dir, TEMP_FILE_AGE);
        if removed > 0 {
            log::info!("removed {removed} old temp file(s) from the data folder");
        }
        // The poller starts first, so a record that arrives while the history is read is
        // reported as new and de-duplicated by `Model::apply`.
        let (event_tx, events) = mpsc::channel();
        spawn_poller(data_dir, event_tx, wake.clone());
        let (stats_tx, stats_in) = mpsc::channel();
        if let Some(dir) = claude_dir {
            spawn_scanner(dir, stats_tx, wake);
        }
        let mut model = Model::default();
        let mut load_error = None;
        match store::read_history(data_dir) {
            Ok(history) => model.records = history.records,
            Err(error) => {
                log::error!("the history cannot be read: {error}");
                load_error = Some(data_dir.display().to_string());
            }
        }
        model.last_error_ms = store::read_last_error(data_dir);
        if let Ok(Some(latest)) = store::read_latest(data_dir) {
            model.apply(Event::Record(latest));
        }
        if !model.records.is_empty() {
            load_error = None;
        }
        Self {
            settings,
            model,
            stats: None,
            load_error,
            events,
            stats_in,
            version: 0,
            cache: None,
        }
    }

    /// Takes in everything the threads sent since the last frame; returns whether anything
    /// changed.
    pub fn drain(&mut self) -> bool {
        let mut changed = false;
        while let Ok(event) = self.events.try_recv() {
            changed |= self.model.apply(event);
        }
        // Only the newest statistics matter.
        while let Ok(stats) = self.stats_in.try_recv() {
            self.stats = Some(stats);
            changed = true;
        }
        if changed {
            // Data is arriving after all: a failed read of the history no longer says the data
            // folder is unreadable.
            if !self.model.records.is_empty() {
                self.load_error = None;
            }
            self.version += 1;
        }
        changed
    }

    /// The view model for `now_ms`; rebuilt when the data or the second changed.
    pub fn view_model(&mut self, now_ms: i64) -> &ViewModel {
        let second = now_ms.div_euclid(1000);
        let fresh = matches!(&self.cache, Some((s, v, _)) if *s == second && *v == self.version);
        if !fresh {
            let view = build(&Inputs {
                now_ms,
                records: &self.model.records,
                last_error_ms: self.model.last_error_ms,
                load_error: self.load_error.as_deref(),
                settings: &self.settings,
                stats: self.stats.as_ref(),
                tz: &Local,
            });
            self.cache = Some((second, self.version, view));
        }
        match &self.cache {
            Some((_, _, view)) => view,
            None => unreachable!("the cache was filled above"),
        }
    }

    /// The view model for the current time.
    pub fn view_model_now(&mut self) -> &ViewModel {
        self.view_model(Utc::now().timestamp_millis())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    use cockpit_core::model::WindowSample;

    use super::*;

    fn record(received_ms: i64, used: f64) -> Record {
        Record {
            received_at_ms: received_ms,
            session_id: None,
            cc_version: None,
            five_hour: Some(WindowSample {
                used_pct: used,
                resets_at: 1_738_425_600,
            }),
            seven_day: None,
            model: None,
            context_used_pct: None,
            cost_usd: None,
        }
    }

    #[test]
    fn req_010_poll_reports_a_new_record_once() {
        let dir = tempfile::tempdir().unwrap();
        let mut poller = Poller::new(dir.path());
        assert!(poller.poll().is_empty());
        store::write_latest(dir.path(), &record(1_000, 10.0)).unwrap();
        assert_eq!(poller.poll(), [Event::Record(record(1_000, 10.0))]);
        assert!(
            poller.poll().is_empty(),
            "the same content is not new again"
        );
        // Same content written again (a new time stamp): still nothing new.
        store::write_latest(dir.path(), &record(1_000, 10.0)).unwrap();
        assert!(poller.poll().is_empty());
        store::write_latest(dir.path(), &record(2_000, 12.0)).unwrap();
        assert_eq!(poller.poll(), [Event::Record(record(2_000, 12.0))]);
    }

    #[test]
    fn req_010_poller_starts_with_what_is_already_there() {
        let dir = tempfile::tempdir().unwrap();
        store::write_latest(dir.path(), &record(1_000, 10.0)).unwrap();
        store::write_last_error(dir.path(), 500).unwrap();
        let mut poller = Poller::new(dir.path());
        assert!(poller.poll().is_empty());
    }

    #[test]
    fn req_108_poll_reports_a_new_malformed_input() {
        let dir = tempfile::tempdir().unwrap();
        let mut poller = Poller::new(dir.path());
        store::write_last_error(dir.path(), 5_000).unwrap();
        assert_eq!(poller.poll(), [Event::LastError(5_000)]);
        assert!(poller.poll().is_empty());
        store::write_last_error(dir.path(), 6_000).unwrap();
        assert_eq!(poller.poll(), [Event::LastError(6_000)]);
    }

    #[test]
    fn req_010_poll_ignores_a_file_that_cannot_be_parsed() {
        let dir = tempfile::tempdir().unwrap();
        let mut poller = Poller::new(dir.path());
        std::fs::write(dir.path().join(store::LATEST_FILE), b"{ half a file").unwrap();
        assert!(poller.poll().is_empty());
        store::write_latest(dir.path(), &record(1_000, 10.0)).unwrap();
        assert_eq!(poller.poll().len(), 1);
    }

    #[test]
    fn req_010_poller_detects_new_record_within_two_seconds() {
        for trial in 0..10_u64 {
            let dir = tempfile::tempdir().unwrap();
            let (tx, rx) = mpsc::channel();
            let woken = Arc::new(AtomicUsize::new(0));
            let counter = Arc::clone(&woken);
            spawn_poller(dir.path(), tx, move || {
                counter.fetch_add(1, Ordering::SeqCst);
            });
            // Start somewhere inside the poll interval, so the delay is not always the same.
            thread::sleep(Duration::from_millis(37 * trial));
            let started = Instant::now();
            let sent = record(1_000 + trial as i64, 10.0);
            store::write_latest(dir.path(), &sent).unwrap();
            let event = rx
                .recv_timeout(Duration::from_secs(2))
                .unwrap_or_else(|_| panic!("trial {trial}: nothing within 2 s"));
            assert_eq!(event, Event::Record(sent));
            assert!(started.elapsed() < Duration::from_secs(2));
            // The wake-up call follows right after the event was sent.
            let deadline = Instant::now() + Duration::from_secs(1);
            while woken.load(Ordering::SeqCst) == 0 {
                assert!(Instant::now() < deadline, "the window must be woken");
                thread::sleep(Duration::from_millis(5));
            }
        }
    }

    #[test]
    fn req_013_model_does_not_add_a_record_twice() {
        let mut model = Model {
            records: vec![record(1_000, 10.0), record(2_000, 20.0)],
            last_error_ms: None,
        };
        assert!(!model.apply(Event::Record(record(2_000, 20.0))));
        assert!(!model.apply(Event::Record(record(1_000, 10.0))));
        assert!(model.apply(Event::Record(record(3_000, 30.0))));
        assert_eq!(model.records.len(), 3);
        assert!(model.apply(Event::LastError(4_000)));
        assert!(!model.apply(Event::LastError(4_000)));
        assert_eq!(model.last_error_ms, Some(4_000));
    }

    #[test]
    fn req_013_app_state_loads_the_history_and_picks_up_new_records() {
        let dir = tempfile::tempdir().unwrap();
        let old = record(1_738_400_000_000, 10.0);
        store::append_history(dir.path(), &old, Duration::from_secs(1)).unwrap();
        store::write_latest(dir.path(), &old).unwrap();
        let mut state = AppState::new(dir.path(), Settings::default(), None, || {});
        assert_eq!(
            state.model.records,
            std::slice::from_ref(&old),
            "history and latest are one record"
        );
        assert!(!state.drain());

        let newer = record(1_738_400_060_000, 20.0);
        store::write_latest(dir.path(), &newer).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !state.drain() {
            assert!(Instant::now() < deadline, "the new record did not arrive");
            thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(state.model.records, [old, newer]);
    }

    #[test]
    fn req_016_view_model_is_cached_per_second_and_rebuilt_on_new_data() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = AppState::new(dir.path(), Settings::default(), None, || {});
        let now = 1_738_400_000_000;
        assert!(state.view_model(now).banner.is_some(), "no record yet");
        let key = |s: &AppState| s.cache.as_ref().map(|(sec, ver, _)| (*sec, *ver));
        let first = key(&state);
        state.view_model(now + 500);
        assert_eq!(key(&state), first, "same second, nothing rebuilt");
        state.view_model(now + 1_000);
        assert_ne!(key(&state), first, "the next second rebuilds");
    }

    #[test]
    fn req_013_an_unreadable_history_is_reported_as_a_load_error() {
        let dir = tempfile::tempdir().unwrap();
        // A folder where the history file should be makes reading fail.
        std::fs::create_dir(dir.path().join(store::HISTORY_FILE)).unwrap();
        let mut state = AppState::new(dir.path(), Settings::default(), None, || {});
        let banner = state.view_model(1_738_400_000_000).banner.clone().unwrap();
        assert!(
            banner.starts_with("Cannot read the data folder: "),
            "{banner}"
        );
        assert!(banner.contains(&dir.path().display().to_string()));
    }

    #[test]
    fn req_013_the_load_error_goes_away_once_records_arrive() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(store::HISTORY_FILE)).unwrap();
        let mut state = AppState::new(dir.path(), Settings::default(), None, || {});
        assert!(state.view_model(1_738_400_000_000).banner.is_some());
        store::write_latest(dir.path(), &record(1_738_400_000_000, 10.0)).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !state.drain() {
            assert!(Instant::now() < deadline, "the record did not arrive");
            thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(state.view_model(1_738_400_001_000).banner, None);
    }
}
