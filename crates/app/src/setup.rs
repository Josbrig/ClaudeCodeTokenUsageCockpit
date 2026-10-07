// SPDX-License-Identifier: Apache-2.0
//! `setup-bridge` and `remove-bridge` (concept §5.4): make the bridge the status line command of
//! Claude Code and undo that.
//!
//! The functions take every path and the consent as arguments, so the command line and, later,
//! the window use them in the same way and tests need no global state.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use cockpit_core::store::write_atomic;
use serde_json::{Map, Value, json};

/// Key of the status line in the Claude Code settings.
const STATUS_LINE_KEY: &str = "statusLine";
/// Key in `bridge-state.json` that holds the replaced status line.
const PREVIOUS_KEY: &str = "previous_status_line";
/// Name of the program in the status line command.
const PROGRAM_NAME: &str = "usage-cockpit";

/// What `setup` did.
#[derive(Debug, PartialEq, Eq)]
pub enum SetupOutcome {
    /// The settings were changed; `backup` is the copy of the previous file, if there was one.
    Changed { backup: Option<PathBuf> },
    /// The settings already name this program as the status line command.
    AlreadySetUp,
    /// The user did not agree; nothing was written.
    Declined,
}

/// What `remove` did.
#[derive(Debug, PartialEq, Eq)]
pub enum RemoveOutcome {
    /// The previous status line was put back, or the key was removed if there was none.
    Removed { backup: PathBuf },
    /// The current status line is not the bridge; nothing was changed.
    NotTheBridge,
    /// The user did not agree; nothing was written.
    Declined,
}

/// Why `setup` or `remove` could not do their job; the files are unchanged in every case that
/// happens before the first write.
#[derive(Debug, thiserror::Error)]
pub enum SetupError {
    /// A space in the path would need quoting, which is only done on Windows so far.
    #[error(
        "the path of the executable contains a space ({0}); place usage-cockpit in a folder \
         without spaces and run the command again"
    )]
    #[cfg_attr(windows, allow(dead_code))]
    PathWithSpace(PathBuf),
    /// The settings file does not hold a JSON object.
    #[error("{0} is not a JSON object; fix or remove it by hand")]
    NotAnObject(PathBuf),
    /// The settings file is not valid JSON.
    #[error("{path} cannot be parsed: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    /// `bridge-state.json` exists but cannot be used; removing the bridge would lose the
    /// previous status line stored in it.
    #[error(
        "{path} cannot be used: {reason}; fix or delete it by hand, then run the command again"
    )]
    State { path: PathBuf, reason: String },
    /// A file operation failed.
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

fn io_error(path: &Path) -> impl FnOnce(io::Error) -> SetupError + '_ {
    move |source| SetupError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Sets the bridge as status line command in `claude_settings`.
///
/// `confirm` gets the planned change as text and returns whether to go on. A replaced status
/// line is kept in `bridge_state`.
pub fn setup(
    claude_settings: &Path,
    exe: &Path,
    bridge_state: &Path,
    confirm: &mut dyn FnMut(&str) -> bool,
) -> Result<SetupOutcome, SetupError> {
    let command = bridge_command(exe)?;
    let (mut settings, existed) = read_settings(claude_settings)?;
    let current = settings.get(STATUS_LINE_KEY).cloned();
    let current_is_bridge = current.as_ref().is_some_and(is_bridge);
    if current_command(current.as_ref()) == Some(command.as_str()) {
        return Ok(SetupOutcome::AlreadySetUp);
    }
    let plan = setup_plan(
        claude_settings,
        &command,
        current.as_ref().filter(|_| !current_is_bridge),
        bridge_state,
        existed,
    );
    if !confirm(&plan) {
        return Ok(SetupOutcome::Declined);
    }
    let backup = if existed {
        Some(back_up(claude_settings)?)
    } else {
        None
    };
    if !current_is_bridge {
        // An old bridge entry (for example from another folder) must not become "previous".
        write_state(bridge_state, current)?;
    }
    settings.insert(
        STATUS_LINE_KEY.to_owned(),
        json!({"type": "command", "command": command}),
    );
    write_settings(claude_settings, settings)?;
    Ok(SetupOutcome::Changed { backup })
}

