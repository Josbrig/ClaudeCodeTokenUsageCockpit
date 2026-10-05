// SPDX-License-Identifier: Apache-2.0
//! Per-user data and configuration directories (concept §5.1) and the Claude Code
//! settings directory (concept §5.4).
//!
//! The environment is passed in as a function (`*_with`) so that callers and tests need no
//! global state; the plain functions use the real process environment.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use directories::{BaseDirs, ProjectDirs};

/// Overrides both directories: data in `<home>/data`, config in `<home>/config`.
pub const HOME_OVERRIDE_VAR: &str = "USAGE_COCKPIT_HOME";
/// Overrides the Claude Code settings directory.
pub const CLAUDE_DIR_VAR: &str = "CLAUDE_CONFIG_DIR";

/// Errors while locating or creating directories.
#[derive(Debug, thiserror::Error)]
pub enum PathsError {
    /// The operating system gave no home directory.
    #[error("cannot determine the user's home directory")]
    NoHome,
    /// A directory could not be created.
    #[error("cannot create directory {path}: {source}")]
    Create {
        /// The directory that could not be created.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },
}

fn non_empty(value: Option<OsString>) -> Option<OsString> {
    value.filter(|v| !v.is_empty())
}

fn project_dirs() -> Result<ProjectDirs, PathsError> {
    ProjectDirs::from("", "", "usage-cockpit").ok_or(PathsError::NoHome)
}

fn real_env(name: &str) -> Option<OsString> {
    std::env::var_os(name)
}

/// Data directory (`latest.json`, history, logs), with the environment given.
pub fn data_dir_with(env: &dyn Fn(&str) -> Option<OsString>) -> Result<PathBuf, PathsError> {
    if let Some(home) = non_empty(env(HOME_OVERRIDE_VAR)) {
        return Ok(PathBuf::from(home).join("data"));
    }
    Ok(project_dirs()?.data_local_dir().to_path_buf())
}

/// Configuration directory (`settings.toml`, `bridge-state.json`), with the environment given.
pub fn config_dir_with(env: &dyn Fn(&str) -> Option<OsString>) -> Result<PathBuf, PathsError> {
    if let Some(home) = non_empty(env(HOME_OVERRIDE_VAR)) {
        return Ok(PathBuf::from(home).join("config"));
    }
    Ok(project_dirs()?.config_dir().to_path_buf())
}

/// Data directory of the current user.
pub fn data_dir() -> Result<PathBuf, PathsError> {
    data_dir_with(&real_env)
}

/// Configuration directory of the current user.
pub fn config_dir() -> Result<PathBuf, PathsError> {
    config_dir_with(&real_env)
}

/// Creates the directory and any missing parents.
pub fn ensure_dir(path: &Path) -> Result<(), PathsError> {
    fs::create_dir_all(path).map_err(|source| PathsError::Create {
        path: path.to_path_buf(),
        source,
    })
}

/// Claude Code settings directory: `CLAUDE_CONFIG_DIR` if set, else `<home>/.claude`.
///
/// `home` is the user's home directory; `None` means unknown.
pub fn claude_dir_with(
    env: &dyn Fn(&str) -> Option<OsString>,
    home: Option<&Path>,
) -> Option<PathBuf> {
    if let Some(dir) = non_empty(env(CLAUDE_DIR_VAR)) {
        return Some(PathBuf::from(dir));
    }
    home.map(|h| h.join(".claude"))
}

/// Claude Code settings directory of the current user.
pub fn claude_dir() -> Option<PathBuf> {
    let base = BaseDirs::new();
    claude_dir_with(&real_env, base.as_ref().map(|b| b.home_dir()))
}
