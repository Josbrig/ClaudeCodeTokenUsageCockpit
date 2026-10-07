// SPDX-License-Identifier: Apache-2.0
//! "Remove everything" (REQ-119): undoes what the cockpit created outside its own file, so that
//! the portable executable can be deleted by hand without leftovers.
//!
//! Steps, in this order: the bridge entry in the Claude Code settings (with a backup, as
//! `remove-bridge`), the start entry of the system (REQ-118), and, only if the person chose it,
//! the data and configuration folders. The folders are cleaned file by file: only the files the
//! cockpit knows are deleted, and a folder is removed only when it is empty afterwards, so a
//! `USAGE_COCKPIT_HOME` that points at something important cannot lose other files. The backups
//! of the Claude Code settings and the program file itself always stay.
//!
//! Nothing here asks questions: the callers (command line, window) show [`describe`] first.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use crate::autostart;
use crate::setup::{self, RemoveOutcome};

/// Files of the data folder that the cockpit and the bridge create.
const DATA_FILES: &[&str] = &[
    "latest.json",
    "last_error.json",
    "history-v1.jsonl",
    "history.lock",
    "log.txt",
    "log.1.txt",
    "log.lock",
    "cockpit.lock",
    MARKER,
];
/// Files of the configuration folder.
const CONFIG_FILES: &[&str] = &[
    "settings.toml",
    "settings.toml.invalid",
    "bridge-state.json",
];
/// Written into the data folder when the window asks the helper to delete the data. The helper
/// does nothing without it, so a hand-typed `finish-uninstall` cannot delete anything.
const MARKER: &str = "uninstall-pending";
/// The folder above `data` and `config` on Windows and macOS, removed too when it is empty.
const PARENT_NAME: &str = "usage-cockpit";
/// How often and how long to retry a file that is still locked by a process that just ended.
const RETRIES: u32 = 50;
const RETRY_PAUSE: Duration = Duration::from_millis(200);

/// What the person decided about the data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Data {
    /// Keep history and settings.
    Keep,
    /// Delete history, settings, logs and the stored status line.
    Delete,
}

/// The files and folders involved.
#[derive(Debug, Clone)]
pub struct Locations {
    /// The Claude Code settings file.
    pub claude_settings: PathBuf,
    /// `bridge-state.json`.
    pub bridge_state: PathBuf,
    /// The data folder.
    pub data_dir: PathBuf,
    /// The configuration folder.
    pub config_dir: PathBuf,
    /// This program.
    pub exe: PathBuf,
    /// Name of the start entry of the system ([`autostart::default_name`]; tests use their own).
    pub autostart_name: String,
}

/// What a run did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// One line per step, in plain words.
    pub lines: Vec<String>,
    /// A step failed; nothing that depends on it was done.
    pub failed: bool,
    /// The folders are to be deleted after this process ended ([`finish_data`]).
    pub data_pending: bool,
}

impl Report {
    /// The lines as one text.
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }
}

/// The bridge is the status line command at the moment.
fn bridge_is_set(loc: &Locations) -> Result<bool, setup::SetupError> {
    let outcome = setup::remove(&loc.claude_settings, &loc.bridge_state, &mut |_| false)?;
    Ok(outcome != RemoveOutcome::NotTheBridge)
}

