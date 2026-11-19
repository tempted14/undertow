use super::*;

struct Scrub {
    pointer: Pos2,
    seconds: f64,
    preview: bool,
    commit: bool,
}

fn scrub(
    ui: &egui::Ui,
    rail: Rect,
    generation: u64,
    duration: f64,
    seekable: bool,
) -> Option<Scrub> {
    if !seekable || !duration.is_finite() || duration <= 0. || !ui.is_enabled() {
        return None;
    }
    let seek = ui
        .interact(
            rail.expand2(vec2(0., 8.)),
            ui.make_persistent_id(("seek", generation)),
            Sense::click_and_drag(),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let button = egui::PointerButton::Primary;
    let preview = seek.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_down());
    let commit = seek.clicked_by(button) || seek.drag_stopped_by(button);
    let pointer = if commit || preview {
        seek.interact_pointer_pos()
    } else {
        seek.hover_pos()
    }?;
    Some(Scrub {
        pointer,
        seconds: crate::model::seek_time(pointer.x, rail.left(), rail.width(), duration),
        preview,
        commit,
    })
}

impl App {
    pub(super) fn player_timeline(
        &self,
        ui: &egui::Ui,
        player: Rect,
        rail: Rect,
        duration: f64,
    ) -> f64 {
        let seek = scrub(ui, rail, self.generation, duration, self.audio.is_some());
        let mut position = self.audio.as_ref().map_or(0., Audio::timeline_position);
        if let Some(s) = &seek {
            if s.commit
                && let Some(audio) = &self.audio
            {
                audio.seek(s.seconds);
            }
            if s.preview || s.commit {
                position = s.seconds;
            }
        }
        let painter = ui.painter();
        painter.rect_filled(rail, 2, design::SEEK_TRACK);
        if duration.is_finite() && duration > 0. {
            let x = rail.left() + rail.width() * (position / duration).clamp(0., 1.) as f32;
            painter.rect_filled(
                Rect::from_min_max(rail.min, pos2(x, rail.bottom())),
                2,
                design::BRAND,
            );
            if let Some(s) = seek {
                painter.circle_filled(
                    pos2(x, rail.center().y),
                    if s.preview { 6. } else { 4.5 },
                    design::BRAND,
                );
                let bubble = Rect::from_center_size(
                    pos2(
                        layout::seek_preview_x(player, s.pointer.x),
                        rail.top() - 22.,
                    ),
                    vec2(54., 27.),
                );
                let preview = ui.ctx().layer_painter(egui::LayerId::new(
                    egui::Order::Tooltip,
                    Id::new("seek-preview"),
                ));
                preview.rect_filled(bubble, 7, design::BRAND);
                preview.text(
                    bubble.center(),
                    Align2::CENTER_CENTER,
                    clock(s.seconds),
                    font(13., true),
                    design::INK,
                );
            }
        }
        position
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Modifiers, PointerButton, RawInput};

    fn click(button: PointerButton, x: f32, pressed: bool) -> Event {
        Event::PointerButton {
            pos: pos2(x, 30.),
            button,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }

    fn frame(ctx: &egui::Context, generation: u64, events: Vec<Event>) -> Option<Scrub> {
        let mut result = None;
        let _ = ctx.run(
            RawInput {
                events,
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(320., 100.))),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let rail = Rect::from_min_size(pos2(20., 28.), vec2(200., 4.));
                    result = scrub(ui, rail, generation, 100., true);
                });
            },
        );
        result
    }

    #[test]
    fn dragging_only_commits_on_release_even_outside_the_rail() {
        let ctx = egui::Context::default();
        frame(&ctx, 1, vec![Event::PointerMoved(pos2(70., 30.))]);
        let press = frame(&ctx, 1, vec![click(PointerButton::Primary, 70., true)]).unwrap();
        assert!(press.preview && !press.commit);
        assert_eq!(press.seconds, 25.);
        for (x, seconds) in [(120., 50.), (280., 100.)] {
            let drag = frame(&ctx, 1, vec![Event::PointerMoved(pos2(x, 30.))]).unwrap();
            assert!(drag.preview && !drag.commit);
            assert_eq!(drag.seconds, seconds);
        }
        let release = frame(&ctx, 1, vec![click(PointerButton::Primary, 280., false)]).unwrap();
        assert!(release.commit && !release.preview);
        assert_eq!(release.seconds, 100.);
        assert!(frame(&ctx, 1, vec![]).is_none_or(|s| !s.commit));
    }

    #[test]
    fn only_primary_click_seeks_and_track_changes_cancel_drag() {
        let ctx = egui::Context::default();
        frame(&ctx, 1, vec![Event::PointerMoved(pos2(70., 30.))]);
        let hover = frame(&ctx, 1, vec![]).unwrap();
        assert!(!hover.preview && !hover.commit);
        for pressed in [true, false] {
            assert!(
                frame(&ctx, 1, vec![click(PointerButton::Secondary, 70., pressed)])
                    .is_none_or(|s| !s.preview && !s.commit)
            );
        }
        frame(&ctx, 1, vec![click(PointerButton::Primary, 70., true)]);
        assert!(
            frame(&ctx, 1, vec![click(PointerButton::Primary, 70., false)])
                .unwrap()
                .commit
        );
        frame(&ctx, 1, vec![click(PointerButton::Primary, 70., true)]);
        frame(&ctx, 1, vec![Event::PointerMoved(pos2(150., 30.))]);
        frame(&ctx, 2, vec![]);
        assert!(
            frame(&ctx, 2, vec![click(PointerButton::Primary, 150., false)])
                .is_none_or(|s| !s.commit)
        );
    }
}
