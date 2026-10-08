// SPDX-License-Identifier: Apache-2.0
//! Runs the kept user status line command (concept §5.5) through the shell Claude Code would use.
//!
//! Unix: `sh -c`. Windows: Git for Windows bash if one is found, else Windows PowerShell. Std has
//! no wait-with-timeout, so the child is polled and the pipes are served from threads.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Variable that may point to the bash Claude Code uses on Windows.
pub const GIT_BASH_VAR: &str = "CLAUDE_CODE_GIT_BASH_PATH";
/// Output of the kept command beyond this size is cut off.
const MAX_OUTPUT_BYTES: u64 = 1024 * 1024;
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Runs `command` with `input` on its standard input.
///
/// Returns the standard output unchanged if the command exited with 0 within `timeout` and
/// printed something other than whitespace; `None` in every other case (the caller then prints
/// its own text). The bytes are not required to be UTF-8.
pub fn run_kept_command(command: &str, input: &[u8], timeout: Duration) -> Option<Vec<u8>> {
    let deadline = Instant::now() + timeout;
    let mut child = match shell_command(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            log::warn!("bridge: cannot start the kept status line command: {error}");
            return None;
        }
    };
    // On Windows the whole process tree dies with the job when it is dropped.
    let _job = tree_guard::enclose(&child);
    // Both pipes are served from threads so that a command which does not read its input, or
    // writes a lot, cannot block the polling loop. The threads are never joined: a grandchild
    // may keep a pipe open after the child is gone.
    if let Some(mut stdin) = child.stdin.take() {
        let input = input.to_vec();
        thread::spawn(move || {
            // A broken pipe only means the command did not want the input.
            let _ = stdin.write_all(&input);
        });
    }
    let (sender, receiver) = mpsc::channel();
    if let Some(stdout) = child.stdout.take() {
        thread::spawn(move || {
            let mut buffer = Vec::new();
            let _ = stdout.take(MAX_OUTPUT_BYTES).read_to_end(&mut buffer);
            let _ = sender.send(buffer);
        });
    }
    if !wait_for_success(&mut child, deadline) {
        return None;
    }
    let remaining = deadline.saturating_duration_since(Instant::now());
    let output = receiver.recv_timeout(remaining).ok()?;
    if output.trim_ascii().is_empty() {
        log::info!("bridge: the kept status line command printed nothing");
        return None;
    }
    Some(output)
}

/// `true` if the child exited with 0 before the deadline; otherwise the child is killed if it
/// still runs.
fn wait_for_success(child: &mut Child, deadline: Instant) -> bool {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    log::info!("bridge: the kept status line command failed: {status}");
                }
                return status.success();
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(POLL_INTERVAL),
            Ok(None) => {
                log::warn!("bridge: the kept status line command timed out");
                break;
            }
            Err(error) => {
                log::warn!("bridge: cannot wait for the kept status line command: {error}");
                break;
            }
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    false
}

/// Kills the grandchildren of a timed-out command too (Windows only).
///
/// Without it a `sleep` started by bash keeps the inherited output handles of the bridge open
/// after bash was killed, and Claude Code would wait for the end of the output.
#[cfg(windows)]
mod tree_guard {
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };

    /// Closing the handle ends every process still in the job.
    pub struct Job(HANDLE);

    impl Drop for Job {
        fn drop(&mut self) {
            // SAFETY: the handle was created by `CreateJobObjectW` and is closed only here.
            unsafe { CloseHandle(self.0) };
        }
    }

    /// Puts `child` into a new job that kills its members when dropped; `None` if that fails
    /// (the command then runs without the guard).
    pub fn enclose(child: &Child) -> Option<Job> {
        // SAFETY: plain Win32 calls; the structure is zeroed, which is a valid initial value, and
        // the handles are valid for the duration of the calls.
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return None;
            }
            let job = Job(handle);
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let set = SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                std::ptr::from_ref(&info).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            if set == 0 || AssignProcessToJobObject(handle, child.as_raw_handle() as HANDLE) == 0 {
                return None;
            }
            Some(job)
        }
    }
}

#[cfg(not(windows))]
mod tree_guard {
    use std::process::Child;

    pub fn enclose(_child: &Child) -> Option<()> {
        None
    }
}

#[cfg(not(windows))]
fn shell_command(command: &str) -> Command {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(command);
    cmd
}

