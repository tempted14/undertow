use super::*;

impl App {
    pub(in crate::ui) fn home_viewport(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_rect_before_wrap();
        let viewport = Rect::from_min_max(
            available.min,
            pos2(
                available.right(),
                available.bottom() - reserved_player(ui.ctx()) - 18.,
            ),
        );
        if !viewport.is_finite() || viewport.width() <= 10. || viewport.height() <= 0. {
            return;
        }
        let limit = (self.home_content_height - viewport.height()).max(0.);
        if ui.rect_contains_pointer(viewport) {
            // Keep the fractional displacement from the shared wheel motion;
            // ScrollArea rounds its content origin to whole pixels at high refresh rates.
            let delta = ui.ctx().input_mut(|i| {
                let d = i.smooth_scroll_delta.y;
                i.smooth_scroll_delta.y = 0.;
                d
            });
            self.home_scroll = (self.home_scroll - delta).clamp(0., limit);
            if delta.abs() > 0.01 {
                ui.ctx().request_repaint();
            }
        }
        let origin = viewport.min - vec2(0., self.home_scroll);
        let mut content = ui.new_child(egui::UiBuilder::new().id_salt("home-subpixel").max_rect(
            Rect::from_min_size(origin, vec2(viewport.width() - 10., 10000.)),
        ));
        content.set_clip_rect(viewport.intersect(ui.clip_rect()));
        self.home(&mut content);
        self.home_content_height = content.min_rect().height();
        let limit = (self.home_content_height - viewport.height()).max(0.);
        self.home_scroll = self.home_scroll.clamp(0., limit);
        if limit > 0. {
            let track =
                Rect::from_min_max(viewport.right_top() - vec2(9., 0.), viewport.right_bottom());
            let thumb_height = (viewport.height() * viewport.height() / self.home_content_height)
                .clamp(45f32.min(viewport.height()), viewport.height());
            let response = ui.interact(track, Id::new("home-scrollbar"), Sense::click_and_drag());
            if (response.dragged() || response.clicked())
                && let Some(p) = response.interact_pointer_pos()
            {
                self.home_scroll = ((p.y - track.top() - thumb_height * 0.5)
                    / (track.height() - thumb_height).max(1.))
                .clamp(0., 1.)
                    * limit;
                ui.ctx().request_repaint();
            }
            let thumb = Rect::from_min_size(
                pos2(
                    track.left() + 2.,
                    track.top() + (track.height() - thumb_height) * self.home_scroll / limit,
                ),
                vec2(5., thumb_height),
            );
            ui.painter().rect_filled(
                thumb,
                4,
                Color32::from_white_alpha(if response.hovered() || response.dragged() {
                    155
                } else {
                    65
                }),
            );
            if !ui.ctx().wants_keyboard_input() {
                let mut destination = self.home_scroll;
                ui.ctx().input(|i| {
                    if i.key_pressed(egui::Key::PageDown) {
                        destination += viewport.height() * 0.9;
                    }
                    if i.key_pressed(egui::Key::PageUp) {
                        destination -= viewport.height() * 0.9;
                    }
                    if i.key_pressed(egui::Key::Home) {
                        destination = 0.;
                    }
                    if i.key_pressed(egui::Key::End) {
                        destination = limit;
                    }
                });
                self.home_scroll = destination.clamp(0., limit);
            }
        }
    }
}
