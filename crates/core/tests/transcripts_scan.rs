// SPDX-License-Identifier: Apache-2.0
//! The incremental scanner on real files in a temporary Claude folder.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::Utc;
use cockpit_core::transcripts::Scanner;

fn line(id: &str, output: u32) -> String {
    format!(
        r#"{{"type":"assistant","timestamp":"2026-03-01T10:00:00Z","requestId":"r_{id}","message":{{"id":"{id}","model":"m","usage":{{"input_tokens":1,"output_tokens":{output}}}}}}}"#
    )
}

fn transcript(claude: &Path, project: &str, name: &str) -> PathBuf {
    let dir = claude.join("projects").join(project);
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn append(path: &Path, text: &str) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    file.write_all(text.as_bytes()).unwrap();
}

fn outputs(scanner: &Scanner) -> u64 {
    scanner.stats(&Utc).total().output
}

#[test]
fn req_014_scan_reads_only_new_lines() {
    let claude = tempfile::tempdir().unwrap();
    let file = transcript(claude.path(), "p1", "a.jsonl");
    append(&file, &format!("{}\n{}\n", line("m1", 10), line("m2", 20)));
    let mut scanner = Scanner::new();
    scanner.scan(claude.path(), UNIX_EPOCH).unwrap();
    assert_eq!(outputs(&scanner), 30);

    append(&file, &format!("{}\n", line("m3", 5)));
    scanner.scan(claude.path(), UNIX_EPOCH).unwrap();
    assert_eq!(outputs(&scanner), 35, "only the appended line was added");
    scanner.scan(claude.path(), UNIX_EPOCH).unwrap();
    assert_eq!(
        outputs(&scanner),
        35,
        "a scan without change changes nothing"
    );
    assert_eq!(scanner.stats(&Utc).understood_lines, 3);
}

#[test]
fn req_014_scan_handles_truncated_file() {
    let claude = tempfile::tempdir().unwrap();
    let file = transcript(claude.path(), "p1", "a.jsonl");
    append(&file, &format!("{}\n{}\n", line("m1", 10), line("m2", 20)));
    let mut scanner = Scanner::new();
    scanner.scan(claude.path(), UNIX_EPOCH).unwrap();
    assert_eq!(outputs(&scanner), 30);

    // The file is replaced by a shorter one: its old entries must not stay.
    fs::write(&file, format!("{}\n", line("n1", 7))).unwrap();
    scanner.scan(claude.path(), UNIX_EPOCH).unwrap();
    assert_eq!(outputs(&scanner), 7);
    assert_eq!(scanner.stats(&Utc).understood_lines, 1);
}

#[test]
fn req_014_scan_ignores_partial_last_line() {
    let claude = tempfile::tempdir().unwrap();
    let file = transcript(claude.path(), "p1", "a.jsonl");
    let second = line("m2", 20);
    let (head, tail) = second.split_at(second.len() / 2);
    append(&file, &format!("{}\n{head}", line("m1", 10)));
    let mut scanner = Scanner::new();
    scanner.scan(claude.path(), UNIX_EPOCH).unwrap();
    assert_eq!(outputs(&scanner), 10, "the half line is not read");

    append(&file, &format!("{tail}\n"));
    scanner.scan(claude.path(), UNIX_EPOCH).unwrap();
    assert_eq!(outputs(&scanner), 30, "the finished line is read in full");
}

#[test]
fn req_014_scan_walks_all_projects_and_only_jsonl_files() {
    let claude = tempfile::tempdir().unwrap();
    append(
        &transcript(claude.path(), "p1", "a.jsonl"),
        &format!("{}\n", line("m1", 1)),
    );
    append(
        &transcript(claude.path(), "p2/deeper", "b.jsonl"),
        &format!("{}\n", line("m2", 2)),
    );
    append(
        &transcript(claude.path(), "p2", "notes.txt"),
        &format!("{}\n", line("m3", 4)),
    );
    let mut scanner = Scanner::new();
    scanner.scan(claude.path(), UNIX_EPOCH).unwrap();
    assert_eq!(outputs(&scanner), 3);
    assert_eq!(scanner.file_count(), 2);
}

#[test]
fn req_014_scan_skips_files_not_modified_after_since() {
    let claude = tempfile::tempdir().unwrap();
    let old = transcript(claude.path(), "p1", "old.jsonl");
    append(&old, &format!("{}\n", line("m1", 100)));
    let long_ago = SystemTime::now() - Duration::from_secs(40 * 86_400);
    File::options()
        .write(true)
        .open(&old)
        .unwrap()
        .set_modified(long_ago)
        .unwrap();
    append(
        &transcript(claude.path(), "p1", "new.jsonl"),
        &format!("{}\n", line("m2", 1)),
    );

    let mut scanner = Scanner::new();
    let since = SystemTime::now() - Duration::from_secs(30 * 86_400);
    scanner.scan(claude.path(), since).unwrap();
    assert_eq!(outputs(&scanner), 1);
}

#[test]
fn req_014_a_message_repeated_in_two_files_counts_once() {
    let claude = tempfile::tempdir().unwrap();
    let text = format!("{}\n", line("m1", 9));
    append(&transcript(claude.path(), "p1", "a.jsonl"), &text);
    append(&transcript(claude.path(), "p1", "b.jsonl"), &text);
    let mut scanner = Scanner::new();
    scanner.scan(claude.path(), UNIX_EPOCH).unwrap();
    assert_eq!(outputs(&scanner), 9);
}

#[test]
fn req_014_scan_without_projects_folder_is_not_an_error() {
    let claude = tempfile::tempdir().unwrap();
    let mut scanner = Scanner::new();
    scanner.scan(claude.path(), UNIX_EPOCH).unwrap();
    scanner
        .scan(&claude.path().join("missing"), UNIX_EPOCH)
        .unwrap();
    assert_eq!(scanner.stats(&Utc).understood_lines, 0);
}

#[test]
fn req_014_deleted_files_are_forgotten() {
    let claude = tempfile::tempdir().unwrap();
    let file = transcript(claude.path(), "p1", "a.jsonl");
    append(&file, &format!("{}\n", line("m1", 5)));
    let mut scanner = Scanner::new();
    scanner.scan(claude.path(), UNIX_EPOCH).unwrap();
    fs::remove_file(&file).unwrap();
    scanner.scan(claude.path(), UNIX_EPOCH).unwrap();
    assert_eq!(scanner.file_count(), 0);
    assert_eq!(outputs(&scanner), 0);
}

#[test]
fn req_014_a_line_with_windows_line_ending_is_read() {
    let claude = tempfile::tempdir().unwrap();
    let file = transcript(claude.path(), "p1", "a.jsonl");
    append(
        &file,
        &format!("{}\r\n{}\r\n", line("m1", 1), line("m2", 2)),
    );
    let mut scanner = Scanner::new();
    scanner.scan(claude.path(), UNIX_EPOCH).unwrap();
    assert_eq!(outputs(&scanner), 3);
}

#[test]
fn req_014_the_same_message_in_two_files_resolves_the_same_way_every_time() {
    let claude = tempfile::tempdir().unwrap();
    append(
        &transcript(claude.path(), "p1", "a.jsonl"),
        &format!(
            "{}
",
            line("m1", 5)
        ),
    );
    append(
        &transcript(claude.path(), "p1", "b.jsonl"),
        &format!(
            "{}
",
            line("m1", 9)
        ),
    );
    for _ in 0..20 {
        let mut scanner = Scanner::new();
        scanner.scan(claude.path(), UNIX_EPOCH).unwrap();
        assert_eq!(outputs(&scanner), 9, "the later path wins");
    }
}