#[cfg(windows)]
fn shell_command(command: &str) -> Command {
    use std::os::windows::process::CommandExt;
    // No console window may flash up: the bridge is a Windows GUI program.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut cmd = match find_git_bash() {
        Some(bash) => {
            let mut cmd = Command::new(bash);
            cmd.arg("-c");
            cmd
        }
        None => {
            let mut cmd = Command::new("powershell");
            cmd.arg("-NoProfile").arg("-Command");
            cmd
        }
    };
    cmd.arg(command).creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Finds Git for Windows bash in the order of concept §5.5.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn find_git_bash() -> Option<PathBuf> {
    find_git_bash_with(&|name| std::env::var_os(name))
}

/// [`find_git_bash`] with the environment given.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn find_git_bash_with(env: &dyn Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let system32 = env("SystemRoot")
        .filter(|root| !root.is_empty())
        .map(|root| clean(&Path::new(&root).join("System32")));
    let usable = |candidate: PathBuf| -> Option<PathBuf> {
        let candidate = clean(&candidate);
        let wsl = system32
            .as_ref()
            .is_some_and(|dir| is_below(&candidate, dir));
        (candidate.is_file() && !wsl).then_some(candidate)
    };
    let from_var = |name: &str, tail: &str| -> Option<PathBuf> {
        let base = env(name).filter(|value| !value.is_empty())?;
        usable(Path::new(&base).join(tail))
    };
    if let Some(found) = env(GIT_BASH_VAR)
        .filter(|path| !path.is_empty())
        .and_then(|path| usable(PathBuf::from(path)))
    {
        return Some(found);
    }
    let git_bash = Path::new("Git").join("bin").join("bash.exe");
    let git_bash = git_bash.to_string_lossy();
    if let Some(found) = from_var("ProgramFiles", &git_bash)
        .or_else(|| from_var("ProgramFiles(x86)", &git_bash))
        .or_else(|| {
            let tail = Path::new("Programs").join(&*git_bash);
            from_var("LOCALAPPDATA", &tail.to_string_lossy())
        })
    {
        return Some(found);
    }
    // `Git\cmd\git.exe` and `Git\mingw64\bin\git.exe` are one and two levels below `Git`.
    let path = env("PATH")?;
    std::env::split_paths(&path)
        .filter(|dir| dir.join("git.exe").is_file())
        .find_map(|dir| {
            usable(dir.join("..").join("bin").join("bash.exe"))
                .or_else(|| usable(dir.join("..").join("..").join("bin").join("bash.exe")))
        })
}

/// Resolves `.` and `..` without touching the file system.
fn clean(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !result.pop() {
                    result.push(component);
                }
            }
            other => result.push(other),
        }
    }
    result
}

