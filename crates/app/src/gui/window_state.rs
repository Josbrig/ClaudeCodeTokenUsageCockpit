// SPDX-License-Identifier: Apache-2.0
//! Position, size and level of the window (concept §9, §11.2): what is saved to the settings,
//! when, and how it is restored. Everything here is plain functions on values, so it can be
//! tested without a window.

use std::time::{Duration, Instant};

use cockpit_core::settings::{
    MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH, Settings, StartView, WindowSettings,
};
use eframe::egui::{Rect, WindowLevel};

/// A changed position or size is saved when it has stayed the same for this long.
pub const SAVE_AFTER: Duration = Duration::from_secs(1);
/// Positions beyond this distance from the origin are not trusted. A window that is minimised
/// is reported far outside the screen, and a saved position like that would hide the window.
const MAX_COORDINATE: f64 = 16_000.0;
/// Largest size that is restored.
const MAX_SIZE: f64 = 8_000.0;

/// Position (outer left top) and size (inner) from what the window system reports. `None` while
/// the window is minimised, when a value is missing, or when it is not a usable number.
pub fn geometry_from(
    outer: Option<Rect>,
    inner: Option<Rect>,
    minimized: bool,
) -> Option<WindowSettings> {
    if minimized {
        return None;
    }
    let (outer, inner) = (outer?, inner?);
    let geometry = WindowSettings {
        x: f64::from(outer.min.x),
        y: f64::from(outer.min.y),
        width: f64::from(inner.width()),
        height: f64::from(inner.height()),
    };
    let numbers = [geometry.x, geometry.y, geometry.width, geometry.height];
    let usable = numbers.iter().all(|n| n.is_finite())
        && geometry.x.abs() <= MAX_COORDINATE
        && geometry.y.abs() <= MAX_COORDINATE
        && geometry.width >= MIN_WINDOW_WIDTH
        && geometry.height >= MIN_WINDOW_HEIGHT;
    usable.then_some(geometry)
}

/// The position to restore and the size to open with, taken from the settings. The position is
/// `None` if it is not trustworthy; the size is held between the smallest and the largest size.
pub fn start_geometry(settings: &WindowSettings) -> (Option<[f32; 2]>, [f32; 2]) {
    let position = (settings.x.is_finite()
        && settings.y.is_finite()
        && settings.x.abs() <= MAX_COORDINATE
        && settings.y.abs() <= MAX_COORDINATE)
        .then_some([settings.x as f32, settings.y as f32]);
    let size = |value: f64, least: f64| {
        let value = if value.is_finite() { value } else { least };
        value.clamp(least, MAX_SIZE) as f32
    };
    (
        position,
        [
            size(settings.width, MIN_WINDOW_WIDTH),
            size(settings.height, MIN_WINDOW_HEIGHT),
        ],
    )
}

/// The view that is shown at start, from the settings.
pub fn start_view_is_detailed(settings: &Settings) -> bool {
    settings.start_view == StartView::Detailed
}

/// Decides when a changed geometry is written: after it stayed the same for [`SAVE_AFTER`].
#[derive(Debug, Default)]
pub struct Debounce {
    last: Option<WindowSettings>,
    since: Option<Instant>,
}

impl Debounce {
    /// Takes in the geometry seen at `now`. Returns the geometry to save once it has stayed the
    /// same for [`SAVE_AFTER`] and differs from what is `saved`; otherwise `None`.
    pub fn update(
        &mut self,
        now: Instant,
        current: &WindowSettings,
        saved: &WindowSettings,
    ) -> Option<WindowSettings> {
        if self.last.as_ref() != Some(current) {
            self.last = Some(current.clone());
            self.since = Some(now);
            return None;
        }
        let stable = self
            .since
            .is_some_and(|since| now.saturating_duration_since(since) >= SAVE_AFTER);
        (stable && current != saved).then(|| current.clone())
    }
}

