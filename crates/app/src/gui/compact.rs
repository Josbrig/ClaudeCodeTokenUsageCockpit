// SPDX-License-Identifier: Apache-2.0
//! The compact view (concept §11.2): two rows, one per window, and a bottom line. It only lays
//! out what the view model provides; all texts and decisions come from there.

use cockpit_core::viewmodel::{
    BINDING_GLYPH, NO_DATA_GLYPH, STALE_GLYPH, STALE_LABEL, ViewModel, WindowView,
};
use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Sense, Shape, Stroke,
    StrokeKind, Ui, Vec2,
};

use super::theme;

/// Height of one window row in logical pixels.
const ROW_HEIGHT: f32 = 26.0;
/// Width of the bar in logical pixels.
const BAR_WIDTH: f32 = 90.0;
/// Height of the bar in logical pixels.
const BAR_HEIGHT: f32 = 12.0;
/// Text size in the rows.
const TEXT_SIZE: f32 = 12.5;
/// Left margin and the widths of the columns of a row; together they fit 320 pixels.
const MARGIN: f32 = 2.0;
const LABEL_WIDTH: f32 = 22.0;
const FLAG_WIDTH: f32 = 14.0;
const STATE_WIDTH: f32 = 68.0;
const USED_WIDTH: f32 = 50.0;
/// Size of the symbols in logical pixels.
const ICON_SIZE: f32 = 10.0;
/// Width the symbol and the gap after it take in front of a state text.
const ICON_SPACE: f32 = 15.0;

/// What the person did in the compact view.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Action {
    /// The switch button or the key `D` was used: show the detailed view.
    pub switch_view: bool,
}

/// Draws the compact view into `ui`.
pub fn show(ui: &mut Ui, view: &ViewModel) -> Action {
    ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);
    if let Some(banner) = &view.banner {
        ui.horizontal_wrapped(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(ICON_SIZE), Sense::hover());
            if let Some(icon) = theme::icon_for_glyph(NO_DATA_GLYPH) {
                theme::paint_icon(ui.painter(), rect.center(), ICON_SIZE, icon, theme::GREY);
            }
            ui.add_space(5.0);
            ui.label(banner);
        });
    } else {
        row(ui, "5h", &view.five_hour, view.stale);
        row(ui, "7d", &view.seven_day, view.stale);
    }
    let mut action = Action::default();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(&view.age_text).size(TEXT_SIZE));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (rect, response) = ui.allocate_exact_size(Vec2::new(24.0, 16.0), Sense::click());
            let response = response.on_hover_text("Detailed view (D)");
            paint_switch_button(ui, rect, &response);
            action.switch_view = response.clicked();
        });
    });
    if super::plain_d_pressed(ui.ctx()) {
        action.switch_view = true;
    }
    action
}

