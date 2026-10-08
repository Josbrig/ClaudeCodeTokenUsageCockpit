// SPDX-License-Identifier: Apache-2.0
//! The start entries of Linux and macOS, which are files (REQ-118).
//!
//! - **Linux:** a desktop entry `usage-cockpit.desktop` in `$XDG_CONFIG_HOME/autostart` (default
//!   `~/.config/autostart`), which every common desktop starts at sign-in. A desktop that lets the
//!   person switch an entry off writes `Hidden=true` (or `X-GNOME-Autostart-enabled=false`) into it;
//!   that is reported as "switched off".
//! - **macOS:** a LaunchAgent property list `io.github.josbrig.usage-cockpit.plist` in
//!   `~/Library/LaunchAgents`, which launchd loads at the next login. Nothing is started or
//!   stopped when the entry is written or removed (a `launchctl bootstrap` would start a second
//!   window at once): the entry only counts from the next login.
//!
//! All of it is plain text and plain files with the folder passed in, so it is compiled and tested on
//! every system, also on Windows, with temporary folders. The thin part that chooses the folder
//! from the environment is in `autostart.rs`.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use cockpit_core::store::write_atomic;

use crate::autostart::State;

/// Which kind of file the system uses. Each system uses one of the two (the other is compiled for
/// the tests and for the sake of one shared code path).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Kind {
    /// An XDG desktop entry (Linux).
    Desktop,
    /// A LaunchAgent property list (macOS).
    Plist,
}

/// The reverse-domain prefix of the label of the LaunchAgent.
const PLIST_PREFIX: &str = "io.github.josbrig";

// ---------------------------------------------------------------- folders and file names

/// `$XDG_CONFIG_HOME/autostart` if that is set to an absolute path, else `$HOME/.config/autostart`.
#[allow(dead_code)] // used on Linux only
pub fn xdg_autostart_dir(
    xdg_config_home: Option<OsString>,
    home: Option<OsString>,
) -> Option<PathBuf> {
    let xdg = xdg_config_home
        .map(PathBuf::from)
        .filter(|path| path.is_absolute());
    match xdg {
        Some(dir) => Some(dir.join("autostart")),
        None => home
            .filter(|home| !home.is_empty())
            .map(|home| PathBuf::from(home).join(".config").join("autostart")),
    }
}

/// `$HOME/Library/LaunchAgents`.
#[allow(dead_code)] // used on macOS only
pub fn launch_agents_dir(home: Option<OsString>) -> Option<PathBuf> {
    home.filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join("Library").join("LaunchAgents"))
}

/// The file of the entry `name` in `dir`.
pub fn entry_file(kind: Kind, dir: &Path, name: &str) -> PathBuf {
    match kind {
        Kind::Desktop => dir.join(format!("{name}.desktop")),
        Kind::Plist => dir.join(format!("{}.plist", plist_label(name))),
    }
}

/// The label of the LaunchAgent of the entry `name`.
pub fn plist_label(name: &str) -> String {
    format!("{PLIST_PREFIX}.{name}")
}

// ---------------------------------------------------------------- desktop entry (Linux)

/// The `Exec` value for the program at `exe`, as the Desktop Entry Specification wants it: a path
/// with a space or another reserved character in double quotes (`"`, `` ` ``, `$` and `\` escaped
/// with a backslash inside), a `%` doubled, and a backslash doubled once more because the file's
/// own escaping reads `\\` as one backslash.
pub fn desktop_exec(exe: &Path) -> String {
    let path = exe.to_string_lossy();
    let reserved = " \t\n\"'\\><~|&;$*?#()`";
    let exec_level = if path.chars().any(|c| reserved.contains(c)) {
        let mut quoted = String::from("\"");
        for c in path.chars() {
            match c {
                '"' | '`' | '$' | '\\' => {
                    quoted.push('\\');
                    quoted.push(c);
                }
                '%' => quoted.push_str("%%"),
                other => quoted.push(other),
            }
        }
        quoted.push('"');
        quoted
    } else {
        path.replace('%', "%%")
    };
    // a newline must not end the line of the file: it is written as the escape `\n`
    exec_level.replace('\\', "\\\\").replace('\n', "\\n")
}

/// The text of the desktop entry file.
pub fn desktop_entry(exe: &Path) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=usage-cockpit\nComment=Usage cockpit for Claude Code\nExec={}\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
        desktop_exec(exe)
    )
}