/// Puts the previous status line back, or removes the key if there was none.
///
/// Does nothing if the current status line is not the bridge.
pub fn remove(
    claude_settings: &Path,
    bridge_state: &Path,
    confirm: &mut dyn FnMut(&str) -> bool,
) -> Result<RemoveOutcome, SetupError> {
    if !claude_settings.exists() {
        return Ok(RemoveOutcome::NotTheBridge);
    }
    let (mut settings, _) = read_settings(claude_settings)?;
    if !settings.get(STATUS_LINE_KEY).is_some_and(is_bridge) {
        return Ok(RemoveOutcome::NotTheBridge);
    }
    let previous = read_previous(bridge_state)?;
    let plan = remove_plan(claude_settings, previous.as_ref());
    if !confirm(&plan) {
        return Ok(RemoveOutcome::Declined);
    }
    let backup = back_up(claude_settings)?;
    match previous {
        Some(previous) => {
            settings.insert(STATUS_LINE_KEY.to_owned(), previous);
        }
        None => {
            settings.shift_remove(STATUS_LINE_KEY);
        }
    }
    write_settings(claude_settings, settings)?;
    // Only after the settings are written: a failure above keeps the stored command usable.
    write_state(bridge_state, None)?;
    Ok(RemoveOutcome::Removed { backup })
}

/// `<exe with forward slashes> bridge`.
///
/// A path with a space is not accepted by every shell Claude Code may use. On Windows the 8.3
/// short name is used then (it has no space and works in every shell); if the volume has none,
/// the path is put in double quotes, which Git Bash and cmd accept. Elsewhere such a path is
/// refused until the quoting for `sh -c` is done.
fn bridge_command(exe: &Path) -> Result<String, SetupError> {
    let path = exe.to_string_lossy().replace('\\', "/");
    if !path.contains(' ') {
        return Ok(format!("{path} bridge"));
    }
    #[cfg(windows)]
    {
        // Only the folder is shortened: the file name must stay `usage-cockpit[.exe]` so that
        // `is_bridge` still recognises the command.
        let short = exe
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.contains(' '))
            .zip(exe.parent().and_then(short_path))
            .map(|(name, folder)| format!("{}/{name}", folder.replace('\\', "/")))
            .filter(|short| !short.contains(' '));
        Ok(windows_command(&path, short))
    }
    #[cfg(not(windows))]
    {
        Err(SetupError::PathWithSpace(exe.to_path_buf()))
    }
}

/// The command for a path with a space: the short form if there is one, else the quoted path.
#[cfg(windows)]
fn windows_command(path: &str, short: Option<String>) -> String {
    match short {
        Some(short) => format!("{short} bridge"),
        None => format!("\"{path}\" bridge"),
    }
}

/// The 8.3 short form of an existing path, if the volume has one.
#[cfg(windows)]
fn short_path(path: &Path) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    // SAFETY: `wide` is NUL-terminated; a null buffer of length 0 only asks for the needed size.
    let needed = unsafe { GetShortPathNameW(wide.as_ptr(), std::ptr::null_mut(), 0) };
    if needed == 0 {
        return None;
    }
    let mut buffer = vec![0u16; needed as usize];
    // SAFETY: `buffer` holds `needed` units and `wide` is NUL-terminated.
    let written = unsafe { GetShortPathNameW(wide.as_ptr(), buffer.as_mut_ptr(), needed) };
    if written == 0 || written >= needed {
        return None;
    }
    String::from_utf16(&buffer[..written as usize]).ok()
}

fn current_command(status_line: Option<&Value>) -> Option<&str> {
    status_line?.get("command")?.as_str()
}

/// `true` if the status line is a command of the form `<path>/usage-cockpit[.exe] bridge`.
fn is_bridge(status_line: &Value) -> bool {
    if status_line.get("type").and_then(Value::as_str) != Some("command") {
        return false;
    }
    let Some(command) = current_command(Some(status_line)) else {
        return false;
    };
    // `"<path with spaces>" bridge` or `<path> bridge`
    let command = command.trim_start();
    let (program, rest) = match command.strip_prefix('"') {
        Some(quoted) => match quoted.split_once('"') {
            Some((program, rest)) => (program, rest),
            None => return false,
        },
        None => match command.split_once(char::is_whitespace) {
            Some((program, rest)) => (program, rest),
            None => return false,
        },
    };
    // after the closing quote a space must follow, else a shell sees one word
    if command.starts_with('"') && !rest.starts_with(char::is_whitespace) {
        return false;
    }
    let mut parts = rest.split_whitespace();
    let (Some("bridge"), None) = (parts.next(), parts.next()) else {
        return false;
    };
    let name = program.rsplit(['/', '\\']).next().unwrap_or_default();
    let name = name.to_lowercase();
    name == PROGRAM_NAME || name == format!("{PROGRAM_NAME}.exe")
}

