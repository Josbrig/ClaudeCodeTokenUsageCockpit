// SPDX-License-Identifier: Apache-2.0
//! Colours, symbols and small drawing helpers (concept §11.1). The hues come from the
//! Okabe-Ito palette, which stays distinguishable for people with colour vision deficiency, and
//! are the same in the dark and the light theme. Colour is never the only carrier: every state
//! also has a symbol and a text (see the view model).

use cockpit_core::metrics::PaceState;
use cockpit_core::viewmodel::{BINDING_GLYPH, NO_DATA_GLYPH, STALE_GLYPH, glyph};
use eframe::egui::{Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke, Vec2};

/// Under pace: blue.
pub const UNDER: Color32 = Color32::from_rgb(0x00, 0x72, 0xB2);
/// On pace: bluish green.
pub const ON_PACE: Color32 = Color32::from_rgb(0x00, 0x9E, 0x73);
/// Over pace: vermillion.
pub const OVER: Color32 = Color32::from_rgb(0xD5, 0x5E, 0x00);
/// Stale data and windows without data: grey.
pub const GREY: Color32 = Color32::from_rgb(0x7F, 0x7F, 0x7F);

/// The colour of a pace state.
pub fn state_color(state: PaceState) -> Color32 {
    match state {
        PaceState::Under => UNDER,
        PaceState::On => ON_PACE,
        PaceState::Over => OVER,
    }
}

/// Where a bar of `width` pixels is filled and where the target marker stands, for `used` and
/// `target` in percent. Values outside 0 to 100 are held at the ends, NaN counts as 0.
pub fn bar_positions(used: f64, target: f64, width: f32) -> (f32, f32) {
    // A value that is not a number counts as 0, so no drawing position is ever NaN.
    let share = |percent: f64| {
        let percent = if percent.is_nan() { 0.0 } else { percent };
        (percent.clamp(0.0, 100.0) / 100.0) as f32 * width
    };
    (share(used), share(target))
}

/// The symbols of concept §11.1. The standard fonts of the toolkit lack them, so they are
/// drawn as shapes; the text next to them comes from the view model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    /// ▼ under pace.
    Down,
    /// ● on pace.
    Dot,
    /// ▲ over pace.
    Up,
    /// ⏸ stale.
    Pause,
    /// ○ no data.
    Ring,
    /// ⚑ binding limit.
    Flag,
}

/// The icon that belongs to a glyph of the view model; `None` for an unknown glyph.
pub fn icon_for_glyph(glyph_text: &str) -> Option<Icon> {
    [
        (glyph(PaceState::Under), Icon::Down),
        (glyph(PaceState::On), Icon::Dot),
        (glyph(PaceState::Over), Icon::Up),
        (STALE_GLYPH, Icon::Pause),
        (NO_DATA_GLYPH, Icon::Ring),
        (BINDING_GLYPH, Icon::Flag),
    ]
    .into_iter()
    .find(|(text, _)| *text == glyph_text)
    .map(|(_, icon)| icon)
}

/// Draws `icon` centred on `center`, inside a square of `size` pixels.
pub fn paint_icon(painter: &Painter, center: Pos2, size: f32, icon: Icon, color: Color32) {
    let half = size / 2.0;
    let at = |dx: f32, dy: f32| Pos2::new(center.x + dx * half, center.y + dy * half);
    match icon {
        Icon::Down => {
            painter.add(Shape::convex_polygon(
                vec![at(-1.0, -0.7), at(1.0, -0.7), at(0.0, 0.8)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Up => {
            painter.add(Shape::convex_polygon(
                vec![at(-1.0, 0.7), at(1.0, 0.7), at(0.0, -0.8)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Dot => {
            painter.add(Shape::circle_filled(center, half * 0.8, color));
        }
        Icon::Ring => {
            painter.add(Shape::circle_stroke(
                center,
                half * 0.75,
                Stroke::new(1.5_f32, color),
            ));
        }
        Icon::Pause => {
            let bar = Vec2::new(half * 0.55, size * 0.8);
            for dx in [-0.55_f32, 0.55] {
                painter.rect_filled(
                    Rect::from_center_size(at(dx, 0.0), bar),
                    CornerRadius::ZERO,
                    color,
                );
            }
        }
        Icon::Flag => {
            painter.line_segment([at(-0.6, 1.0), at(-0.6, -1.0)], Stroke::new(1.5_f32, color));
            painter.add(Shape::convex_polygon(
                vec![at(-0.6, -1.0), at(0.9, -0.5), at(-0.6, 0.1)],
                color,
                Stroke::NONE,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_003_each_state_has_its_own_okabe_ito_colour() {
        assert_eq!(
            state_color(PaceState::Under),
            Color32::from_rgb(0, 114, 178)
        );
        assert_eq!(state_color(PaceState::On), Color32::from_rgb(0, 158, 115));
        assert_eq!(state_color(PaceState::Over), Color32::from_rgb(213, 94, 0));
        let all = [UNDER, ON_PACE, OVER, GREY];
        for (i, a) in all.iter().enumerate() {
            assert!(all[i + 1..].iter().all(|b| a != b), "colours must differ");
        }
    }

    #[test]
    fn req_114_every_glyph_of_the_view_model_has_an_icon() {
        let glyphs = [
            glyph(PaceState::Under),
            glyph(PaceState::On),
            glyph(PaceState::Over),
            STALE_GLYPH,
            NO_DATA_GLYPH,
            BINDING_GLYPH,
        ];
        let icons: Vec<Option<Icon>> = glyphs.iter().map(|g| icon_for_glyph(g)).collect();
        assert!(icons.iter().all(Option::is_some));
        for (i, a) in icons.iter().enumerate() {
            assert!(icons[i + 1..].iter().all(|b| a != b), "icons must differ");
        }
        assert_eq!(icon_for_glyph("?"), None);
    }

    #[test]
    fn req_029_bar_positions_follow_the_percentages() {
        assert_eq!(bar_positions(50.0, 25.0, 100.0), (50.0, 25.0));
        assert_eq!(bar_positions(0.0, 0.0, 80.0), (0.0, 0.0));
        assert_eq!(bar_positions(100.0, 100.0, 80.0), (80.0, 80.0));
    }

    #[test]
    fn req_029_bar_positions_are_held_inside_the_bar() {
        assert_eq!(bar_positions(130.0, -5.0, 100.0), (100.0, 0.0));
        assert_eq!(bar_positions(f64::NAN, f64::INFINITY, 100.0), (0.0, 100.0));
    }
}