/// What [`run`] would do, in plain words. Changes nothing.
pub fn describe(loc: &Locations, data: Data) -> String {
    let mut lines = Vec::new();
    lines.push(match bridge_is_set(loc) {
        Ok(true) => format!(
            "- Remove the bridge from {} (a backup is made first and your previous status line comes back).",
            loc.claude_settings.display()
        ),
        Ok(false) => "- The bridge is not set up in the Claude Code settings: nothing to remove there.".to_owned(),
        Err(error) => format!("- The Claude Code settings cannot be changed now: {error}"),
    });
    lines.push(match autostart::state_for(&loc.autostart_name, &loc.exe) {
        Ok(autostart::State::Off | autostart::State::Unsupported) => {
            "- There is no start entry of the system: nothing to remove there.".to_owned()
        }
        Ok(autostart::State::Stale { found }) => {
            format!("- Remove the start entry of the system (it starts {found}, not this file).")
        }
        Ok(_) => "- Remove the start entry of the system (\"start with Windows\").".to_owned(),
        Err(error) => format!("- The start entry cannot be read: {error}"),
    });
    lines.push(match data {
        Data::Keep => format!(
            "- Keep the history and settings in {} and {}.",
            loc.data_dir.display(),
            loc.config_dir.display()
        ),
        Data::Delete => format!(
            "- Delete the history, logs and settings in {} and {}. This cannot be undone.",
            loc.data_dir.display(),
            loc.config_dir.display()
        ),
    });
    lines.push(
        "The copies of your Claude Code settings made earlier stay. The program file stays too: delete it by hand afterwards."
            .to_owned(),
    );
    lines.join("\n")
}

/// Does the steps. With `Data::Delete` the folders are deleted at once if `data_now`, else
/// the report says `data_pending` and the caller starts [`finish_data`] after it ended itself
/// (the running window holds files open).
pub fn run(loc: &Locations, data: Data, data_now: bool) -> Report {
    let mut report = Report::default();

    match setup::remove(&loc.claude_settings, &loc.bridge_state, &mut |_| true) {
        Ok(RemoveOutcome::Removed { backup }) => report.lines.push(format!(
            "The bridge is removed from the Claude Code settings. A copy of the old settings is {}.",
            backup.display()
        )),
        Ok(_) => report
            .lines
            .push("The bridge was not set up; nothing changed in the Claude Code settings.".to_owned()),
        Err(error) => {
            report.failed = true;
            report
                .lines
                .push(format!("The bridge could not be removed: {error}"));
        }
    }

    match autostart::state_for(&loc.autostart_name, &loc.exe) {
        Ok(autostart::State::Off | autostart::State::Unsupported) => {}
        Ok(_) => match autostart::set_for(&loc.autostart_name, &loc.exe, false) {
            Ok(_) => report
                .lines
                .push("The start entry of the system is removed.".to_owned()),
            Err(error) => {
                report.failed = true;
                report
                    .lines
                    .push(format!("The start entry could not be removed: {error}"));
            }
        },
        Err(error) => {
            report.failed = true;
            report
                .lines
                .push(format!("The start entry could not be read: {error}"));
        }
    }

    match data {
        Data::Keep => report
            .lines
            .push("History and settings are kept.".to_owned()),
        // The stored status line lives in the configuration folder: if the bridge is still in
        // the Claude Code settings, deleting it would lose the person's previous status line.
        Data::Delete if report.failed => report
            .lines
            .push("History and settings are kept, because a step above failed.".to_owned()),
        Data::Delete if !data_now => match write_marker(loc) {
            Ok(()) => report.data_pending = true,
            Err(error) => {
                report.failed = true;
                report.lines.push(format!(
                    "The history and settings are kept: the deletion could not be prepared ({error})."
                ));
            }
        },
        Data::Delete => {
            let (lines, failed) = delete_data(loc, 1);
            report.lines.extend(lines);
            report.failed |= failed;
        }
    }
    report
}

fn write_marker(loc: &Locations) -> io::Result<()> {
    fs::create_dir_all(&loc.data_dir)?;
    fs::write(loc.data_dir.join(MARKER), b"")
}