/// The command that brings the window level in line with the setting, if they differ.
pub fn level_command(applied_on_top: bool, wanted_on_top: bool) -> Option<WindowLevel> {
    (applied_on_top != wanted_on_top).then_some(if wanted_on_top {
        WindowLevel::AlwaysOnTop
    } else {
        WindowLevel::Normal
    })
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Pos2, Vec2};

    use super::*;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h))
    }

    fn geometry(x: f64, y: f64, width: f64, height: f64) -> WindowSettings {
        WindowSettings {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn req_018_geometry_is_outer_position_and_inner_size() {
        let got = geometry_from(
            Some(rect(100.0, 50.0, 336.0, 159.0)),
            Some(rect(108.0, 89.0, 320.0, 120.0)),
            false,
        );
        assert_eq!(got, Some(geometry(100.0, 50.0, 320.0, 120.0)));
    }

    #[test]
    fn req_018_geometry_is_not_taken_while_minimised_or_missing_or_unusable() {
        let outer = Some(rect(100.0, 50.0, 336.0, 159.0));
        let inner = Some(rect(108.0, 89.0, 320.0, 120.0));
        assert_eq!(geometry_from(outer, inner, true), None);
        assert_eq!(geometry_from(None, inner, false), None);
        assert_eq!(geometry_from(outer, None, false), None);
        // A minimised window on Windows sits far outside the screen.
        let far = Some(rect(-32_000.0, -32_000.0, 160.0, 28.0));
        assert_eq!(geometry_from(far, far, false), None);
        let tiny = Some(rect(0.0, 0.0, 10.0, 10.0));
        assert_eq!(geometry_from(outer, tiny, false), None);
        let nan = Some(rect(f32::NAN, 0.0, 320.0, 120.0));
        assert_eq!(geometry_from(nan, inner, false), None);
    }

    #[test]
    fn req_018_negative_positions_of_a_second_monitor_are_kept() {
        let got = geometry_from(
            Some(rect(-1900.0, 40.0, 336.0, 159.0)),
            Some(rect(-1892.0, 79.0, 320.0, 120.0)),
            false,
        );
        assert_eq!(got.map(|g| g.x), Some(-1900.0));
    }

    #[test]
    fn req_018_start_geometry_restores_what_was_saved() {
        let (position, size) = start_geometry(&geometry(250.0, 300.0, 520.0, 640.0));
        assert_eq!(position, Some([250.0, 300.0]));
        assert_eq!(size, [520.0, 640.0]);
    }

    #[test]
    fn req_018_start_geometry_holds_sizes_and_drops_untrustworthy_positions() {
        let (position, size) = start_geometry(&geometry(1.0e9, 5.0, 10.0, 1.0e9));
        assert_eq!(position, None);
        assert_eq!(size, [100.0, 8_000.0]);
        let (position, size) = start_geometry(&geometry(f64::NAN, 0.0, f64::NAN, f64::INFINITY));
        assert_eq!(position, None);
        // A value that is not a finite number falls back to the smallest size.
        assert_eq!(size, [100.0, 60.0]);
    }

    #[test]
    fn req_018_save_comes_after_one_quiet_second() {
        let start = Instant::now();
        let saved = geometry(100.0, 100.0, 320.0, 120.0);
        let moved = geometry(300.0, 100.0, 320.0, 120.0);
        let mut debounce = Debounce::default();
        // The first sight of a new geometry starts the wait.
        assert_eq!(debounce.update(start, &moved, &saved), None);
        assert_eq!(
            debounce.update(start + Duration::from_millis(999), &moved, &saved),
            None
        );
        assert_eq!(
            debounce.update(start + Duration::from_millis(1000), &moved, &saved),
            Some(moved.clone())
        );
    }

    #[test]
    fn req_018_a_window_that_keeps_moving_is_not_saved() {
        let start = Instant::now();
        let saved = geometry(100.0, 100.0, 320.0, 120.0);
        let mut debounce = Debounce::default();
        for step in 0..10_u32 {
            let moving = geometry(100.0 + f64::from(step), 100.0, 320.0, 120.0);
            let at = start + Duration::from_millis(400) * step;
            assert_eq!(debounce.update(at, &moving, &saved), None, "step {step}");
        }
    }

    #[test]
    fn req_018_nothing_is_saved_when_the_geometry_equals_the_saved_one() {
        let start = Instant::now();
        let saved = geometry(100.0, 100.0, 320.0, 120.0);
        let mut debounce = Debounce::default();
        assert_eq!(debounce.update(start, &saved, &saved), None);
        assert_eq!(
            debounce.update(start + Duration::from_secs(5), &saved, &saved),
            None
        );
    }

    #[test]
    fn req_018_always_on_top_follows_the_setting() {
        assert_eq!(level_command(true, true), None);
        assert_eq!(level_command(false, false), None);
        assert_eq!(level_command(true, false), Some(WindowLevel::Normal));
        assert_eq!(level_command(false, true), Some(WindowLevel::AlwaysOnTop));
    }

    #[test]
    fn req_029_the_start_view_comes_from_the_settings() {
        let mut settings = Settings::default();
        assert!(!start_view_is_detailed(&settings));
        settings.start_view = StartView::Detailed;
        assert!(start_view_is_detailed(&settings));
    }
}