/// `true` if `path` lies below `dir`; the comparison ignores case and the kind of separator.
fn is_below(path: &Path, dir: &Path) -> bool {
    let normal = |p: &Path| p.to_string_lossy().replace('/', "\\").to_lowercase();
    let dir = normal(dir);
    normal(path).starts_with(&format!("{}\\", dir.trim_end_matches('\\')))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[cfg(unix)]
    const SECOND: Duration = Duration::from_secs(1);

    #[cfg(unix)]
    #[test]
    fn req_012_run_returns_the_output_unchanged() {
        let out = run_kept_command("printf 'a  b\\n\\n'", b"", SECOND).unwrap();
        assert_eq!(out, b"a  b\n\n");
    }

    #[cfg(unix)]
    #[test]
    fn req_012_run_passes_the_input_on() {
        assert_eq!(run_kept_command("cat", b"hello", SECOND).unwrap(), b"hello");
    }

    #[cfg(unix)]
    #[test]
    fn req_012_run_keeps_output_that_is_not_utf8() {
        let out = run_kept_command("printf '\\377\\376x'", b"", SECOND).unwrap();
        assert_eq!(out, [0xff, 0xfe, b'x']);
    }

    #[cfg(unix)]
    #[test]
    fn req_012_run_rejects_failure_and_whitespace_only_output() {
        assert_eq!(run_kept_command("echo out; exit 3", b"", SECOND), None);
        assert_eq!(run_kept_command("true", b"", SECOND), None);
        assert_eq!(run_kept_command("printf ' \\n\\t'", b"", SECOND), None);
    }

    #[cfg(unix)]
    #[test]
    fn req_109_run_kills_a_command_that_takes_too_long() {
        let started = Instant::now();
        let out = run_kept_command("sleep 5", b"", Duration::from_millis(200));
        assert_eq!(out, None);
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"").unwrap();
    }

    fn env_of<'a>(vars: &'a [(&'a str, &'a Path)]) -> impl Fn(&str) -> Option<OsString> + use<'a> {
        move |name| {
            vars.iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .map(|(_, value)| value.as_os_str().to_owned())
        }
    }

    #[test]
    fn req_012_git_bash_prefers_the_variable_and_skips_a_missing_file() {
        let root = tempfile::tempdir().unwrap();
        let pf = root.path().join("pf");
        let bash = pf.join("Git").join("bin").join("bash.exe");
        touch(&bash);
        let custom = root.path().join("custom").join("bash.exe");
        let missing = root.path().join("nothing").join("bash.exe");

        touch(&custom);
        let vars = [(GIT_BASH_VAR, custom.as_path()), ("ProgramFiles", &pf)];
        assert_eq!(find_git_bash_with(&env_of(&vars)), Some(custom.clone()));

        let vars = [(GIT_BASH_VAR, missing.as_path()), ("ProgramFiles", &pf)];
        assert_eq!(find_git_bash_with(&env_of(&vars)), Some(bash));
    }

    #[test]
    fn req_012_git_bash_search_order_of_the_standard_folders() {
        let root = tempfile::tempdir().unwrap();
        let (pf, x86, local) = (
            root.path().join("pf"),
            root.path().join("x86"),
            root.path().join("local"),
        );
        let vars = [
            ("ProgramFiles", pf.as_path()),
            ("ProgramFiles(x86)", &x86),
            ("LOCALAPPDATA", &local),
        ];
        assert_eq!(find_git_bash_with(&env_of(&vars)), None);

        let in_local = local
            .join("Programs")
            .join("Git")
            .join("bin")
            .join("bash.exe");
        touch(&in_local);
        assert_eq!(find_git_bash_with(&env_of(&vars)), Some(in_local));

        let in_x86 = x86.join("Git").join("bin").join("bash.exe");
        touch(&in_x86);
        assert_eq!(find_git_bash_with(&env_of(&vars)), Some(in_x86));

        let in_pf = pf.join("Git").join("bin").join("bash.exe");
        touch(&in_pf);
        assert_eq!(find_git_bash_with(&env_of(&vars)), Some(in_pf));
    }

    #[test]
    fn req_012_git_bash_is_found_next_to_git_exe_on_the_path() {
        let root = tempfile::tempdir().unwrap();
        let git = root.path().join("Git");
        let bash = git.join("bin").join("bash.exe");
        touch(&bash);
        for (dir, expected) in [("cmd", &bash), ("mingw64/bin", &bash)] {
            let dir = git.join(dir);
            touch(&dir.join("git.exe"));
            let path = std::env::join_paths([root.path().join("other"), dir]).unwrap();
            let found = find_git_bash_with(&|name| (name == "PATH").then(|| path.clone()));
            assert_eq!(found.as_deref(), Some(clean(expected).as_path()));
        }
    }

    #[test]
    fn req_012_git_bash_never_comes_from_system32() {
        let root = tempfile::tempdir().unwrap();
        let windows = root.path().join("Windows");
        let wsl = windows.join("System32").join("bash.exe");
        touch(&wsl);
        // The variable spelled with other case and a `..` still leads into System32.
        let sneaky = windows
            .join("system32")
            .join("..")
            .join("SYSTEM32")
            .join("bash.exe");
        let vars = [("SystemRoot", windows.as_path()), (GIT_BASH_VAR, &sneaky)];
        assert_eq!(find_git_bash_with(&env_of(&vars)), None);
        let vars = [("SystemRoot", windows.as_path()), (GIT_BASH_VAR, &wsl)];
        assert_eq!(find_git_bash_with(&env_of(&vars)), None);
    }

    #[test]
    fn req_012_is_below_ignores_case_and_separators() {
        let dir = Path::new("C:\\Windows\\System32");
        assert!(is_below(Path::new("c:/windows/SYSTEM32/bash.exe"), dir));
        assert!(!is_below(
            Path::new("C:\\Windows\\System32x\\bash.exe"),
            dir
        ));
        assert!(!is_below(Path::new("C:\\Windows\\System32"), dir));
    }
}