/// The part that runs after the window ended (the helper process). It deletes only if the window
/// asked for it (the marker), if `wait` says that the window's process has ended, and if the
/// bridge is really gone from the Claude Code settings (the stored status line in the
/// configuration folder would be lost otherwise).
pub fn finish_data(loc: &Locations, wait: impl FnOnce() -> bool) -> Vec<String> {
    if !loc.data_dir.join(MARKER).is_file() {
        return vec!["The window did not ask for a deletion: nothing was deleted.".to_owned()];
    }
    if !wait() {
        return vec!["The cockpit window is still open: nothing was deleted.".to_owned()];
    }
    match bridge_is_set(loc) {
        Ok(false) => delete_data(loc, RETRIES).0,
        Ok(true) | Err(_) => {
            vec!["The bridge is still set up: nothing was deleted.".to_owned()]
        }
    }
}

/// Deletes the known files and then the folders if they are empty. `attempts` is how often a
/// file that cannot be deleted yet is tried.
fn delete_data(loc: &Locations, attempts: u32) -> (Vec<String>, bool) {
    let mut lines = Vec::new();
    let mut failed = false;
    for (dir, names) in [(&loc.data_dir, DATA_FILES), (&loc.config_dir, CONFIG_FILES)] {
        match clean_dir(dir, names, attempts) {
            Ok(None) => lines.push(format!("Deleted {}.", dir.display())),
            Ok(Some(left)) => lines.push(format!(
                "{} was cleaned, but it also holds files that the cockpit did not create: {}. They stay.",
                dir.display(),
                left.join(", ")
            )),
            Err(error) => {
                failed = true;
                lines.push(format!("{} could not be cleaned: {error}", dir.display()));
            }
        }
        if let Some(parent) = dir.parent()
            && parent.file_name().is_some_and(|name| name == PARENT_NAME)
        {
            // Only succeeds if it is empty, which is what is wanted.
            let _ = fs::remove_dir(parent);
        }
    }
    (lines, failed)
}

/// Removes `names` and the leftover temporary files of their atomic writes (`<name>.<n>.<n>.<n>.tmp`)
/// from `dir`, then the folder itself if it is empty.
/// Returns the names of the files that stay, `None` if the folder is gone (or never was there).
fn clean_dir(dir: &Path, names: &[&str], attempts: u32) -> io::Result<Option<Vec<String>>> {
    // A link as the folder would send the deletion somewhere else: not followed.
    match fs::symlink_metadata(dir) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(io::Error::other("it is a link, not a folder"));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    }
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut left = Vec::new();
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let ours = names.contains(&name.as_str()) || is_temp_of(&name, names);
        if ours && entry.file_type()?.is_file() {
            remove_file_with_retries(&entry.path(), attempts)?;
        } else {
            left.push(name);
        }
    }
    if !left.is_empty() {
        left.sort();
        return Ok(Some(left));
    }
    match fs::remove_dir(dir) {
        Ok(()) => Ok(None),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// `<known name>.<digits and dots>.tmp`, the name the atomic write of the store gives its
/// temporary file.
fn is_temp_of(name: &str, names: &[&str]) -> bool {
    let Some(stem) = name.strip_suffix(".tmp") else {
        return false;
    };
    names.iter().any(|known| {
        stem.strip_prefix(known)
            .and_then(|rest| rest.strip_prefix('.'))
            .is_some_and(|rest| {
                !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit() || c == '.')
            })
    })
}

fn remove_file_with_retries(path: &Path, attempts: u32) -> io::Result<()> {
    let mut tried = 0;
    loop {
        match fs::remove_file(path) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                tried += 1;
                if tried >= attempts {
                    return Err(error);
                }
                thread::sleep(RETRY_PAUSE);
            }
        }
    }
}

/// Waits until the process with this id has ended (at most five minutes); `true` if it has ended
/// or is not there. The helper started by the window uses it to wait for the window.
#[cfg(windows)]
pub fn wait_for_exit(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{OpenProcess, WaitForSingleObject};

    const SYNCHRONIZE: u32 = 0x0010_0000;
    // SAFETY: plain handle calls; the handle is closed again, and a null handle (the process is
    // gone already or cannot be opened) is not used.
    unsafe {
        let handle = OpenProcess(SYNCHRONIZE, 0, pid);
        if handle.is_null() {
            return true;
        }
        let ended = WaitForSingleObject(handle, 5 * 60 * 1000) == WAIT_OBJECT_0;
        CloseHandle(handle);
        ended
    }
}

