// SPDX-License-Identifier: Apache-2.0

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use cockpit_core::paths::{
    CLAUDE_DIR_VAR, HOME_OVERRIDE_VAR, claude_dir_with, config_dir_with, data_dir_with, ensure_dir,
};

fn env_with(name: &'static str, value: &'static str) -> impl Fn(&str) -> Option<OsString> {
    move |n| (n == name).then(|| OsString::from(value))
}

fn no_env(_: &str) -> Option<OsString> {
    None
}

fn has_component(path: &Path, name: &str) -> bool {
    path.components().any(|c| c.as_os_str() == name)
}

#[test]
fn req_020_paths_home_override() {
    let env = env_with(HOME_OVERRIDE_VAR, "somewhere");
    assert_eq!(
        data_dir_with(&env).unwrap(),
        PathBuf::from("somewhere").join("data")
    );
    assert_eq!(
        config_dir_with(&env).unwrap(),
        PathBuf::from("somewhere").join("config")
    );
}

#[test]
fn req_020_empty_override_is_ignored() {
    let env = env_with(HOME_OVERRIDE_VAR, "");
    assert!(has_component(
        &data_dir_with(&env).unwrap(),
        "usage-cockpit"
    ));
}

#[test]
fn req_020_default_dirs_contain_app_name() {
    // Without the override both directories live below a folder named after the application
    // (on Windows the data directory is `...\usage-cockpit\data`, so the name is not the last
    // component). This checks the `ProjectDirs` call on every platform.
    let data = data_dir_with(&no_env).unwrap();
    let config = config_dir_with(&no_env).unwrap();
    assert!(has_component(&data, "usage-cockpit"), "{data:?}");
    assert!(has_component(&config, "usage-cockpit"), "{config:?}");
}

#[test]
fn req_020_ensure_dir_creates_missing_parents() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("a").join("b").join("c");
    ensure_dir(&target).unwrap();
    assert!(target.is_dir());
    // Calling it again on an existing directory is fine.
    ensure_dir(&target).unwrap();
}

#[test]
fn req_020_ensure_dir_reports_the_path_on_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("file");
    std::fs::write(&file, b"x").unwrap();
    let err = ensure_dir(&file.join("sub")).unwrap_err();
    assert!(err.to_string().contains("file"), "{err}");
}

#[test]
fn req_023_claude_dir_from_env() {
    let env = env_with(CLAUDE_DIR_VAR, "custom-claude");
    let got = claude_dir_with(&env, Some(Path::new("home")));
    assert_eq!(got, Some(PathBuf::from("custom-claude")));
}

#[test]
fn req_023_claude_dir_default() {
    let got = claude_dir_with(&no_env, Some(Path::new("home")));
    assert_eq!(got, Some(PathBuf::from("home").join(".claude")));
}

#[test]
fn req_023_claude_dir_unknown_without_home() {
    assert_eq!(claude_dir_with(&no_env, None), None);
}