/// The settings as an ordered JSON object, and whether the file existed.
fn read_settings(path: &Path) -> Result<(Map<String, Value>, bool), SetupError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok((Map::new(), false)),
        Err(error) => return Err(io_error(path)(error)),
    };
    let text = String::from_utf8_lossy(&bytes);
    let text = text.trim_start_matches('\u{feff}');
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(map)) => Ok((map, true)),
        Ok(_) => Err(SetupError::NotAnObject(path.to_path_buf())),
        Err(source) => Err(SetupError::Parse {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn write_settings(path: &Path, settings: Map<String, Value>) -> Result<(), SetupError> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(io_error(dir))?;
    }
    let mut text = serde_json::to_string_pretty(&Value::Object(settings))
        .expect("a JSON value can always be written");
    text.push('\n');
    write_atomic(path, text.as_bytes()).map_err(io_error(path))
}

/// `bridge-state.json` with the given previous status line (`None` is written as `null`).
fn write_state(path: &Path, previous: Option<Value>) -> Result<(), SetupError> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(io_error(dir))?;
    }
    let state = json!({"v": 1, PREVIOUS_KEY: previous.unwrap_or(Value::Null)});
    let mut text = serde_json::to_string(&state).expect("a JSON value can always be written");
    text.push('\n');
    write_atomic(path, text.as_bytes()).map_err(io_error(path))
}

/// The stored previous status line; `None` if the file is missing or holds none. A file that
/// exists but cannot be used is an error, so that the user's old command is not overwritten.
fn read_previous(path: &Path) -> Result<Option<Value>, SetupError> {
    let state_error = |reason: String| SetupError::State {
        path: path.to_path_buf(),
        reason,
    };
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(state_error(error.to_string())),
    };
    let state: Value =
        serde_json::from_slice(&bytes).map_err(|error| state_error(error.to_string()))?;
    let Some(object) = state.as_object() else {
        return Err(state_error("not a JSON object".to_owned()));
    };
    Ok(object.get(PREVIOUS_KEY).filter(|v| !v.is_null()).cloned())
}

