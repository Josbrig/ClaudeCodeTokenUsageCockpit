// SPDX-License-Identifier: Apache-2.0
//! Bridge mode (concept §5.3): Claude Code starts `usage-cockpit bridge` as its status line
//! command and hands one JSON record on standard input.
//!
//! The bridge stores the record for the cockpit, prints a short status text for Claude Code to
//! show, and exits with 0 in every case: it must never fail visibly. All errors go to the log.
//! It touches no GUI code.

use std::io::{Read, Write};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cockpit_core::model::{Record, WindowSample};
use cockpit_core::parse::parse_status_line;
use cockpit_core::store;

use crate::shell;

/// Input larger than this is treated like malformed input.
pub const MAX_INPUT_BYTES: u64 = 1024 * 1024;
/// How long the bridge waits for the history lock before it skips the history line.
pub const HISTORY_LOCK_WAIT: Duration = Duration::from_millis(200);
/// How long the kept user status line command may run.
pub const KEPT_COMMAND_TIMEOUT: Duration = Duration::from_secs(1);
/// Printed when the input could not be used.
pub const NO_DATA_TEXT: &str = "usage-cockpit: no data";
/// File in the configuration directory that holds the status line replaced by the bridge.
pub const BRIDGE_STATE_FILE: &str = "bridge-state.json";

/// Runs the bridge once and returns the exit code, which is always 0.
///
/// The record is stored first. Then the kept user command, if there is one, answers; if it does
/// not succeed, the bridge prints its own text.
pub fn run(stdin: impl Read, mut stdout: impl Write, data_dir: &Path, config_dir: &Path) -> i32 {
    let input = read_input(stdin);
    let text = handle(input.as_deref(), now_ms(), data_dir);
    let kept = kept_command(config_dir).and_then(|command| {
        shell::run_kept_command(
            &command,
            input.as_deref().unwrap_or_default(),
            KEPT_COMMAND_TIMEOUT,
        )
    });
    // A closed or broken output must not change the result.
    let _ = match kept {
        Some(output) => stdout.write_all(&output),
        None => writeln!(stdout, "{text}"),
    };
    let _ = stdout.flush();
    0
}

/// The previous status line command from `bridge-state.json`, if there is a usable one: the file
/// parses, `previous_status_line` is an object of type `command` with a non-blank `command`.
fn kept_command(config_dir: &Path) -> Option<String> {
    let bytes = match std::fs::read(config_dir.join(BRIDGE_STATE_FILE)) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            log::warn!("bridge: cannot read {BRIDGE_STATE_FILE}: {error}");
            return None;
        }
    };
    let Ok(state) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        log::warn!("bridge: {BRIDGE_STATE_FILE} cannot be parsed");
        return None;
    };
    let previous = state.get("previous_status_line")?;
    if previous.get("type")?.as_str()? != "command" {
        return None;
    }
    let command = previous.get("command")?.as_str()?;
    (!command.trim().is_empty()).then(|| command.to_owned())
}

/// Reads at most [`MAX_INPUT_BYTES`]; `None` if the input is larger or cannot be read.
fn read_input(stdin: impl Read) -> Option<Vec<u8>> {
    let mut buffer = Vec::new();
    stdin
        .take(MAX_INPUT_BYTES + 1)
        .read_to_end(&mut buffer)
        .ok()?;
    (buffer.len() as u64 <= MAX_INPUT_BYTES).then_some(buffer)
}

fn handle(input: Option<&[u8]>, received_at_ms: i64, data_dir: &Path) -> String {
    let record = input
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .and_then(|text| parse_status_line(text, received_at_ms).ok());
    match record {
        Some(record) => {
            store_record(&record, data_dir);
            status_text(&record)
        }
        None => {
            log::warn!(
                "bridge: input could not be used (not a JSON object, not UTF-8 or too large)"
            );
            if let Err(error) = store::write_last_error(data_dir, received_at_ms) {
                log::warn!("bridge: cannot write {}: {error}", store::LAST_ERROR_FILE);
            }
            NO_DATA_TEXT.to_owned()
        }
    }
}

/// Writes `latest.json` and appends the history line; every failure is only logged.
fn store_record(record: &Record, data_dir: &Path) {
    log_version_change(record, data_dir);
    if let Err(error) = store::write_latest(data_dir, record) {
        log::warn!("bridge: cannot write {}: {error}", store::LATEST_FILE);
    }
    if let Err(error) = store::append_history(data_dir, record, HISTORY_LOCK_WAIT) {
        log::warn!("bridge: history line skipped: {error}");
    }
}