/// Other systems follow in their own issues; a short pause stands in for the wait.
#[cfg(not(windows))]
pub fn wait_for_exit(_pid: u32) -> bool {
    thread::sleep(Duration::from_secs(3));
    true
}

/// Starts this program again, hidden and detached, to run [`finish_data`] after the current
/// process has ended.
pub fn start_finish_helper(exe: &Path) -> io::Result<()> {
    use std::process::{Command, Stdio};

    let mut command = Command::new(exe);
    command
        .arg("finish-uninstall")
        .arg("--after")
        .arg(std::process::id().to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW | DETACHED_PROCESS
        command.creation_flags(0x0800_0000 | 0x0000_0008);
    }
    command.spawn().map(|_| ())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;

    use super::*;

    struct Fixture {
        _root: tempfile::TempDir,
        loc: Locations,
    }

    fn fixture() -> Fixture {
        let root = tempfile::tempdir().unwrap();
        let base = root.path();
        let loc = Locations {
            claude_settings: base.join("claude").join("settings.json"),
            bridge_state: base.join("config").join("bridge-state.json"),
            data_dir: base.join("data"),
            config_dir: base.join("config"),
            exe: base.join("bin").join("usage-cockpit.exe"),
            autostart_name: format!("UsageCockpitTest{}", std::process::id()),
        };
        for dir in [&loc.data_dir, &loc.config_dir, &base.join("claude")] {
            fs::create_dir_all(dir).unwrap();
        }
        for name in DATA_FILES {
            fs::write(loc.data_dir.join(name), b"x").unwrap();
        }
        fs::write(loc.data_dir.join("latest.json.123.4.tmp"), b"x").unwrap();
        fs::write(loc.config_dir.join("settings.toml"), b"version = 1\n").unwrap();
        Fixture { _root: root, loc }
    }

    fn put_bridge(f: &Fixture, previous: Option<&str>) {
        let command = format!("{} bridge", f.loc.exe.to_string_lossy().replace('\\', "/"));
        fs::write(
            &f.loc.claude_settings,
            json!({"model": "x", "statusLine": {"type": "command", "command": command}})
                .to_string(),
        )
        .unwrap();
        let state = match previous {
            Some(command) => {
                json!({"v": 1, "previous_status_line": {"type": "command", "command": command}})
            }
            None => json!({"v": 1, "previous_status_line": null}),
        };
        fs::write(&f.loc.bridge_state, state.to_string()).unwrap();
    }

    #[test]
    fn req_119_the_plan_names_every_step_and_changes_nothing() {
        let f = fixture();
        put_bridge(&f, Some("ccusage"));
        let before = fs::read(&f.loc.claude_settings).unwrap();
        let text = describe(&f.loc, Data::Delete);
        assert!(text.contains("Remove the bridge"), "{text}");
        assert!(text.contains("Delete the history"), "{text}");
        assert!(text.contains("stay"), "{text}");
        assert!(describe(&f.loc, Data::Keep).contains("Keep the history"));
        assert_eq!(fs::read(&f.loc.claude_settings).unwrap(), before);
        assert!(f.loc.data_dir.join("latest.json").exists());
    }

    #[test]
    fn req_119_keeping_the_data_removes_only_the_bridge() {
        let f = fixture();
        put_bridge(&f, Some("ccusage"));
        let report = run(&f.loc, Data::Keep, true);
        assert!(!report.failed && !report.data_pending, "{report:?}");
        let settings: serde_json::Value =
            serde_json::from_slice(&fs::read(&f.loc.claude_settings).unwrap()).unwrap();
        assert_eq!(settings["statusLine"]["command"], "ccusage");
        assert_eq!(settings["model"], "x");
        assert!(f.loc.data_dir.join("latest.json").exists());
        assert!(f.loc.config_dir.join("settings.toml").exists());
        let backups = fs::read_dir(f.loc.claude_settings.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .contains("usage-cockpit-backup")
            })
            .count();
        assert_eq!(backups, 1, "the backup stays");
    }

    #[test]
    fn req_119_deleting_the_data_leaves_no_folder_and_no_cockpit_file() {
        let f = fixture();
        put_bridge(&f, None);
        let report = run(&f.loc, Data::Delete, true);
        assert!(!report.failed && !report.data_pending, "{report:?}");
        assert!(!f.loc.data_dir.exists(), "{report:?}");
        assert!(!f.loc.config_dir.exists(), "{report:?}");
        let settings: serde_json::Value =
            serde_json::from_slice(&fs::read(&f.loc.claude_settings).unwrap()).unwrap();
        assert!(settings.get("statusLine").is_none());
    }

    #[test]
    fn req_119_foreign_files_in_the_folders_stay() {
        let f = fixture();
        fs::write(f.loc.data_dir.join("notes.txt"), b"mine").unwrap();
        let report = run(&f.loc, Data::Delete, true);
        assert!(f.loc.data_dir.join("notes.txt").exists());
        assert!(!f.loc.data_dir.join("latest.json").exists());
        assert!(report.text().contains("notes.txt"), "{report:?}");
        assert!(!f.loc.config_dir.exists());
    }

    #[test]
    fn req_119_the_data_stays_when_the_bridge_cannot_be_removed() {
        let f = fixture();
        put_bridge(&f, Some("ccusage"));
        // A state file that cannot be used makes the removal refuse; it holds the old status line.
        fs::write(&f.loc.bridge_state, b"not json").unwrap();
        let report = run(&f.loc, Data::Delete, true);
        assert!(report.failed, "{report:?}");
        assert!(f.loc.data_dir.join("latest.json").exists());
        assert!(f.loc.config_dir.join("bridge-state.json").exists());
        assert!(report.text().contains("kept"), "{report:?}");
    }

    #[test]
    fn req_119_a_window_defers_the_deletion_until_it_ended() {
        let f = fixture();
        let report = run(&f.loc, Data::Delete, false);
        assert!(report.data_pending && !report.failed, "{report:?}");
        assert!(f.loc.data_dir.join("latest.json").exists());
        // the helper deletes after the wait
        assert!(
            f.loc.data_dir.join(MARKER).is_file(),
            "the window asked for it"
        );
        let mut waited = false;
        let lines = finish_data(&f.loc, || {
            waited = true;
            true
        });
        assert!(waited);
        assert!(
            !f.loc.data_dir.exists() && !f.loc.config_dir.exists(),
            "{lines:?}"
        );
    }

    /// Removes a test start entry when the test ends, also after a failure.
    #[cfg(windows)]
    struct ForgetEntry(String);

    #[cfg(windows)]
    impl Drop for ForgetEntry {
        fn drop(&mut self) {
            autostart::forget_test_entry(&self.0);
        }
    }

    #[cfg(windows)]
    #[test]
    fn req_119_the_start_entry_is_removed() {
        let f = fixture();
        let _forget = ForgetEntry(f.loc.autostart_name.clone());
        autostart::set_for(&f.loc.autostart_name, &f.loc.exe, true).unwrap();
        assert!(describe(&f.loc, Data::Keep).contains("Remove the start entry"));
        let report = run(&f.loc, Data::Keep, true);
        assert!(!report.failed, "{report:?}");
        assert!(report.text().contains("start entry"), "{report:?}");
        assert_eq!(
            autostart::state_for(&f.loc.autostart_name, &f.loc.exe).unwrap(),
            autostart::State::Off
        );
    }

    #[test]
    fn req_119_the_helper_deletes_nothing_without_the_marker() {
        let f = fixture();
        fs::remove_file(f.loc.data_dir.join(MARKER)).unwrap();
        let lines = finish_data(&f.loc, || true);
        assert!(f.loc.data_dir.join("latest.json").exists(), "{lines:?}");
        assert!(lines[0].contains("did not ask"), "{lines:?}");
    }

    #[test]
    fn req_119_the_helper_deletes_nothing_while_the_window_runs() {
        let f = fixture();
        run(&f.loc, Data::Delete, false);
        let lines = finish_data(&f.loc, || false);
        assert!(f.loc.data_dir.join("latest.json").exists(), "{lines:?}");
        assert!(lines[0].contains("still open"), "{lines:?}");
    }

    #[test]
    fn req_119_the_helper_deletes_nothing_while_the_bridge_is_set_up() {
        let f = fixture();
        put_bridge(&f, Some("ccusage"));
        run(&f.loc, Data::Delete, false);
        // the bridge came back after the window decided
        put_bridge(&f, Some("ccusage"));
        let lines = finish_data(&f.loc, || true);
        assert!(
            f.loc.config_dir.join("bridge-state.json").exists(),
            "{lines:?}"
        );
        assert!(lines[0].contains("bridge is still set up"), "{lines:?}");
    }

    #[test]
    fn req_119_only_the_temporary_files_of_the_cockpit_are_deleted() {
        let f = fixture();
        fs::write(f.loc.data_dir.join("my-notes.tmp"), b"mine").unwrap();
        fs::write(f.loc.data_dir.join("latest.json.not-ours.tmp"), b"mine").unwrap();
        let report = run(&f.loc, Data::Delete, true);
        assert!(f.loc.data_dir.join("my-notes.tmp").exists(), "{report:?}");
        assert!(f.loc.data_dir.join("latest.json.not-ours.tmp").exists());
        assert!(!f.loc.data_dir.join("latest.json.123.4.tmp").exists());
        assert!(is_temp_of("settings.toml.9.10.11.tmp", CONFIG_FILES));
        assert!(!is_temp_of("settings.toml.tmp", CONFIG_FILES));
    }

    #[test]
    fn req_119_a_failed_deletion_makes_the_run_fail() {
        let f = fixture();
        // a folder where a file of the cockpit should be cannot be deleted as a file: it stays
        // and the folder is not empty, which is reported but not an error; a file that cannot be
        // deleted is an error. A directory named like a known file is left alone (not a file).
        fs::remove_file(f.loc.data_dir.join("log.txt")).unwrap();
        fs::create_dir(f.loc.data_dir.join("log.txt")).unwrap();
        let report = run(&f.loc, Data::Delete, true);
        assert!(report.text().contains("log.txt"), "{report:?}");
        assert!(f.loc.data_dir.join("log.txt").is_dir());
    }

    #[test]
    fn req_119_a_missing_folder_is_not_an_error() {
        let f = fixture();
        fs::remove_dir_all(&f.loc.data_dir).unwrap();
        let report = run(&f.loc, Data::Delete, true);
        assert!(!report.failed, "{report:?}");
    }

    #[test]
    fn req_119_the_parent_folder_goes_when_it_is_empty() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join(PARENT_NAME);
        let loc = Locations {
            claude_settings: root.path().join("settings.json"),
            bridge_state: parent.join("config").join("bridge-state.json"),
            data_dir: parent.join("data"),
            config_dir: parent.join("config"),
            exe: root.path().join("usage-cockpit.exe"),
            autostart_name: format!("UsageCockpitTest{}", std::process::id()),
        };
        fs::create_dir_all(&loc.data_dir).unwrap();
        fs::create_dir_all(&loc.config_dir).unwrap();
        fs::write(loc.data_dir.join("log.txt"), b"x").unwrap();
        run(&loc, Data::Delete, true);
        assert!(!parent.exists());
    }
}