/// Draws one window row: label, binding flag, bar with target marker, state, used and reset.
fn row(ui: &mut Ui, label: &str, window: &WindowView, stale: bool) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, ROW_HEIGHT), Sense::hover());
    let painter = ui.painter_at(rect);
    let text_color = ui.visuals().text_color();
    let font = FontId::proportional(TEXT_SIZE);
    let mid = rect.center().y;
    let text = |x: f32, text: &str, color: Color32| {
        painter.text(
            Pos2::new(x, mid),
            Align2::LEFT_CENTER,
            text,
            font.clone(),
            color,
        );
    };
    let mut x = rect.left() + MARGIN;

    text(x, label, text_color);
    x += LABEL_WIDTH;
    let binding = matches!(window, WindowView::Data(data) if data.binding);
    if binding && let Some(flag) = theme::icon_for_glyph(BINDING_GLYPH) {
        let at = Pos2::new(x + 5.0, mid);
        theme::paint_icon(&painter, at, ICON_SIZE + 2.0, flag, text_color);
    }
    x += FLAG_WIDTH;
    let bar = Rect::from_min_size(
        Pos2::new(x, mid - BAR_HEIGHT / 2.0),
        Vec2::new(BAR_WIDTH, BAR_HEIGHT),
    );
    x += BAR_WIDTH + 6.0;
    painter.rect_filled(bar, CornerRadius::same(2), ui.visuals().extreme_bg_color);

    match window {
        WindowView::Data(data) => {
            let color = if stale {
                theme::GREY
            } else {
                theme::state_color(data.state)
            };
            let (fill, marker) = theme::bar_positions(data.used, data.target, BAR_WIDTH);
            let filled = Rect::from_min_size(bar.min, Vec2::new(fill, BAR_HEIGHT));
            painter.rect_filled(filled, CornerRadius::same(2), color);
            let marker_x = bar.left() + marker;
            painter.line_segment(
                [
                    Pos2::new(marker_x, bar.top() - 2.0),
                    Pos2::new(marker_x, bar.bottom() + 2.0),
                ],
                Stroke::new(1.5_f32, text_color),
            );
            let (glyph, state_text) = if stale {
                (STALE_GLYPH, STALE_LABEL)
            } else {
                (data.glyph, data.label)
            };
            symbol_and_text(&painter, x, mid, glyph, state_text, color, &font);
            x += STATE_WIDTH;
            text(x, &data.used_text, text_color);
            x += USED_WIDTH;
            text(x, &data.countdown_text, text_color);
        }
        WindowView::NoData { text: reason } => {
            symbol_and_text(&painter, x, mid, NO_DATA_GLYPH, reason, theme::GREY, &font);
        }
    }
    painter.rect_stroke(
        bar,
        CornerRadius::same(2),
        Stroke::new(1.0_f32, theme::GREY),
        StrokeKind::Inside,
    );
    if stale {
        // A dashed outline marks stale values without relying on colour.
        // Inset by half the line width so the whole line stays inside the clipped area.
        let inner = rect.shrink(0.5);
        let corners = [
            inner.left_top(),
            inner.right_top(),
            inner.right_bottom(),
            inner.left_bottom(),
            inner.left_top(),
        ];
        let shapes = Shape::dashed_line(&corners, Stroke::new(1.0_f32, theme::GREY), 4.0, 3.0);
        painter.extend(shapes);
    }
}

/// A symbol followed by a text, the symbol drawn as a shape.
fn symbol_and_text(
    painter: &Painter,
    x: f32,
    mid: f32,
    glyph: &str,
    text: &str,
    color: Color32,
    font: &FontId,
) {
    if let Some(icon) = theme::icon_for_glyph(glyph) {
        let at = Pos2::new(x + ICON_SIZE / 2.0, mid);
        theme::paint_icon(painter, at, ICON_SIZE, icon, color);
    }
    painter.text(
        Pos2::new(x + ICON_SPACE, mid),
        Align2::LEFT_CENTER,
        text,
        font.clone(),
        color,
    );
}

/// The button that switches to the detailed view: a frame with two arrows pointing apart.
fn paint_switch_button(ui: &Ui, rect: Rect, response: &egui::Response) {
    let visuals = ui.style().interact(response);
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(3),
        visuals.bg_fill,
        visuals.bg_stroke,
        StrokeKind::Inside,
    );
    let stroke = Stroke::new(1.5_f32, visuals.fg_stroke.color);
    let c = rect.center();
    let a = Pos2::new(c.x - 4.0, c.y + 3.5);
    let b = Pos2::new(c.x + 4.0, c.y - 3.5);
    painter.line_segment([a, b], stroke);
    for (tip, side) in [(b, 1.0_f32), (a, -1.0_f32)] {
        painter.line_segment([tip, Pos2::new(tip.x - 3.0 * side, tip.y)], stroke);
        painter.line_segment([tip, Pos2::new(tip.x, tip.y + 3.0 * side)], stroke);
    }
}