/// Logs one line when the Claude Code version differs from the one of the previous record.
fn log_version_change(record: &Record, data_dir: &Path) {
    let Ok(Some(previous)) = store::read_latest(data_dir) else {
        return;
    };
    if let (Some(old), Some(new)) = (&previous.cc_version, &record.cc_version)
        && old != new
    {
        log::info!("bridge: Claude Code version changed from {old} to {new}");
    }
}

/// `5h 23.5% · 7d 41.2%`, with `–` for a window that was not delivered. Values above 100 show
/// as 100.
fn status_text(record: &Record) -> String {
    format!(
        "5h {} · 7d {}",
        window_text(record.five_hour.as_ref()),
        window_text(record.seven_day.as_ref())
    )
}

fn window_text(window: Option<&WindowSample>) -> String {
    match window {
        Some(w) => format!("{:.1}%", w.used_pct.clamp(0.0, 100.0)),
        None => "–".to_owned(),
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;

    use cockpit_core::store::{StoreError, read_history, read_latest};

    use super::*;

    const BOTH: &str = r#"{"session_id":"s","version":"2.1.90","rate_limits":{
        "five_hour":{"used_percentage":23.5,"resets_at":1738425600},
        "seven_day":{"used_percentage":41.2,"resets_at":1738857600}}}"#;

    fn run_with(input: &str, dir: &Path) -> (i32, String) {
        let mut out = Vec::new();
        let code = run(input.as_bytes(), &mut out, dir, dir);
        (code, String::from_utf8(out).unwrap())
    }

    fn state_dir(content: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(BRIDGE_STATE_FILE), content).unwrap();
        dir
    }

    #[test]
    fn req_012_kept_command_is_read_from_the_state_file() {
        let usable = |json: &str| kept_command(state_dir(json).path());
        let command =
            r#"{"v":1,"previous_status_line":{"type":"command","command":"ccusage","padding":2}}"#;
        assert_eq!(usable(command).as_deref(), Some("ccusage"));
    }

    #[test]
    fn req_012_no_kept_command_for_unusable_state() {
        let empty = tempfile::tempdir().unwrap();
        assert_eq!(kept_command(empty.path()), None, "file missing");
        for json in [
            "",
            "{ nope",
            "[]",
            r#"{"v":1}"#,
            r#"{"v":1,"previous_status_line":null}"#,
            r#"{"v":1,"previous_status_line":"ccusage"}"#,
            r#"{"v":1,"previous_status_line":{"type":"text","command":"x"}}"#,
            r#"{"v":1,"previous_status_line":{"command":"x"}}"#,
            r#"{"v":1,"previous_status_line":{"type":"command"}}"#,
            r#"{"v":1,"previous_status_line":{"type":"command","command":""}}"#,
            r#"{"v":1,"previous_status_line":{"type":"command","command":"  "}}"#,
            r#"{"v":1,"previous_status_line":{"type":"command","command":5}}"#,
        ] {
            assert_eq!(kept_command(state_dir(json).path()), None, "{json:?}");
        }
    }

    #[test]
    fn req_020_bridge_stores_record() {
        let dir = tempfile::tempdir().unwrap();
        let (code, _) = run_with(BOTH, dir.path());
        assert_eq!(code, 0);
        let latest = read_latest(dir.path()).unwrap().unwrap();
        assert_eq!(latest.five_hour.unwrap().used_pct, 23.5);
        assert_eq!(read_history(dir.path()).unwrap().records.len(), 1);
    }

    #[test]
    fn req_020_bridge_prints_usage_text() {
        let dir = tempfile::tempdir().unwrap();
        let (_, text) = run_with(BOTH, dir.path());
        assert_eq!(text, "5h 23.5% · 7d 41.2%\n");
    }

    #[test]
    fn req_020_bridge_shows_a_dash_for_a_missing_window() {
        let dir = tempfile::tempdir().unwrap();
        let one = r#"{"rate_limits":{"five_hour":{"used_percentage":12,"resets_at":1738425600}}}"#;
        assert_eq!(run_with(one, dir.path()).1, "5h 12.0% · 7d –\n");
        assert_eq!(run_with("{}", dir.path()).1, "5h – · 7d –\n");
    }

    #[test]
    fn req_020_bridge_shows_values_above_100_as_100() {
        let dir = tempfile::tempdir().unwrap();
        let over =
            r#"{"rate_limits":{"seven_day":{"used_percentage":120.5,"resets_at":1738857600}}}"#;
        assert_eq!(run_with(over, dir.path()).1, "5h – · 7d 100.0%\n");
    }

    #[test]
    fn req_109_bridge_exit_zero_on_malformed_input() {
        let dir = tempfile::tempdir().unwrap();
        for input in ["{ nope", "[]", "", "null"] {
            let (code, text) = run_with(input, dir.path());
            assert_eq!(code, 0, "{input:?}");
            assert_eq!(text, "usage-cockpit: no data\n");
        }
        assert!(store::read_last_error(dir.path()).is_some());
        assert_eq!(read_latest(dir.path()).unwrap(), None, "nothing stored");
        assert!(read_history(dir.path()).unwrap().records.is_empty());
    }

    #[test]
    fn req_109_bridge_treats_input_that_is_not_utf8_as_malformed() {
        let dir = tempfile::tempdir().unwrap();
        let mut out = Vec::new();
        let code = run(&[0xff, 0xfe, 0x7b][..], &mut out, dir.path(), dir.path());
        assert_eq!(code, 0);
        assert_eq!(String::from_utf8(out).unwrap(), "usage-cockpit: no data\n");
        assert!(store::read_last_error(dir.path()).is_some());
    }

    #[test]
    fn req_109_bridge_ignores_input_over_one_mebibyte() {
        let dir = tempfile::tempdir().unwrap();
        let big = format!(r#"{{"padding":"{}"}}"#, "x".repeat(1024 * 1024));
        let (code, text) = run_with(&big, dir.path());
        assert_eq!(code, 0);
        assert_eq!(text, "usage-cockpit: no data\n");
        assert_eq!(read_latest(dir.path()).unwrap(), None);
    }

    #[test]
    fn req_109_bridge_accepts_input_of_exactly_one_mebibyte() {
        let dir = tempfile::tempdir().unwrap();
        let prefix = r#"{"padding":""#;
        let suffix = r#""}"#;
        let fill = MAX_INPUT_BYTES as usize - prefix.len() - suffix.len();
        let exact = format!("{prefix}{}{suffix}", "x".repeat(fill));
        assert_eq!(exact.len() as u64, MAX_INPUT_BYTES);
        let (code, text) = run_with(&exact, dir.path());
        assert_eq!(code, 0);
        assert_eq!(text, "5h – · 7d –\n", "exactly the limit is still accepted");
        assert!(read_latest(dir.path()).unwrap().is_some());
    }

    #[test]
    fn req_109_bridge_exit_zero_on_unwritable_dir() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a-file");
        std::fs::write(&file, b"x").unwrap();
        // The data directory would have to lie below a regular file.
        let unusable = file.join("data");
        let (code, text) = run_with(BOTH, &unusable);
        assert_eq!(code, 0);
        assert_eq!(text, "5h 23.5% · 7d 41.2%\n", "the text is printed anyway");
        let (code, text) = run_with("{ nope", &unusable);
        assert_eq!(code, 0);
        assert_eq!(text, "usage-cockpit: no data\n");
    }

    #[test]
    fn req_109_bridge_exits_zero_when_the_output_is_closed() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Err(std::io::ErrorKind::BrokenPipe.into())
            }
        }
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(run(BOTH.as_bytes(), Broken, dir.path(), dir.path()), 0);
    }

    #[test]
    fn req_020_bridge_1000_records_with_concurrent_reader() {
        let dir = tempfile::tempdir().unwrap();
        let path = Arc::new(dir.path().to_path_buf());
        let done = Arc::new(AtomicBool::new(false));
        let reader = {
            let (path, done) = (Arc::clone(&path), Arc::clone(&done));
            thread::spawn(move || {
                let (mut parsed, mut partial, mut other) = (0u32, 0u32, 0u32);
                while !done.load(Ordering::Relaxed) {
                    match read_latest(&path) {
                        Ok(Some(_)) => parsed += 1,
                        Ok(None) => {}
                        // A file that does not parse would mean a reader saw a half-written file.
                        Err(StoreError::Format(_)) => partial += 1,
                        Err(_) => other += 1,
                    }
                    thread::sleep(Duration::from_millis(1));
                }
                (parsed, partial, other)
            })
        };
        for i in 0..1000 {
            let input = format!(
                r#"{{"session_id":"s{i}","rate_limits":{{"five_hour":{{"used_percentage":{},"resets_at":1738425600}}}}}}"#,
                i % 100
            );
            assert_eq!(run(input.as_bytes(), std::io::sink(), &path, &path), 0);
        }
        done.store(true, Ordering::Relaxed);
        let (parsed, partial, other) = reader.join().unwrap();
        assert_eq!(partial, 0, "a reader saw an unparsable latest.json");
        assert!(parsed > 0, "the reader never saw a record");
        eprintln!("reader: {parsed} records read, {other} transient read errors");
        assert_eq!(read_history(&path).unwrap().records.len(), 1000);
    }
}
