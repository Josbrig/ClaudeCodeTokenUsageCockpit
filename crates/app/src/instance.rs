// SPDX-License-Identifier: Apache-2.0
//! Only one cockpit per user at a time (concept §11.6, REQ-033): the running cockpit holds an
//! exclusive lock on `cockpit.lock` in the data folder.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::path::Path;

/// Name of the lock file in the data folder.
pub const LOCK_FILE: &str = "cockpit.lock";

/// Another cockpit holds the lock.
#[derive(Debug, PartialEq, Eq)]
pub struct AlreadyRunning;

/// Holds the lock while it lives; the lock ends with the guard or with the process.
#[derive(Debug)]
pub struct InstanceGuard {
    /// `None` if the lock file could not be used: the cockpit then runs without protection
    /// instead of not running at all.
    file: Option<File>,
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        if let Some(file) = &self.file {
            let _ = file.unlock();
        }
    }
}

/// Takes the lock in `data_dir`.
///
/// Fails with [`AlreadyRunning`] if another cockpit holds it. If the lock file cannot be
/// created or locked for another reason, that is logged and the guard holds nothing: a cockpit
/// that cannot show its window is worse than two cockpits.
pub fn acquire(data_dir: &Path) -> Result<InstanceGuard, AlreadyRunning> {
    let opened = fs::create_dir_all(data_dir).and_then(|()| {
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(data_dir.join(LOCK_FILE))
    });
    let file = match opened {
        Ok(file) => file,
        Err(error) => {
            log::warn!("cannot open {LOCK_FILE}: {error}; starting without the lock");
            return Ok(InstanceGuard { file: None });
        }
    };
    match file.try_lock() {
        Ok(()) => Ok(InstanceGuard { file: Some(file) }),
        Err(TryLockError::WouldBlock) => Err(AlreadyRunning),
        Err(TryLockError::Error(error)) => {
            log::warn!("cannot lock {LOCK_FILE}: {error}; starting without the lock");
            Ok(InstanceGuard { file: None })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_033_second_acquire_fails_while_first_held() {
        let dir = tempfile::tempdir().unwrap();
        let first = acquire(dir.path()).unwrap();
        assert!(first.file.is_some());
        assert_eq!(acquire(dir.path()).unwrap_err(), AlreadyRunning);
        // The failed attempt does not take the lock away from the first one.
        assert_eq!(acquire(dir.path()).unwrap_err(), AlreadyRunning);
        drop(first);
    }

    #[test]
    fn req_033_acquire_after_release_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        drop(acquire(dir.path()).unwrap());
        let again = acquire(dir.path()).unwrap();
        assert!(again.file.is_some());
    }

    #[test]
    fn req_033_the_data_folder_is_created() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("a").join("b");
        let guard = acquire(&nested).unwrap();
        assert!(guard.file.is_some());
        assert!(nested.join(LOCK_FILE).is_file());
    }

    #[test]
    fn req_033_a_lock_file_that_cannot_be_opened_does_not_stop_the_cockpit() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a-file");
        fs::write(&file, b"x").unwrap();
        // The data folder would have to lie below a regular file.
        let guard = acquire(&file.join("data")).unwrap();
        assert!(guard.file.is_none());
    }

    #[test]
    fn req_033_two_folders_do_not_disturb_each_other() {
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let _first = acquire(a.path()).unwrap();
        assert!(acquire(b.path()).is_ok());
    }
}
