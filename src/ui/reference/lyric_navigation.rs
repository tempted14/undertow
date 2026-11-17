use super::*;

/// Only navigates the lyric viewport: never seeks audio or changes timestamps.
pub(super) fn scroll(ui: &mut egui::Ui, rect: Rect, content_height: f32, offset: &mut f32) -> bool {
    // Native startup, resize and shutdown can briefly expose a collapsed viewport.
    // Keep the reading position without constructing an invalid scrollbar.
    if !rect.is_finite()
        || rect.width() <= 0.
        || rect.height() <= 0.
        || !content_height.is_finite()
        || content_height <= 0.
    {
        return false;
    }
    let limit = (content_height - rect.height()).max(0.);
    *offset = if offset.is_finite() {
        offset.clamp(0., limit)
    } else {
        0.
    };
    if limit <= 0. {
        return false;
    }
    let mut manual = false;
    if ui.rect_contains_pointer(rect) {
        let delta = ui.input_mut(|i| {
            let delta = i.smooth_scroll_delta.y;
            i.smooth_scroll_delta.y = 0.;
            delta
        });
        if delta.abs() > 0.01 {
            *offset = (*offset - delta).clamp(0., limit);
            manual = true;
        }
    }
    let rail = Rect::from_min_max(rect.right_top() - vec2(10., 0.), rect.right_bottom());
    let thumb_height = (rail.height() * rail.height() / content_height)
        .clamp(24f32.min(rail.height()), rail.height());
    let response = ui
        .interact(rail, Id::new("lyric-scrollbar"), Sense::click_and_drag())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if (response.dragged() || response.clicked())
        && let Some(p) = response.interact_pointer_pos()
    {
        *offset = ((p.y - rail.top() - thumb_height * 0.5)
            / (rail.height() - thumb_height).max(1.))
        .clamp(0., 1.)
            * limit;
        manual = true;
    }
    let thumb = Rect::from_min_size(
        pos2(
            rail.left() + 3.,
            rail.top() + (rail.height() - thumb_height) * *offset / limit,
        ),
        vec2(4., thumb_height),
    );
    ui.painter().rect_filled(
        thumb,
        3,
        Color32::from_white_alpha(if response.hovered() || response.dragged() {
            170
        } else {
            55
        }),
    );
    if manual {
        ui.ctx().request_repaint();
    }
    manual
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn collapsed_tiny_and_invalid_viewports_keep_navigation_safe() {
        for height in [-304., 0., 1., 10., 23., 24., 300.] {
            let ctx = egui::Context::default();
            let mut offset = 90.;
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    assert!(!scroll(
                        ui,
                        Rect::from_min_size(pos2(300., 100.), vec2(300., height)),
                        1200.,
                        &mut offset
                    ));
                });
            });
            assert_eq!(
                offset, 90.,
                "Transient viewports must retain the reading position"
            );
        }
        for content in [0., f32::NAN, f32::INFINITY] {
            let ctx = egui::Context::default();
            let mut offset = 90.;
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    assert!(!scroll(
                        ui,
                        Rect::from_min_size(Pos2::ZERO, vec2(300., 300.)),
                        content,
                        &mut offset
                    ));
                });
            });
            assert_eq!(offset, 90.);
        }
    }
    #[test]
    fn wheel_and_scrollbar_navigate_only_inside_the_lyric_viewport() {
        let rect = Rect::from_min_size(pos2(300., 100.), vec2(300., 300.));
        for (pointer, wheel, expected) in [
            (pos2(450., 200.), -90., true),
            (pos2(50., 50.), -90., false),
        ] {
            let ctx = egui::Context::default();
            let mut offset = 0.;
            let mut moved = false;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800., 600.))),
                    events: vec![egui::Event::PointerMoved(pointer)],
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.input_mut(|i| i.smooth_scroll_delta = vec2(0., wheel));
                        moved = scroll(ui, rect, 1200., &mut offset);
                    });
                },
            );
            assert_eq!(moved, expected);
            assert_eq!(offset, if expected { 90. } else { 0. });
        }
        let ctx = egui::Context::default();
        let mut offset = 0.;
        let pointer = pos2(595., 360.);
        let mut moved = false;
        for pressed in [None, Some(true), Some(false)] {
            let mut events = vec![egui::Event::PointerMoved(pointer)];
            if let Some(pressed) = pressed {
                events.push(egui::Event::PointerButton {
                    pos: pointer,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                });
            }
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800., 600.))),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        moved |= scroll(ui, rect, 1200., &mut offset);
                    });
                },
            );
        }
        assert!(moved && offset > 700. && offset <= 900.);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                scroll(ui, rect, 400., &mut offset);
            });
        });
        assert!(
            offset <= 100.,
            "Resizing clamps the retained reading position"
        );
    }
}