/// How the desktop entry `contents` (`None`: no file) compares with this program.
pub fn classify_desktop(contents: Option<&str>, exe: &Path) -> State {
    let Some(contents) = contents else {
        return State::Off;
    };
    // a byte order mark in front of the first line would hide the group
    let contents = contents.trim_start_matches('\u{feff}');
    let mut in_group = false;
    let (mut exec, mut hidden, mut disabled) = (None, false, false);
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_group = line == "[Desktop Entry]";
        } else if in_group && let Some((key, value)) = line.split_once('=') {
            match key.trim() {
                "Exec" => exec = Some(value.trim().to_owned()),
                "Hidden" => hidden = value.trim() == "true",
                "X-GNOME-Autostart-enabled" => disabled = value.trim() == "false",
                _ => {}
            }
        }
    }
    if hidden || disabled {
        return State::SwitchedOff;
    }
    match exec {
        Some(found) if found == desktop_exec(exe) => State::On,
        Some(found) => State::Stale { found },
        None => State::Stale {
            found: "(no Exec line)".to_owned(),
        },
    }
}

// ---------------------------------------------------------------- property list (macOS)

/// `text` for the content of an XML element.
pub fn xml_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            other => out.push(other),
        }
    }
    out
}

fn xml_unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// The text of the LaunchAgent property list.
pub fn plist_text(label: &str, exe: &Path) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n  <key>Label</key>\n  <string>{}</string>\n  <key>ProgramArguments</key>\n  <array>\n    <string>{}</string>\n  </array>\n  <key>RunAtLoad</key>\n  <true/>\n</dict>\n</plist>\n",
        xml_escape(label),
        xml_escape(&exe.to_string_lossy())
    )
}

/// How the property list `contents` (`None`: no file) compares with this program.
pub fn classify_plist(contents: Option<&str>, exe: &Path) -> State {
    let Some(contents) = contents else {
        return State::Off;
    };
    let found = contents
        .split_once("<key>ProgramArguments</key>")
        .and_then(|(_, after)| after.split_once("<string>"))
        .and_then(|(_, after)| after.split_once("</string>"))
        .map(|(value, _)| xml_unescape(value.trim()));
    match found {
        Some(found) if found == exe.to_string_lossy() => State::On,
        Some(found) => State::Stale { found },
        None => State::Stale {
            found: "(no program)".to_owned(),
        },
    }
}

// ---------------------------------------------------------------- the files

fn read_text(path: &Path) -> Result<Option<String>, String> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("cannot read {}: {error}", path.display())),
    }
}

/// A path that cannot be written into a start entry as it is: not valid text, or with a control
/// character (the entry would point at another path or be invalid).
fn check_path(exe: &Path) -> Result<(), String> {
    match exe.to_str() {
        Some(text) if !text.chars().any(char::is_control) => Ok(()),
        _ => Err(format!(
            "the path of the program ({}) has a character that cannot be written into a start entry; move the program to a folder with an ordinary name",
            exe.display()
        )),
    }
}

/// The real state of the entry `name` in `dir`.
pub fn state_in(kind: Kind, dir: &Path, name: &str, exe: &Path) -> Result<State, String> {
    check_path(exe)?;
    let contents = read_text(&entry_file(kind, dir, name))?;
    Ok(match kind {
        Kind::Desktop => classify_desktop(contents.as_deref(), exe),
        Kind::Plist => classify_plist(contents.as_deref(), exe),
    })
}

