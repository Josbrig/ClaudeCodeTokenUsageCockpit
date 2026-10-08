// SPDX-License-Identifier: Apache-2.0

use std::fs;
use std::path::PathBuf;

use cockpit_core::settings::{Settings, StartView, WindowSettings, load, save};

fn settings_path(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("settings.toml")
}

fn custom() -> Settings {
    Settings {
        tolerance_pp: 7.5,
        stale_after_s: 900,
        rate_period_s: 600,
        always_on_top: false,
        start_view: StartView::Detailed,
        window: WindowSettings {
            x: -1200.0,
            y: 40.5,
            width: 520.0,
            height: 640.0,
        },
    }
}

#[test]
fn req_024_defaults_when_missing() {
    let dir = tempfile::tempdir().unwrap();
    let s = load(&settings_path(&dir));
    assert_eq!(s, Settings::default());
    assert_eq!(s.tolerance_pp, 5.0);
    assert_eq!(s.stale_after_s, 600);
    assert_eq!(s.rate_period_s, 1_800);
    assert!(s.always_on_top);
    assert_eq!(s.start_view, StartView::Compact);
    assert_eq!(s.window.width, 320.0);
    assert_eq!(s.window.height, 120.0);
}

#[test]
fn req_024_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = settings_path(&dir);
    save(&path, &custom()).unwrap();
    assert_eq!(load(&path), custom());
}

#[test]
fn req_115_saved_file_carries_version_1() {
    let dir = tempfile::tempdir().unwrap();
    let path = settings_path(&dir);
    save(&path, &Settings::default()).unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert!(
        text.lines().next().unwrap().trim() == "version = 1",
        "{text}"
    );
    assert!(text.contains("[window]"), "{text}");
}

#[test]
fn req_024_save_creates_the_directory_and_leaves_no_temp_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config").join("settings.toml");
    save(&path, &Settings::default()).unwrap();
    assert!(path.is_file());
    let leftovers: Vec<_> = fs::read_dir(path.parent().unwrap())
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty());
}

#[test]
fn req_024_invalid_file_falls_back_and_is_renamed() {
    let dir = tempfile::tempdir().unwrap();
    let path = settings_path(&dir);
    fs::write(&path, "tolerance_pp = [this is not toml").unwrap();
    assert_eq!(load(&path), Settings::default());
    assert!(!path.exists(), "the invalid file must be moved away");
    let moved = dir.path().join("settings.toml.invalid");
    assert_eq!(
        fs::read_to_string(moved).unwrap(),
        "tolerance_pp = [this is not toml"
    );
}

#[test]
fn req_024_file_that_is_not_utf8_falls_back_and_is_renamed() {
    let dir = tempfile::tempdir().unwrap();
    let path = settings_path(&dir);
    fs::write(&path, [0xff, 0xfe, 0x00, 0x41]).unwrap();
    assert_eq!(load(&path), Settings::default());
    assert!(dir.path().join("settings.toml.invalid").exists());
}

#[test]
fn req_024_out_of_range_value_uses_default() {
    let dir = tempfile::tempdir().unwrap();
    let path = settings_path(&dir);
    fs::write(
        &path,
        "tolerance_pp = 99.0\nstale_after_s = 5\nrate_period_s = 100000\nalways_on_top = false\n",
    )
    .unwrap();
    let s = load(&path);
    // The three out-of-range values fall back, the valid one is kept.
    assert_eq!(s.tolerance_pp, 5.0);
    assert_eq!(s.stale_after_s, 600);
    assert_eq!(s.rate_period_s, 1_800);
    assert!(!s.always_on_top);
    // An out-of-range value does not make the file invalid.
    assert!(path.exists());
}

#[test]
fn req_024_negative_tolerance_uses_default() {
    let dir = tempfile::tempdir().unwrap();
    let path = settings_path(&dir);
    fs::write(&path, "tolerance_pp = -1.0\n").unwrap();
    assert_eq!(load(&path).tolerance_pp, 5.0);
}

#[test]
fn req_024_limits_of_the_ranges_are_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let path = settings_path(&dir);
    fs::write(
        &path,
        "tolerance_pp = 0.0\nstale_after_s = 86400\nrate_period_s = 300\n",
    )
    .unwrap();
    let s = load(&path);
    assert_eq!(s.tolerance_pp, 0.0);
    assert_eq!(s.stale_after_s, 86_400);
    assert_eq!(s.rate_period_s, 300);
}

#[test]
fn req_024_wrong_type_uses_default() {
    let dir = tempfile::tempdir().unwrap();
    let path = settings_path(&dir);
    fs::write(
        &path,
        "always_on_top = \"yes\"\nstale_after_s = 12.5\nstart_view = 3\ntolerance_pp = \"big\"\n",
    )
    .unwrap();
    assert_eq!(load(&path), Settings::default());
}

#[test]
fn req_024_integer_is_accepted_for_a_float_value() {
    let dir = tempfile::tempdir().unwrap();
    let path = settings_path(&dir);
    fs::write(&path, "tolerance_pp = 8\n").unwrap();
    assert_eq!(load(&path).tolerance_pp, 8.0);
}

#[test]
fn req_115_file_without_version_is_read_as_version_1() {
    let dir = tempfile::tempdir().unwrap();
    let path = settings_path(&dir);
    fs::write(&path, "tolerance_pp = 7.0\nstart_view = \"detailed\"\n").unwrap();
    let s = load(&path);
    assert_eq!(s.tolerance_pp, 7.0);
    assert_eq!(s.start_view, StartView::Detailed);
}

#[test]
fn req_115_unknown_keys_and_newer_versions_are_tolerated() {
    let dir = tempfile::tempdir().unwrap();
    let path = settings_path(&dir);
    fs::write(
        &path,
        "version = 2\nfuture_option = true\ntolerance_pp = 6.0\n",
    )
    .unwrap();
    let s = load(&path);
    assert_eq!(s.tolerance_pp, 6.0);
    assert!(path.exists());
}

#[test]
fn req_024_invalid_start_view_uses_default() {
    let dir = tempfile::tempdir().unwrap();
    let path = settings_path(&dir);
    fs::write(&path, "start_view = \"huge\"\n").unwrap();
    assert_eq!(load(&path).start_view, StartView::Compact);
}

#[test]
fn req_018_window_values_are_read_and_checked_one_by_one() {
    let dir = tempfile::tempdir().unwrap();
    let path = settings_path(&dir);
    fs::write(
        &path,
        "[window]\nx = -300\ny = 12.5\nwidth = 10.0\nheight = 700.0\n",
    )
    .unwrap();
    let w = load(&path).window;
    assert_eq!(w.x, -300.0, "negative positions are valid");
    assert_eq!(w.y, 12.5);
    assert_eq!(w.width, 320.0, "a width below the minimum falls back");
    assert_eq!(w.height, 700.0);
}

#[test]
fn req_018_window_of_the_wrong_type_uses_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let path = settings_path(&dir);
    fs::write(&path, "window = 5\n").unwrap();
    assert_eq!(load(&path).window, WindowSettings::default());
}

#[test]
fn req_024_utf8_byte_order_mark_is_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let path = settings_path(&dir);
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(b"tolerance_pp = 9.0\n");
    fs::write(&path, bytes).unwrap();
    assert_eq!(load(&path).tolerance_pp, 9.0);
    assert!(
        path.exists(),
        "a file with a byte order mark is not invalid"
    );
}
