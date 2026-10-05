// SPDX-License-Identifier: Apache-2.0
//! Console output of a program built for the Windows GUI subsystem.
//!
//! The executable has no console of its own on Windows, so that opening it by double click
//! shows no black window. Commands that print for a person (`--version`, `--help`, errors,
//! `setup-bridge`, `remove-bridge`) attach to the console of the program that started them. The
//! bridge never does: Claude Code reads its standard output through a pipe.

/// Attaches to the parent process's console, if there is one, and points standard output and
/// standard error at it unless they are already redirected. Does nothing on other systems.
#[cfg(windows)]
pub fn attach_to_parent() {
    use std::fs::OpenOptions;
    use std::os::windows::io::IntoRawHandle;

    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Console::{
        ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE,
        SetStdHandle,
    };

    // SAFETY: plain Win32 calls with valid constants; the raw handle comes from a file this
    // function opened and is handed over to the process's standard handles on purpose.
    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            return; // no parent console (started by double click or through a pipe)
        }
        for id in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            let current = GetStdHandle(id);
            if !current.is_null() && current != INVALID_HANDLE_VALUE {
                continue; // already redirected to a pipe or file: keep it
            }
            if let Ok(console) = OpenOptions::new().write(true).open("CONOUT$") {
                SetStdHandle(id, console.into_raw_handle());
            }
        }
    }
}

/// Does nothing on systems other than Windows.
#[cfg(not(windows))]
pub fn attach_to_parent() {}