/// Copies the file to `<name>.usage-cockpit-backup-<YYYYMMDD-HHMMSS>` next to it.
fn back_up(path: &Path) -> Result<PathBuf, SetupError> {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    for attempt in 0.. {
        let suffix = if attempt == 0 {
            String::new()
        } else {
            format!("-{attempt}")
        };
        let target = path.with_file_name(format!("{name}.usage-cockpit-backup-{stamp}{suffix}"));
        // Never overwrite an earlier backup that was made within the same second.
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
        {
            Ok(_) => {
                if let Err(error) = fs::copy(path, &target) {
                    let _ = fs::remove_file(&target);
                    return Err(io_error(path)(error));
                }
                return Ok(target);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(io_error(&target)(error)),
        }
    }
    unreachable!("the loop ends with a return")
}

/// The plan of `setup` in words. `file_exists`: the settings file is there, so a backup is made;
/// otherwise the file is created and there is nothing to back up.
fn setup_plan(
    settings: &Path,
    command: &str,
    replaced: Option<&Value>,
    state: &Path,
    file_exists: bool,
) -> String {
    let mut plan = format!(
        "File: {}\nstatusLine will be set to the command: {command}\n",
        settings.display()
    );
    match replaced {
        Some(old) => plan.push_str(&format!(
            "The current statusLine is kept in {} and still runs through the bridge: {old}\n",
            state.display()
        )),
        None => plan.push_str("There is no other statusLine to keep.\n"),
    }
    plan.push_str(if file_exists {
        "A backup of the file is made first."
    } else {
        "The file does not exist yet and will be created; there is nothing to back up."
    });
    plan
}

fn remove_plan(settings: &Path, previous: Option<&Value>) -> String {
    let action = match previous {
        Some(old) => format!("statusLine will be set back to: {old}"),
        None => "statusLine will be removed (there was none before)".to_owned(),
    };
    format!(
        "File: {}\n{action}\nA backup of the file is made first.",
        settings.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dirs {
        _dir: tempfile::TempDir,
        settings: PathBuf,
        state: PathBuf,
        exe: PathBuf,
    }

    fn dirs() -> Dirs {
        let dir = tempfile::tempdir().unwrap();
        Dirs {
            settings: dir.path().join("claude").join("settings.json"),
            state: dir.path().join("config").join("bridge-state.json"),
            exe: dir.path().join("bin").join("usage-cockpit"),
            _dir: dir,
        }
    }

    fn put_settings(d: &Dirs, text: &str) {
        fs::create_dir_all(d.settings.parent().unwrap()).unwrap();
        fs::write(&d.settings, text).unwrap();
    }

    fn yes(_: &str) -> bool {
        true
    }

    fn no(_: &str) -> bool {
        false
    }

    fn json_of(path: &Path) -> Value {
        serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
    }

    fn backups(d: &Dirs) -> Vec<PathBuf> {
        let mut found: Vec<PathBuf> = fs::read_dir(d.settings.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.to_string_lossy().contains(".usage-cockpit-backup-"))
            .collect();
        found.sort();
        found
    }

    #[test]
    fn req_023_the_plan_mentions_a_backup_only_when_there_is_a_file() {
        let d = dirs();
        let mut plan = String::new();
        let mut keep = |text: &str| {
            plan = text.to_owned();
            false
        };
        // no settings file yet
        assert_eq!(
            setup(&d.settings, &d.exe, &d.state, &mut keep).unwrap(),
            SetupOutcome::Declined
        );
        assert!(
            plan.contains("does not exist yet and will be created"),
            "{plan}"
        );
        assert!(
            !plan.contains("A backup of the file is made first"),
            "{plan}"
        );
        // with a settings file
        put_settings(&d, r#"{"a":1}"#);
        let mut plan = String::new();
        let mut keep = |text: &str| {
            plan = text.to_owned();
            false
        };
        setup(&d.settings, &d.exe, &d.state, &mut keep).unwrap();
        assert!(
            plan.contains("A backup of the file is made first."),
            "{plan}"
        );
        assert!(!plan.contains("does not exist yet"), "{plan}");
    }

    #[test]
    fn req_023_setup_on_empty_settings() {
        let d = dirs();
        let outcome = setup(&d.settings, &d.exe, &d.state, &mut yes).unwrap();
        assert_eq!(outcome, SetupOutcome::Changed { backup: None });
        let expected = format!(
            "{}/bin/usage-cockpit bridge",
            unix(d.exe.parent().unwrap().parent())
        );
        let settings = json_of(&d.settings);
        assert_eq!(
            settings,
            json!({"statusLine": {"type": "command", "command": expected}})
        );
        assert_eq!(
            json_of(&d.state),
            json!({"v": 1, "previous_status_line": null})
        );
    }

    fn unix(path: Option<&Path>) -> String {
        path.unwrap().to_string_lossy().replace('\\', "/")
    }

    #[test]
    fn req_023_setup_keeps_existing_status_line() {
        let d = dirs();
        put_settings(
            &d,
            r#"{"statusLine":{"type":"command","command":"ccusage","padding":2}}"#,
        );
        let outcome = setup(&d.settings, &d.exe, &d.state, &mut yes).unwrap();
        assert!(matches!(outcome, SetupOutcome::Changed { backup: Some(_) }));
        assert_eq!(
            json_of(&d.state),
            json!({"v": 1, "previous_status_line":
                {"type": "command", "command": "ccusage", "padding": 2}})
        );
        let new = json_of(&d.settings);
        assert!(
            new["statusLine"]["command"]
                .as_str()
                .unwrap()
                .ends_with("usage-cockpit bridge")
        );
        assert!(new["statusLine"].get("padding").is_none());
    }

    #[test]
    fn req_023_setup_preserves_key_order() {
        let d = dirs();
        put_settings(
            &d,
            r#"{"zeta":1,"statusLine":{"type":"command","command":"x"},"alpha":{"b":1,"a":2},"mid":[3]}"#,
        );
        setup(&d.settings, &d.exe, &d.state, &mut yes).unwrap();
        let text = fs::read_to_string(&d.settings).unwrap();
        let position = |key: &str| text.find(&format!("\"{key}\"")).unwrap();
        assert!(position("zeta") < position("statusLine"));
        assert!(position("statusLine") < position("alpha"));
        assert!(position("alpha") < position("mid"));
        assert!(position("b") < position("a"));
        assert_eq!(json_of(&d.settings)["mid"], json!([3]));
    }

    #[test]
    fn req_023_setup_appends_the_key_when_there_was_none() {
        let d = dirs();
        put_settings(&d, r#"{"zeta":1,"alpha":2}"#);
        setup(&d.settings, &d.exe, &d.state, &mut yes).unwrap();
        let text = fs::read_to_string(&d.settings).unwrap();
        assert!(text.find("zeta").unwrap() < text.find("alpha").unwrap());
        assert!(text.find("alpha").unwrap() < text.find("statusLine").unwrap());
    }

    #[test]
    fn req_023_decline_leaves_file_identical() {
        let d = dirs();
        let original =
            "{ \"statusLine\": {\"type\":\"command\",\"command\":\"x\"},\n  \"a\": 1 }\n";
        put_settings(&d, original);
        assert_eq!(
            setup(&d.settings, &d.exe, &d.state, &mut no).unwrap(),
            SetupOutcome::Declined
        );
        assert_eq!(fs::read(&d.settings).unwrap(), original.as_bytes());
        assert!(!d.state.exists());
        assert!(backups(&d).is_empty());
        let stored = format!("{} bridge", unix(Some(&d.exe)));
        put_settings(
            &d,
            &format!(r#"{{"statusLine":{{"type":"command","command":"{stored}"}}}}"#),
        );
        let bridged = fs::read(&d.settings).unwrap();
        assert_eq!(
            remove(&d.settings, &d.state, &mut no).unwrap(),
            RemoveOutcome::Declined
        );
        assert_eq!(fs::read(&d.settings).unwrap(), bridged);
    }

    #[test]
    fn req_023_remove_restores_previous() {
        let d = dirs();
        put_settings(
            &d,
            r#"{"a":1,"statusLine":{"type":"command","command":"ccusage"},"b":2}"#,
        );
        setup(&d.settings, &d.exe, &d.state, &mut yes).unwrap();
        let outcome = remove(&d.settings, &d.state, &mut yes).unwrap();
        assert!(matches!(outcome, RemoveOutcome::Removed { .. }));
        assert_eq!(
            json_of(&d.settings),
            json!({"a": 1, "statusLine": {"type": "command", "command": "ccusage"}, "b": 2})
        );
        assert_eq!(
            json_of(&d.state),
            json!({"v": 1, "previous_status_line": null})
        );
    }

    #[test]
    fn req_023_remove_deletes_the_key_when_there_was_none() {
        let d = dirs();
        put_settings(&d, r#"{"a":1,"b":2}"#);
        setup(&d.settings, &d.exe, &d.state, &mut yes).unwrap();
        remove(&d.settings, &d.state, &mut yes).unwrap();
        assert_eq!(json_of(&d.settings), json!({"a": 1, "b": 2}));
    }

    #[test]
    fn req_023_remove_does_nothing_for_a_foreign_status_line() {
        let d = dirs();
        let original = r#"{"statusLine":{"type":"command","command":"ccusage"}}"#;
        put_settings(&d, original);
        let mut asked = false;
        let outcome = remove(&d.settings, &d.state, &mut |_| {
            asked = true;
            true
        })
        .unwrap();
        assert_eq!(outcome, RemoveOutcome::NotTheBridge);
        assert!(!asked);
        assert_eq!(fs::read_to_string(&d.settings).unwrap(), original);
        assert!(backups(&d).is_empty());
        fs::remove_file(&d.settings).unwrap();
        assert_eq!(
            remove(&d.settings, &d.state, &mut yes).unwrap(),
            RemoveOutcome::NotTheBridge
        );
    }

    #[cfg(windows)]
    #[test]
    fn req_117_path_with_space_is_accepted_on_windows() {
        let d = dirs();
        put_settings(&d, r#"{"a":1}"#);
        let folder = d.exe.parent().unwrap().join("with space");
        fs::create_dir_all(&folder).unwrap();
        let exe = folder.join("usage-cockpit.exe");
        fs::write(&exe, b"").unwrap();
        assert_eq!(
            setup(&d.settings, &exe, &d.state, &mut yes).unwrap(),
            SetupOutcome::Changed {
                backup: Some(backups(&d).remove(0))
            }
        );
        let command = json_of(&d.settings)["statusLine"]["command"]
            .as_str()
            .unwrap()
            .to_owned();
        let program = command.strip_suffix(" bridge").expect(&command);
        // either the short folder name without a space or the quoted long name
        assert!(
            !program.contains(' ') || program.starts_with('"'),
            "{command}"
        );
        assert!(
            program.to_lowercase().contains("usage-cockpit.exe"),
            "{command}"
        );
        assert!(is_bridge(&json!({"type": "command", "command": command})));
        // the same file again is "already set up", and remove puts the settings back
        assert_eq!(
            setup(&d.settings, &exe, &d.state, &mut yes).unwrap(),
            SetupOutcome::AlreadySetUp
        );
        remove(&d.settings, &d.state, &mut yes).unwrap();
        assert_eq!(json_of(&d.settings), json!({"a": 1}));
    }

    #[cfg(windows)]
    #[test]
    fn req_117_without_a_short_name_the_path_is_quoted() {
        let command = windows_command("C:/My Tools/usage-cockpit.exe", None);
        assert_eq!(command, "\"C:/My Tools/usage-cockpit.exe\" bridge");
        assert!(is_bridge(&json!({"type": "command", "command": command})));
        let short = windows_command(
            "C:/My Tools/usage-cockpit.exe",
            Some("C:/MYTOOL~1/usage-cockpit.exe".into()),
        );
        assert_eq!(short, "C:/MYTOOL~1/usage-cockpit.exe bridge");
    }

    #[test]
    fn req_117_quoted_command_is_recognised_as_the_bridge() {
        let line = |command: &str| json!({"type": "command", "command": command});
        assert!(is_bridge(&line("\"C:/My Tools/usage-cockpit.exe\" bridge")));
        assert!(is_bridge(&line("\"/opt/my tools/usage-cockpit\" bridge")));
        assert!(!is_bridge(&line("\"C:/My Tools/usage-cockpit.exe bridge")));
        assert!(!is_bridge(&line("\"C:/My Tools/other.exe\" bridge")));
        assert!(!is_bridge(&line(
            "\"C:/My Tools/usage-cockpit.exe\" bridge x"
        )));
        assert!(!is_bridge(&line("\"C:/My Tools/usage-cockpit.exe\"")));
        assert!(!is_bridge(&line("\"C:/My Tools/usage-cockpit.exe\"bridge")));
        assert!(is_bridge(&line("  /opt/tools/usage-cockpit bridge")));
    }

    #[cfg(not(windows))]
    #[test]
    fn req_023_path_with_space_refused() {
        let d = dirs();
        put_settings(&d, r#"{"a":1}"#);
        let exe = d
            .exe
            .parent()
            .unwrap()
            .join("with space")
            .join("usage-cockpit");
        let error = setup(&d.settings, &exe, &d.state, &mut yes).unwrap_err();
        assert!(matches!(error, SetupError::PathWithSpace(_)), "{error}");
        assert!(error.to_string().contains("without spaces"));
        assert_eq!(fs::read_to_string(&d.settings).unwrap(), r#"{"a":1}"#);
        assert!(!d.state.exists());
    }

    #[test]
    fn req_023_backup_created() {
        let d = dirs();
        let original = r#"{"keep":"me"}"#;
        put_settings(&d, original);
        let SetupOutcome::Changed {
            backup: Some(backup),
        } = setup(&d.settings, &d.exe, &d.state, &mut yes).unwrap()
        else {
            panic!("expected a backup");
        };
        let name = backup.file_name().unwrap().to_string_lossy().into_owned();
        let stamp = name
            .strip_prefix("settings.json.usage-cockpit-backup-")
            .unwrap();
        assert_eq!(stamp.len(), "20261007-131500".len(), "{name}");
        assert_eq!(stamp.as_bytes()[8], b'-');
        assert!(
            stamp
                .bytes()
                .enumerate()
                .all(|(i, b)| i == 8 || b.is_ascii_digit())
        );
        assert_eq!(fs::read_to_string(&backup).unwrap(), original);
    }

    #[test]
    fn req_023_two_backups_in_one_second_do_not_overwrite_each_other() {
        let d = dirs();
        put_settings(&d, r#"{"a":1}"#);
        setup(&d.settings, &d.exe, &d.state, &mut yes).unwrap();
        remove(&d.settings, &d.state, &mut yes).unwrap();
        setup(&d.settings, &d.exe, &d.state, &mut yes).unwrap();
        assert_eq!(backups(&d).len(), 3);
    }

    #[test]
    fn req_023_setup_twice_is_a_no_op() {
        let d = dirs();
        setup(&d.settings, &d.exe, &d.state, &mut yes).unwrap();
        let before = fs::read(&d.settings).unwrap();
        assert_eq!(
            setup(&d.settings, &d.exe, &d.state, &mut no).unwrap(),
            SetupOutcome::AlreadySetUp
        );
        assert_eq!(fs::read(&d.settings).unwrap(), before);
        assert!(backups(&d).is_empty());
    }

    #[test]
    fn req_023_a_moved_bridge_does_not_become_the_previous_status_line() {
        let d = dirs();
        put_settings(
            &d,
            r#"{"statusLine":{"type":"command","command":"ccusage"}}"#,
        );
        setup(&d.settings, &d.exe, &d.state, &mut yes).unwrap();
        let moved = d
            .exe
            .parent()
            .unwrap()
            .join("moved")
            .join("usage-cockpit.exe");
        setup(&d.settings, &moved, &d.state, &mut yes).unwrap();
        assert_eq!(
            json_of(&d.state)["previous_status_line"]["command"],
            json!("ccusage")
        );
        assert!(
            json_of(&d.settings)["statusLine"]["command"]
                .as_str()
                .unwrap()
                .contains("moved")
        );
    }

    #[test]
    fn req_023_unusable_settings_are_not_touched() {
        let d = dirs();
        for text in ["{ nope", "[1,2]", "\"text\""] {
            put_settings(&d, text);
            assert!(
                setup(&d.settings, &d.exe, &d.state, &mut yes).is_err(),
                "{text}"
            );
            assert!(remove(&d.settings, &d.state, &mut yes).is_err(), "{text}");
            assert_eq!(fs::read_to_string(&d.settings).unwrap(), text);
            assert!(!d.state.exists());
        }
    }

    #[test]
    fn req_023_remove_refuses_a_corrupt_state_file() {
        let d = dirs();
        put_settings(
            &d,
            r#"{"statusLine":{"type":"command","command":"ccusage"}}"#,
        );
        setup(&d.settings, &d.exe, &d.state, &mut yes).unwrap();
        fs::write(&d.state, "{ corrupt").unwrap();
        let before = fs::read(&d.settings).unwrap();
        let error = remove(&d.settings, &d.state, &mut yes).unwrap_err();
        assert!(matches!(error, SetupError::State { .. }), "{error}");
        assert_eq!(fs::read(&d.settings).unwrap(), before);
        assert_eq!(fs::read_to_string(&d.state).unwrap(), "{ corrupt");
    }

    #[test]
    fn req_023_a_byte_order_mark_is_accepted() {
        let d = dirs();
        put_settings(&d, "\u{feff}{\"a\":1}");
        setup(&d.settings, &d.exe, &d.state, &mut yes).unwrap();
        assert_eq!(json_of(&d.settings)["a"], json!(1));
    }

    #[test]
    fn req_023_is_bridge_recognises_only_the_bridge_command() {
        let line = |command: &str| json!({"type": "command", "command": command});
        assert!(is_bridge(&line("C:/bin/usage-cockpit.exe bridge")));
        assert!(is_bridge(&line("/opt/tools/usage-cockpit bridge")));
        assert!(is_bridge(&line("C:\\bin\\Usage-Cockpit.EXE bridge")));
        assert!(!is_bridge(&line("/opt/tools/usage-cockpit")));
        assert!(!is_bridge(&line("/opt/tools/usage-cockpit bridge --extra")));
        assert!(!is_bridge(&line("ccusage bridge")));
        assert!(!is_bridge(
            &json!({"type": "text", "command": "usage-cockpit bridge"})
        ));
        assert!(!is_bridge(&json!("usage-cockpit bridge")));
    }
}
