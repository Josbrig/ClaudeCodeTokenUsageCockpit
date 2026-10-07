// SPDX-License-Identifier: Apache-2.0
//! Start with the user's sign-in (REQ-118).
//!
//! Windows: one value below `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, so that no
//! administrator rights are needed. The state shown to the person is the real one: Windows keeps
//! "switched off in the Startup apps list" in a second place (`...\Explorer\StartupApproved\Run`),
//! and that is read as well. Other systems follow in their own issues; there the module reports
//! that the switch is not available.

use std::path::Path;

/// Name of the value in the `Run` key.
#[cfg_attr(not(windows), allow(dead_code))]
pub const VALUE_NAME: &str = "UsageCockpit";

/// What the system has for the cockpit.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum State {
    /// No start entry.
    Off,
    /// An entry for this executable.
    On,
    /// An entry that starts another file (the program was moved, or a copy was set up); `found`
    /// is the command stored.
    Stale { found: String },
    /// An entry exists, but the person switched it off in the Windows list of startup apps.
    SwitchedOffInWindows,
    /// This system has no such switch yet.
    #[cfg_attr(windows, allow(dead_code))]
    Unsupported,
}

/// The command stored in the `Run` value: the path in double quotes, as Windows expects it for a
/// path with spaces. A `\\?\` prefix, which `current_exe` may return, is dropped.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn run_value(exe: &Path) -> String {
    let path = exe.to_string_lossy();
    let path = path.strip_prefix(r"\\?\").unwrap_or(&path);
    format!("\"{path}\"")
}

/// `true` if the `StartupApproved` bytes say "switched off" (the lowest bit of the first byte;
/// `02` and `06` are enabled, `03` and `07` disabled).
#[cfg_attr(not(windows), allow(dead_code))]
fn approved_says_off(bytes: &[u8]) -> bool {
    bytes.first().is_some_and(|first| first & 1 == 1)
}

/// How the stored command compares with this executable. Windows paths do not differ by case.
#[cfg_attr(not(windows), allow(dead_code))]
fn classify(stored: Option<&str>, approved: Option<&[u8]>, exe: &Path) -> State {
    let Some(stored) = stored else {
        return State::Off;
    };
    if approved.is_some_and(approved_says_off) {
        return State::SwitchedOffInWindows;
    }
    if stored.trim().eq_ignore_ascii_case(&run_value(exe)) {
        State::On
    } else {
        State::Stale {
            found: stored.to_owned(),
        }
    }
}

/// The real state for `exe`.
pub fn state(exe: &Path) -> Result<State, String> {
    imp::state(imp::VALUE, exe)
}

/// Switches the start entry on (for `exe`) or off and returns the state afterwards.
pub fn set(exe: &Path, on: bool) -> Result<State, String> {
    imp::set(imp::VALUE, exe, on)
}

