//! Undertow's shared visual roles and small native chrome treatments.
use eframe::egui::{self, Color32, Painter, Rect, Stroke, vec2};

pub(super) const CANVAS: Color32 = Color32::BLACK;
pub(super) const SIDEBAR: Color32 = CANVAS;
pub(super) const SURFACE: Color32 = Color32::from_gray(21);
pub(super) const CHROME: Color32 = Color32::from_gray(30);
pub(super) const POPOVER: Color32 = Color32::from_gray(43);
pub(super) const HOVER: Color32 = Color32::from_gray(58);
pub(super) const SELECTED: Color32 = Color32::from_gray(37);
pub(super) const TEXT: Color32 = Color32::from_gray(248);
pub(super) const MUTED: Color32 = Color32::from_gray(144);
pub(super) const BRAND: Color32 = Color32::WHITE;
pub(super) const INK: Color32 = Color32::from_gray(16);
pub(super) const SEPARATOR: Color32 = Color32::from_gray(32);
pub(super) const SEEK_TRACK: Color32 = Color32::from_gray(65);
pub(super) const CONTROL: u8 = 9;
pub(super) const ART: u8 = 3;
pub(super) const PANEL: u8 = 16;
pub(super) const TITLE_BAR: f32 = 38.;
pub(super) const TOOLBAR: f32 = 80.;
pub(super) const TITLE: f32 = 36.;
pub(super) const SECTION: f32 = 24.;
pub(super) const FAST: f32 = 0.14;

pub(super) fn mark(painter: &Painter, bounds: Rect) {
    let center = bounds.center();
    for (x, y) in [(-10.5, -5.25), (0., -5.25), (10.5, -5.25), (0., 5.25)] {
        let cell = center + vec2(x, y);
        painter.add(egui::Shape::convex_polygon(
            [(0., -5.25), (5.25, 0.), (0., 5.25), (-5.25, 0.)]
                .map(|(dx, dy)| cell + vec2(dx, dy))
                .to_vec(),
            TEXT,
            Stroke::NONE,
        ));
    }
}

pub(super) fn browse_margin(width: f32) -> f32 {
    (width * 0.047).clamp(24., 92.)
}

/// Hover follows the row's bounds, including nested covers, links and buttons.
pub(super) fn row_hover(ui: &egui::Ui, row: &egui::Response, base: Color32) -> bool {
    let hovered = ui.is_enabled() && ui.rect_contains_pointer(row.rect);
    let fill = if !hovered {
        base
    } else if base == Color32::TRANSPARENT {
        Color32::from_white_alpha(16)
    } else {
        base.lerp_to_gamma(Color32::WHITE, 0.065)
    };
    if fill != Color32::TRANSPARENT {
        ui.painter().rect_filled(row.rect, 6, fill);
    }
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    hovered
}

pub(super) fn skeleton(ui: &mut egui::Ui, width: f32) {
    let visible = width.max(0.);
    let tiles = ((visible / 145.) as usize).clamp(2, 5);
    let size = ((visible - 16. * (tiles - 1) as f32) / tiles as f32).max(1.);
    let (shelf, _) = ui.allocate_exact_size(vec2(visible, size + 36.), egui::Sense::hover());
    for index in 0..tiles {
        let start = shelf.min + vec2(index as f32 * (size + 16.), 0.);
        let cover = Rect::from_min_size(start, vec2(size, size));
        ui.painter().rect_filled(cover, ART, SURFACE);
        ui.painter().rect_filled(
            Rect::from_min_size(start + vec2(0., size + 13.), vec2(size * 0.6, 9.)),
            4,
            SURFACE,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{CentralPanel, Context, Event, RawInput, Sense, pos2};

    #[test]
    fn row_stays_highlighted_over_a_nested_control() {
        let ctx = Context::default();
        let mut states = Vec::new();
        for pointer in [pos2(36., 46.), pos2(172., 46.), pos2(265., 46.)] {
            let output = ctx.run(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(pos2(0., 0.), vec2(320., 120.))),
                    events: vec![Event::PointerMoved(pointer)],
                    ..RawInput::default()
                },
                |ctx| {
                    CentralPanel::default().show(ctx, |ui| {
                        let row = ui.interact(
                            Rect::from_min_size(pos2(20., 20.), vec2(200., 54.)),
                            ui.id().with("row"),
                            Sense::click(),
                        );
                        let menu = ui.interact(
                            Rect::from_min_size(pos2(156., 32.), vec2(32., 28.)),
                            row.id.with("menu"),
                            Sense::click(),
                        );
                        states.push((row_hover(ui, &row, Color32::TRANSPARENT), menu.hovered()));
                    });
                },
            );
            if pointer.x < 220. {
                assert_eq!(
                    output.platform_output.cursor_icon,
                    egui::CursorIcon::PointingHand
                );
            }
        }
        assert_eq!(states, [(true, false), (true, true), (false, false)]);
    }
}
