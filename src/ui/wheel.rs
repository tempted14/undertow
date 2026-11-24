//! Sample wheel motion once per frame while leaving scroll-area routing to egui.
use eframe::egui::{self, Event, MouseWheelUnit, Vec2, vec2};

#[derive(Default)]
pub(super) struct Wheel {
    pending: Vec2,
    frame: Option<u64>,
}

impl Wheel {
    pub fn clear(&mut self) {
        self.pending = Vec2::ZERO;
    }

    pub fn apply(&mut self, ctx: &egui::Context) -> bool {
        if self.pending == Vec2::ZERO
            && ctx.input(|input| {
                input.raw_scroll_delta == Vec2::ZERO && input.smooth_scroll_delta == Vec2::ZERO
            })
        {
            return false;
        }
        let frame = ctx.cumulative_frame_nr();
        if self.frame.replace(frame) == Some(frame) {
            // egui may discard and repeat layout on the same display frame.
            ctx.input_mut(|input| input.smooth_scroll_delta = Vec2::ZERO);
            return false;
        }
        let options = ctx.options(|opts| opts.input_options);
        ctx.input_mut(|input| {
            let mut wheel = Vec2::ZERO;
            let mut precise = Vec2::ZERO;
            for event in &input.events {
                if matches!(event, Event::WindowFocused(false)) {
                    self.clear();
                    continue;
                }
                let Event::MouseWheel {
                    unit,
                    delta,
                    modifiers,
                } = event
                else {
                    continue;
                };
                if modifiers.matches_any(options.zoom_modifier) {
                    continue;
                }
                let delta = match unit {
                    MouseWheelUnit::Point => *delta,
                    MouseWheelUnit::Line => *delta * options.line_scroll_speed,
                    MouseWheelUnit::Page => *delta * input.viewport_rect().height(),
                };
                if !delta.is_finite() {
                    continue;
                }
                let horizontal = modifiers.matches_any(options.horizontal_scroll_modifier);
                let vertical = modifiers.matches_any(options.vertical_scroll_modifier);
                let delta = match (horizontal, vertical) {
                    (true, false) => vec2(delta.x + delta.y, 0.),
                    (false, true) => vec2(0., delta.x + delta.y),
                    _ => delta,
                };
                if *unit == MouseWheelUnit::Point {
                    precise += delta;
                } else {
                    wheel += delta;
                }
            }
            let applied = self.sample(wheel, precise, input.stable_dt);
            input.smooth_scroll_delta = applied;
            applied != Vec2::ZERO
        })
    }

    fn sample(&mut self, wheel: Vec2, precise: Vec2, seconds: f32) -> Vec2 {
        // Move on the first input frame and ease the rest using elapsed time.
        // Precision devices already emit their own smooth, momentum-aware deltas.
        let easing = 1. - 2f32.powf(-seconds.clamp(0., 0.1) / 0.018);
        let mut applied = precise;
        for axis in 0..2 {
            let change = wheel[axis];
            let remaining = &mut self.pending[axis];
            if precise[axis] != 0. || change * *remaining < 0. {
                *remaining = 0.;
            }
            let after_easing = *remaining * (1. - easing);
            let tail = if after_easing.abs() < 0.2 {
                *remaining
            } else {
                *remaining - after_easing
            };
            let first = change * 0.4;
            applied[axis] += tail + first;
            *remaining += change - first - tail;
        }
        applied
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wheel_tick_starts_immediately_and_finishes_at_any_refresh_rate() {
        for fps in [30., 60., 120., 165., 240.] {
            let mut wheel = Wheel::default();
            let dt = 1. / fps;
            let first = wheel.sample(vec2(0., -40.), Vec2::ZERO, dt);
            assert_eq!(first.y, -16.);
            let mut total = first;
            for _ in 0..(fps * 0.16) as usize {
                total += wheel.sample(Vec2::ZERO, Vec2::ZERO, dt);
            }
            assert!(
                (total.y + 40.).abs() < 0.001,
                "{fps} Hz travelled {total:?}"
            );
            assert_eq!(wheel.sample(Vec2::ZERO, Vec2::ZERO, dt), Vec2::ZERO);
        }
    }

    #[test]
    fn reverse_wheel_and_precision_input_cancel_old_momentum_on_their_axis() {
        let mut wheel = Wheel::default();
        let dt = 1. / 120.;
        wheel.sample(vec2(40., -40.), Vec2::ZERO, dt);
        let moved = wheel.sample(vec2(0., 10.), vec2(1.25, 0.), dt);
        assert_eq!(moved, vec2(1.25, 4.));
        let tail = wheel.sample(Vec2::ZERO, Vec2::ZERO, dt);
        assert_eq!(tail.x, 0.);
        assert!(tail.y > 0.);
    }

    #[test]
    fn native_scroll_area_receives_the_full_wheel_tick_at_120_hz() {
        let ctx = egui::Context::default();
        let mut wheel = Wheel::default();
        let mut offset = 0.;
        for frame in 0..20 {
            let events = match frame {
                0 => vec![Event::PointerMoved(egui::pos2(60., 60.))],
                1 => vec![Event::MouseWheel {
                    unit: MouseWheelUnit::Line,
                    delta: vec2(0., -1.),
                    modifiers: egui::Modifiers::NONE,
                }],
                _ => Vec::new(),
            };
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        vec2(480., 360.),
                    )),
                    time: Some(f64::from(frame) / 120.),
                    predicted_dt: 1. / 120.,
                    events,
                    ..Default::default()
                },
                |ctx| {
                    wheel.apply(ctx);
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let output = egui::ScrollArea::vertical()
                            .max_height(180.)
                            .show(ui, |ui| {
                                ui.allocate_space(vec2(200., 900.));
                            });
                        offset = output.state.offset.y;
                    });
                },
            );
            if frame == 1 {
                assert_eq!(offset, 16.);
            }
        }
        assert!((offset - 40.).abs() < 0.001);
    }
}