#[cfg(windows)]
mod imp {
    use std::path::Path;

    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ, RegCloseKey,
        RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    };

    use super::{State, VALUE_NAME, classify, run_value};

    pub const VALUE: &str = VALUE_NAME;
    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const APPROVED_KEY: &str =
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain([0]).collect()
    }

    /// An open registry key, closed when dropped.
    struct Key(HKEY);

    impl Key {
        /// `None` if the key does not exist.
        fn open(path: &str, access: u32) -> Result<Option<Key>, String> {
            let mut handle: HKEY = std::ptr::null_mut();
            // SAFETY: `path` is NUL-terminated and `handle` is a valid place for the result.
            let code = unsafe {
                RegOpenKeyExW(
                    HKEY_CURRENT_USER,
                    wide(path).as_ptr(),
                    0,
                    access,
                    &mut handle,
                )
            };
            match code {
                ERROR_SUCCESS => Ok(Some(Key(handle))),
                ERROR_FILE_NOT_FOUND => Ok(None),
                other => Err(format!(
                    "cannot open the registry key {path} (error {other})"
                )),
            }
        }

        /// The raw bytes of a value, `None` if there is none.
        fn read(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
            let name_wide = wide(name);
            let mut size: u32 = 0;
            // SAFETY: a null data pointer only asks for the size.
            let code = unsafe {
                RegQueryValueExW(
                    self.0,
                    name_wide.as_ptr(),
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    &mut size,
                )
            };
            match code {
                ERROR_SUCCESS => {}
                ERROR_FILE_NOT_FOUND => return Ok(None),
                other => return Err(format!("cannot read {name} (error {other})")),
            }
            let mut data = vec![0u8; size as usize];
            // SAFETY: `data` holds `size` bytes, as announced to the call.
            let code = unsafe {
                RegQueryValueExW(
                    self.0,
                    name_wide.as_ptr(),
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    data.as_mut_ptr(),
                    &mut size,
                )
            };
            if code != ERROR_SUCCESS {
                return Err(format!("cannot read {name} (error {code})"));
            }
            data.truncate(size as usize);
            Ok(Some(data))
        }

        fn read_string(&self, name: &str) -> Result<Option<String>, String> {
            Ok(self.read(name)?.map(|bytes| {
                let units: Vec<u16> = bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| u16::from_le_bytes(*pair))
                    .take_while(|unit| *unit != 0)
                    .collect();
                String::from_utf16_lossy(&units)
            }))
        }

        fn write_string(&self, name: &str, text: &str) -> Result<(), String> {
            let data: Vec<u8> = wide(text)
                .iter()
                .flat_map(|unit| unit.to_le_bytes())
                .collect();
            // SAFETY: `data` holds `data.len()` bytes and the name is NUL-terminated.
            let code = unsafe {
                RegSetValueExW(
                    self.0,
                    wide(name).as_ptr(),
                    0,
                    REG_SZ,
                    data.as_ptr(),
                    data.len() as u32,
                )
            };
            if code == ERROR_SUCCESS {
                Ok(())
            } else {
                Err(format!("cannot write {name} (error {code})"))
            }
        }

        /// Removes a value; a value that is not there is fine.
        fn delete(&self, name: &str) -> Result<(), String> {
            // SAFETY: the name is NUL-terminated.
            let code = unsafe { RegDeleteValueW(self.0, wide(name).as_ptr()) };
            match code {
                ERROR_SUCCESS | ERROR_FILE_NOT_FOUND => Ok(()),
                other => Err(format!("cannot remove {name} (error {other})")),
            }
        }
    }

    impl Drop for Key {
        fn drop(&mut self) {
            // SAFETY: the handle was opened by `RegOpenKeyExW` and is closed only here.
            unsafe { RegCloseKey(self.0) };
        }
    }

    pub fn state(name: &str, exe: &Path) -> Result<State, String> {
        let stored = match Key::open(RUN_KEY, KEY_QUERY_VALUE)? {
            Some(key) => key.read_string(name)?,
            None => None,
        };
        let approved = match Key::open(APPROVED_KEY, KEY_QUERY_VALUE)? {
            Some(key) => key.read(name)?,
            None => None,
        };
        Ok(classify(stored.as_deref(), approved.as_deref(), exe))
    }

    pub fn set(name: &str, exe: &Path, on: bool) -> Result<State, String> {
        let run = Key::open(RUN_KEY, KEY_SET_VALUE)?
            .ok_or_else(|| format!("the registry key {RUN_KEY} does not exist"))?;
        if on {
            run.write_string(name, &run_value(exe))?;
        } else {
            run.delete(name)?;
        }
        // A value left from "switched off" in the Windows list would keep the entry off. An
        // absent value means enabled.
        if let Some(approved) = Key::open(APPROVED_KEY, KEY_SET_VALUE)? {
            approved.delete(name)?;
        }
        state(name, exe)
    }

    #[cfg(test)]
    pub fn raw_approved(name: &str, bytes: &[u8]) -> Result<(), String> {
        use windows_sys::Win32::System::Registry::{REG_BINARY, RegCreateKeyExW};
        let mut handle: HKEY = std::ptr::null_mut();
        // SAFETY: valid NUL-terminated name, valid out pointers.
        let code = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                wide(APPROVED_KEY).as_ptr(),
                0,
                std::ptr::null(),
                0,
                KEY_SET_VALUE,
                std::ptr::null(),
                &mut handle,
                std::ptr::null_mut(),
            )
        };
        if code != ERROR_SUCCESS {
            return Err(format!("error {code}"));
        }
        let key = Key(handle);
        // SAFETY: `bytes` holds `bytes.len()` bytes.
        let code = unsafe {
            RegSetValueExW(
                key.0,
                wide(name).as_ptr(),
                0,
                REG_BINARY,
                bytes.as_ptr(),
                bytes.len() as u32,
            )
        };
        if code == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(format!("error {code}"))
        }
    }

    #[cfg(test)]
    pub fn raw_run(name: &str) -> Result<Option<String>, String> {
        match Key::open(RUN_KEY, KEY_QUERY_VALUE)? {
            Some(key) => key.read_string(name),
            None => Ok(None),
        }
    }

    #[cfg(test)]
    pub fn forget(name: &str) {
        for path in [RUN_KEY, APPROVED_KEY] {
            if let Ok(Some(key)) = Key::open(path, KEY_SET_VALUE) {
                let _ = key.delete(name);
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use std::path::Path;

    use super::State;

    pub const VALUE: &str = "";

    pub fn state(_name: &str, _exe: &Path) -> Result<State, String> {
        Ok(State::Unsupported)
    }

    pub fn set(_name: &str, _exe: &Path, _on: bool) -> Result<State, String> {
        Err("starting with the system is not available on this system yet".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn exe() -> PathBuf {
        PathBuf::from(r"C:\Tools\My Tools\usage-cockpit.exe")
    }

    #[test]
    fn req_118_the_value_is_the_quoted_path() {
        assert_eq!(
            run_value(&exe()),
            r#""C:\Tools\My Tools\usage-cockpit.exe""#
        );
        assert_eq!(
            run_value(Path::new(r"\\?\C:\bin\usage-cockpit.exe")),
            r#""C:\bin\usage-cockpit.exe""#
        );
    }

    #[test]
    fn req_118_the_state_follows_the_entry_and_the_windows_list() {
        let mine = run_value(&exe());
        assert_eq!(classify(None, None, &exe()), State::Off);
        assert_eq!(classify(Some(&mine), None, &exe()), State::On);
        assert_eq!(
            classify(Some(&mine.to_uppercase()), Some(&[2, 0, 0, 0]), &exe()),
            State::On,
            "case does not matter, 02 means enabled"
        );
        assert_eq!(
            classify(Some(&mine), Some(&[6, 0, 0, 0]), &exe()),
            State::On
        );
        assert_eq!(
            classify(Some(&mine), Some(&[3, 0, 0, 0]), &exe()),
            State::SwitchedOffInWindows
        );
        assert_eq!(
            classify(Some(&mine), Some(&[7, 0, 0, 0]), &exe()),
            State::SwitchedOffInWindows
        );
        assert_eq!(
            classify(Some(r#""D:\old\usage-cockpit.exe""#), None, &exe()),
            State::Stale {
                found: r#""D:\old\usage-cockpit.exe""#.to_owned()
            }
        );
        assert_eq!(classify(Some(&mine), Some(&[]), &exe()), State::On);
    }

    /// Removes the test value from the registry when the test ends, also after a failure.
    #[cfg(windows)]
    struct Cleanup(String);

    #[cfg(windows)]
    impl Drop for Cleanup {
        fn drop(&mut self) {
            imp::forget(&self.0);
        }
    }

    /// The real registry, with a value name that only this test uses.
    #[cfg(windows)]
    #[test]
    fn req_118_switching_on_and_off_in_the_real_registry() {
        let name = format!("UsageCockpitTest{}", std::process::id());
        let _cleanup = Cleanup(name.clone());
        let exe = exe();
        assert_eq!(imp::state(&name, &exe).unwrap(), State::Off);

        assert_eq!(imp::set(&name, &exe, true).unwrap(), State::On);
        assert_eq!(imp::raw_run(&name).unwrap(), Some(run_value(&exe)));

        // another file at the same name is reported, and a second switch-on corrects it
        let other = PathBuf::from(r"C:\Other\usage-cockpit.exe");
        assert_eq!(
            imp::state(&name, &other).unwrap(),
            State::Stale {
                found: run_value(&exe)
            }
        );
        assert_eq!(imp::set(&name, &other, true).unwrap(), State::On);

        // "switched off" in the Windows list is reported, and switching on clears it
        imp::raw_approved(&name, &[3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).unwrap();
        assert_eq!(
            imp::state(&name, &other).unwrap(),
            State::SwitchedOffInWindows
        );
        assert_eq!(imp::set(&name, &other, true).unwrap(), State::On);

        assert_eq!(imp::set(&name, &other, false).unwrap(), State::Off);
        assert_eq!(imp::raw_run(&name).unwrap(), None);
        // off twice is fine
        assert_eq!(imp::set(&name, &other, false).unwrap(), State::Off);
    }

    #[cfg(not(windows))]
    #[test]
    fn req_118_other_systems_report_that_the_switch_is_missing() {
        assert_eq!(state(&exe()).unwrap(), State::Unsupported);
        assert!(set(&exe(), true).is_err());
    }
}