/// Writes (`on`) or removes the entry `name` in `dir` and returns the state afterwards. Writing
/// replaces a file that was switched off or that starts another program; removing a missing file
/// is not an error.
pub fn set_in(kind: Kind, dir: &Path, name: &str, exe: &Path, on: bool) -> Result<State, String> {
    check_path(exe)?;
    let path = entry_file(kind, dir, name);
    if on {
        let text = match kind {
            Kind::Desktop => desktop_entry(exe),
            Kind::Plist => plist_text(&plist_label(name), exe),
        };
        fs::create_dir_all(dir)
            .and_then(|()| write_atomic(&path, text.as_bytes()))
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    } else {
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("cannot remove {}: {error}", path.display())),
        }
    }
    state_in(kind, dir, name, exe)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exe() -> PathBuf {
        PathBuf::from("/opt/tools/usage-cockpit")
    }

    #[test]
    fn req_118_the_autostart_folders() {
        let home = Some(OsString::from("/home-dir"));
        assert_eq!(
            xdg_autostart_dir(None, home.clone()),
            Some(PathBuf::from("/home-dir").join(".config").join("autostart"))
        );
        // an absolute path of the system the test runs on (`/cfg` is not absolute on Windows)
        let absolute = std::env::temp_dir();
        assert_eq!(
            xdg_autostart_dir(Some(absolute.clone().into_os_string()), home.clone()),
            Some(absolute.join("autostart"))
        );
        // a relative or empty XDG_CONFIG_HOME is ignored, as the XDG specification says
        assert_eq!(
            xdg_autostart_dir(Some("relative".into()), home.clone()),
            Some(PathBuf::from("/home-dir").join(".config").join("autostart"))
        );
        assert_eq!(
            xdg_autostart_dir(Some("".into()), home.clone()),
            Some(PathBuf::from("/home-dir").join(".config").join("autostart"))
        );
        assert_eq!(xdg_autostart_dir(None, None), None);
        assert_eq!(xdg_autostart_dir(None, Some("".into())), None);
        assert_eq!(
            launch_agents_dir(home),
            Some(
                PathBuf::from("/home-dir")
                    .join("Library")
                    .join("LaunchAgents")
            )
        );
        assert_eq!(launch_agents_dir(None), None);
    }

    #[test]
    fn req_118_the_names_of_the_files() {
        let dir = Path::new("/d");
        assert_eq!(
            entry_file(Kind::Desktop, dir, "usage-cockpit"),
            dir.join("usage-cockpit.desktop")
        );
        assert_eq!(
            entry_file(Kind::Plist, dir, "usage-cockpit"),
            dir.join("io.github.josbrig.usage-cockpit.plist")
        );
    }

    #[test]
    fn req_118_exec_is_quoted_as_the_desktop_entry_specification_says() {
        assert_eq!(
            desktop_exec(Path::new("/opt/tools/usage-cockpit")),
            "/opt/tools/usage-cockpit"
        );
        assert_eq!(
            desktop_exec(Path::new("/opt/my tools/usage-cockpit")),
            "\"/opt/my tools/usage-cockpit\""
        );
        // a double quote and a dollar sign: escaped at the quoting level, and the backslash of
        // that escape doubled for the file
        assert_eq!(
            desktop_exec(Path::new("/opt/a\"b$c/x")),
            "\"/opt/a\\\\\"b\\\\$c/x\""
        );
        // a backslash in the path: four in the file
        assert_eq!(
            desktop_exec(Path::new("/opt/a\\b/x")),
            "\"/opt/a\\\\\\\\b/x\""
        );
        // a newline in a path is written as the escape `\n`, not as a line break
        assert!(!desktop_exec(Path::new("/opt/a\nb/x")).contains('\n'));
        // a percent sign is doubled (it is a field code in `Exec`)
        assert_eq!(desktop_exec(Path::new("/opt/100%/x")), "/opt/100%%/x");
    }

    #[test]
    fn req_118_the_desktop_entry_has_the_needed_keys() {
        let text = desktop_entry(&exe());
        for line in [
            "[Desktop Entry]",
            "Type=Application",
            "Name=usage-cockpit",
            "Exec=/opt/tools/usage-cockpit",
            "Terminal=false",
            "X-GNOME-Autostart-enabled=true",
        ] {
            assert!(text.lines().any(|l| l == line), "{line} in {text}");
        }
        assert!(text.ends_with('\n'));
    }

    #[test]
    fn req_118_the_state_of_a_desktop_entry() {
        let exe = exe();
        assert_eq!(classify_desktop(None, &exe), State::Off);
        let mine = desktop_entry(&exe);
        assert_eq!(classify_desktop(Some(&mine), &exe), State::On);
        assert_eq!(
            classify_desktop(Some(&desktop_entry(Path::new("/old/usage-cockpit"))), &exe),
            State::Stale {
                found: "/old/usage-cockpit".to_owned()
            }
        );
        // switched off by the desktop
        let hidden = format!("{mine}Hidden=true\n");
        assert_eq!(classify_desktop(Some(&hidden), &exe), State::SwitchedOff);
        let disabled = mine.replace("Autostart-enabled=true", "Autostart-enabled=false");
        assert_ne!(disabled, mine, "the key was found");
        assert_eq!(classify_desktop(Some(&disabled), &exe), State::SwitchedOff);
        // `Hidden=false` is not off, and a key of another group does not count
        let not_hidden = format!("{mine}Hidden=false\n");
        assert_eq!(classify_desktop(Some(&not_hidden), &exe), State::On);
        let other_group = format!("{mine}[Desktop Action x]\nHidden=true\n");
        assert_eq!(classify_desktop(Some(&other_group), &exe), State::On);
        assert_eq!(
            classify_desktop(Some("[Desktop Entry]\nName=x\n"), &exe),
            State::Stale {
                found: "(no Exec line)".to_owned()
            }
        );
    }

    #[test]
    fn req_118_the_property_list_is_valid_text_and_escaped() {
        let text = plist_text(
            "io.github.josbrig.usage-cockpit",
            Path::new("/Apps/A&B <x>/usage-cockpit"),
        );
        assert!(text.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"));
        assert!(
            text.contains("<key>Label</key>\n  <string>io.github.josbrig.usage-cockpit</string>")
        );
        assert!(text.contains("<string>/Apps/A&amp;B &lt;x&gt;/usage-cockpit</string>"));
        assert!(text.contains("<key>RunAtLoad</key>\n  <true/>"));
        assert!(text.trim_end().ends_with("</plist>"));
        assert_eq!(xml_escape("a'b\"c"), "a&apos;b&quot;c");
        assert_eq!(xml_unescape(&xml_escape("<&'\">x")), "<&'\">x");
    }

    #[test]
    fn req_118_the_state_of_a_property_list() {
        let exe = Path::new("/Apps/My Tools/usage-cockpit");
        assert_eq!(classify_plist(None, exe), State::Off);
        let mine = plist_text("l", exe);
        assert_eq!(classify_plist(Some(&mine), exe), State::On);
        assert_eq!(
            classify_plist(Some(&plist_text("l", Path::new("/old/usage-cockpit"))), exe),
            State::Stale {
                found: "/old/usage-cockpit".to_owned()
            }
        );
        assert_eq!(
            classify_plist(Some("<plist><dict></dict></plist>"), exe),
            State::Stale {
                found: "(no program)".to_owned()
            }
        );
    }

    #[test]
    fn req_118_switching_on_and_off_with_real_files() {
        for kind in [Kind::Desktop, Kind::Plist] {
            let root = tempfile::tempdir().unwrap();
            // the folder does not exist yet: switching on creates it
            let dir = root.path().join("autostart");
            let exe = exe();
            assert_eq!(
                state_in(kind, &dir, "usage-cockpit", &exe).unwrap(),
                State::Off
            );
            assert_eq!(
                set_in(kind, &dir, "usage-cockpit", &exe, true).unwrap(),
                State::On
            );
            assert!(entry_file(kind, &dir, "usage-cockpit").is_file());

            // another file at the same name is reported, and a second switch-on corrects it
            let other = PathBuf::from("/other/usage-cockpit");
            assert!(matches!(
                state_in(kind, &dir, "usage-cockpit", &other).unwrap(),
                State::Stale { .. }
            ));
            assert_eq!(
                set_in(kind, &dir, "usage-cockpit", &other, true).unwrap(),
                State::On
            );

            assert_eq!(
                set_in(kind, &dir, "usage-cockpit", &other, false).unwrap(),
                State::Off
            );
            assert!(!entry_file(kind, &dir, "usage-cockpit").exists());
            // off twice is fine
            assert_eq!(
                set_in(kind, &dir, "usage-cockpit", &other, false).unwrap(),
                State::Off
            );
        }
    }

    #[test]
    fn req_118_a_desktop_that_switched_the_entry_off_is_reported_and_switching_on_clears_it() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path();
        let exe = exe();
        set_in(Kind::Desktop, dir, "usage-cockpit", &exe, true).unwrap();
        let file = entry_file(Kind::Desktop, dir, "usage-cockpit");
        let mut text = fs::read_to_string(&file).unwrap();
        text.push_str("Hidden=true\n");
        fs::write(&file, text).unwrap();
        assert_eq!(
            state_in(Kind::Desktop, dir, "usage-cockpit", &exe).unwrap(),
            State::SwitchedOff
        );
        assert_eq!(
            set_in(Kind::Desktop, dir, "usage-cockpit", &exe, true).unwrap(),
            State::On
        );
    }

    #[test]
    fn req_118_a_path_that_cannot_be_written_is_refused() {
        let root = tempfile::tempdir().unwrap();
        for bad in ["/opt/a\nb/usage-cockpit", "/opt/a\u{1}b/usage-cockpit"] {
            for kind in [Kind::Desktop, Kind::Plist] {
                let error =
                    set_in(kind, root.path(), "usage-cockpit", Path::new(bad), true).unwrap_err();
                assert!(
                    error.contains("cannot be written into a start entry"),
                    "{error}"
                );
                assert!(state_in(kind, root.path(), "usage-cockpit", Path::new(bad)).is_err());
            }
        }
        // nothing was written
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }

    #[test]
    fn req_118_a_desktop_entry_with_a_byte_order_mark_is_read() {
        let exe = exe();
        let text = format!("{}{}", '\u{feff}', desktop_entry(&exe));
        assert_eq!(classify_desktop(Some(&text), &exe), State::On);
    }

    #[test]
    fn req_118_an_unreadable_entry_is_an_error_not_off() {
        let root = tempfile::tempdir().unwrap();
        // a folder where the file should be
        let file = entry_file(Kind::Desktop, root.path(), "usage-cockpit");
        fs::create_dir_all(&file).unwrap();
        assert!(state_in(Kind::Desktop, root.path(), "usage-cockpit", &exe()).is_err());
    }
}
