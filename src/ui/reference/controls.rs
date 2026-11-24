use super::*;

fn paint_transport_glyph(
    painter: &egui::Painter,
    center: Pos2,
    name: &str,
    scale: f32,
    color: Color32,
) {
    let point = |x, y| center + vec2(x, y) * scale;
    let fill = |points: &[(f32, f32)]| {
        painter.add(egui::Shape::convex_polygon(
            points.iter().map(|&(x, y)| point(x, y)).collect(),
            color,
            Stroke::NONE,
        ));
    };
    let bar = |left, right, half_height: f32| {
        painter.rect_filled(
            Rect::from_min_max(point(left, -half_height), point(right, half_height)),
            1,
            color,
        );
    };
    let stroke = Stroke::new(1.8 * scale, color);
    let line = |points: &[(f32, f32)]| {
        painter.add(egui::Shape::line(
            points.iter().map(|&(x, y)| point(x, y)).collect(),
            stroke,
        ));
    };
    let curve = |points: [(f32, f32); 4]| {
        painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
            points.map(|(x, y)| point(x, y)),
            false,
            Color32::TRANSPARENT,
            stroke,
        ));
    };
    match name {
        "play" => fill(&[
            (-9.8, -14.4),
            (-8.6, -14.4),
            (10.5, -1.0),
            (10.9, 0.0),
            (10.5, 1.0),
            (-8.6, 14.4),
            (-9.8, 14.4),
        ]),
        "pause" => {
            bar(-9.5, -3.5, 14.5);
            bar(3.5, 9.5, 14.5);
        }
        "previous" | "next" => {
            let sign = if name == "next" { 1.0 } else { -1.0 };
            fill(
                &[(-8., -8.8), (-7., -8.8), (5.4, 0.), (-7., 8.8), (-8., 8.8)]
                    .map(|(x, y)| (x * sign, y)),
            );
            if sign > 0.0 {
                bar(5., 9., 9.5);
            } else {
                bar(-9., -5., 9.5);
            }
        }
        "shuffle" => {
            curve([(-9., 6.), (0., 6.), (-2., -6.), (8.5, -6.)]);
            curve([(-9., -6.), (-5.7, -6.), (-4., -5.), (-2.1, -2.8)]);
            curve([(1.5, 2.6), (3.4, 5.), (4.9, 6.), (8.5, 6.)]);
            line(&[(5.4, -9.4), (8.8, -6.), (5.4, -2.6)]);
            line(&[(5.4, 2.6), (8.8, 6.), (5.4, 9.4)]);
        }
        "repeat" => {
            for sign in [-1., 1.] {
                curve([
                    (-8.5 * sign, 1.1 * sign),
                    (-8.5 * sign, -4.8 * sign),
                    (-5.7 * sign, -6.3 * sign),
                    (-1.5 * sign, -6.3 * sign),
                ]);
                line(&[(-1.7 * sign, -6.3 * sign), (7.2 * sign, -6.3 * sign)]);
                line(&[
                    (4.0 * sign, -9.5 * sign),
                    (7.5 * sign, -6.3 * sign),
                    (4.0 * sign, -3.1 * sign),
                ]);
            }
        }
        _ => unreachable!("transport glyph {name}"),
    }
}

fn hover_volume_open(
    icon: Rect,
    popup: Option<Rect>,
    pointer: Option<Pos2>,
    pinned: bool,
    dragging: bool,
    deadline: &mut Instant,
    now: Instant,
) -> bool {
    let inside = pointer
        .is_some_and(|p| icon.contains(p) || popup.is_some_and(|r| r.union(icon).contains(p)));
    if inside || (popup.is_some() && dragging) {
        *deadline = now + Duration::from_millis(220);
    }
    pinned || now < *deadline
}

fn vertical_volume(ui: &mut egui::Ui, volume: &mut f32, muted: &mut bool) -> bool {
    let (bubble, _) = ui.allocate_exact_size(vec2(42., 138.), Sense::hover());
    let body = Rect::from_min_size(bubble.min, vec2(42., 130.));
    let response = ui
        .interact(body, ui.id().with("volume-level"), Sense::click_and_drag())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let rail = Rect::from_center_size(body.center(), vec2(4., 98.));
    let before = (*volume, *muted);
    let primary = egui::PointerButton::Primary;
    if (response.clicked_by(primary) || response.dragged_by(primary))
        && let Some(pointer) = response.interact_pointer_pos()
    {
        response.request_focus();
        *volume = ((rail.bottom() - pointer.y) / rail.height()).clamp(0., 1.);
        *muted = false;
    }
    if response.secondary_clicked() {
        *muted = !*muted;
    }
    if response.has_focus() {
        let (delta, toggle) = ui.input_mut(|input| {
            let key =
                |input: &mut egui::InputState, key| input.consume_key(egui::Modifiers::NONE, key);
            let delta = if key(input, egui::Key::ArrowDown) || key(input, egui::Key::ArrowLeft) {
                -0.02
            } else if key(input, egui::Key::ArrowUp) || key(input, egui::Key::ArrowRight) {
                0.02
            } else {
                0.
            };
            (delta, key(input, egui::Key::M))
        });
        if delta != 0. {
            *volume = (*volume + delta).clamp(0., 1.);
            *muted = false;
        }
        if toggle {
            *muted = !*muted;
        }
    }

    let level = if *muted { 0. } else { *volume };
    response.widget_info(|| egui::WidgetInfo::slider(ui.is_enabled(), f64::from(level), "Volume"));
    let background = Color32::from_rgb(35, 36, 39);
    ui.painter().rect_filled(body, 12, background);
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            body.center_bottom() + vec2(-12., -2.),
            body.center_bottom() + vec2(12., -2.),
            bubble.center_bottom() + vec2(2., 0.),
            bubble.center_bottom() + vec2(-2., 0.),
        ],
        background,
        Stroke::NONE,
    ));
    let thumb = pos2(rail.center().x, rail.bottom() - rail.height() * level);
    ui.painter().rect_filled(rail, 2, Color32::from_gray(60));
    ui.painter().rect_filled(
        Rect::from_min_max(pos2(rail.left(), thumb.y), rail.right_bottom()),
        2,
        Color32::WHITE,
    );
    ui.painter().rect_filled(
        Rect::from_center_size(thumb, vec2(16., 5.)),
        2,
        Color32::WHITE,
    );
    response.on_hover_text(format!(
        "{:.0}% · Right-click to {}",
        level * 100.,
        if *muted { "unmute" } else { "mute" }
    ));
    before != (*volume, *muted)
}

#[derive(Debug, PartialEq)]
enum LyricsAction {
    Tab(usize),
    Resync,
    Hide,
    Show,
    Close,
}

fn lyrics_header(
    ui: &mut egui::Ui,
    page: Rect,
    selected_tab: usize,
    hidden: bool,
    can_resync: bool,
    lyrics_status: &str,
) -> Option<LyricsAction> {
    let names = [
        "Similar tracks",
        "Credits",
        "Lyrics",
        "Resync lyrics",
        "Hide UI · Esc to restore",
        "Close lyrics",
    ];
    let rects = layout::lyrics_control_rects(page);
    if hidden {
        let r = rects[5];
        let response = ui
            .interact(r, Id::new("show-lyrics-controls"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let color = if response.hovered() {
            TEXT
        } else {
            Color32::from_white_alpha(95)
        };
        paint_icon(ui.painter(), r.shrink(6.), "eye", color);
        if response.on_hover_text("Show UI · Esc").clicked() {
            return Some(LyricsAction::Show);
        }
        return None;
    }
    for (i, name) in names.iter().enumerate() {
        let rect = rects[i];
        let enabled = i != 3 || can_resync;
        let response = ui.interact(
            rect,
            Id::new(("now-playing-tab", i)),
            if enabled {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        let selected = i == selected_tab && i < 3;
        let hover =
            ui.ctx()
                .animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.14);
        let fill = ui
            .ctx()
            .animate_bool_with_time(response.id.with("selected"), selected, 0.18);
        let fill = layout::ease(fill);
        let hover = layout::ease(hover);
        if i < 3 {
            ui.painter().rect_filled(
                rect,
                22,
                design::SURFACE
                    .gamma_multiply(hover)
                    .lerp_to_gamma(design::BRAND, fill),
            );
            let color =
                if response.hovered() { TEXT } else { GREY }.lerp_to_gamma(design::INK, fill);
            ui.painter().text(
                if i == 2 {
                    rect.left_center() + vec2(15., 0.)
                } else {
                    rect.center()
                },
                if i == 2 {
                    Align2::LEFT_CENTER
                } else {
                    Align2::CENTER_CENTER
                },
                *name,
                font(18., true),
                color,
            );
            if i == 2 {
                paint_icon(
                    ui.painter(),
                    Rect::from_center_size(rect.right_center() - vec2(21., 0.), vec2(25., 25.)),
                    "sparkle",
                    color,
                );
            }
        } else {
            paint_icon(
                ui.painter(),
                rect.shrink(6.),
                match i {
                    3 => "resync",
                    4 => "eye-off",
                    _ => "down",
                },
                if !enabled {
                    Color32::from_white_alpha(55)
                } else if response.hovered() {
                    TEXT
                } else {
                    GREY
                },
            );
        }
        let tooltip = if i == 2 && !lyrics_status.is_empty() {
            lyrics_status
        } else {
            name
        };
        if response.on_hover_text(tooltip).clicked() {
            return Some(match i {
                3 => LyricsAction::Resync,
                4 => LyricsAction::Hide,
                5 => LyricsAction::Close,
                _ => LyricsAction::Tab(i),
            });
        }
    }
    None
}

impl App {
    pub(super) fn signal_popover(&mut self, _ui: &mut egui::Ui, button: &egui::Response) {
        egui::Popup::from_toggle_button_response(button)
            .align(egui::emath::RectAlign::TOP_END)
            .gap(12.)
            .frame(
                Frame::new()
                    .fill(Color32::from_gray(27))
                    .corner_radius(18)
                    .inner_margin(22)
                    .stroke(Stroke::new(1_f32, Color32::from_white_alpha(22))),
            )
            .width(330.)
            .show(|ui| {
                ui.set_width(330.);
                ui.horizontal(|ui| {
                    paint_icon(
                        ui.painter(),
                        Rect::from_min_size(ui.cursor().min, vec2(26., 26.)),
                        "music",
                        ACCENT,
                    );
                    ui.add_space(36.);
                    label(ui, "Music information", 20., true, TEXT);
                });
                ui.add_space(14.);
                if let Some(a) = &self.audio {
                    label(
                        ui,
                        format!(
                            "{} · {}-bit · {:.1} kHz",
                            a.format.codec,
                            a.format.bit_depth,
                            a.format.sample_rate as f64 / 1000.
                        ),
                        18.,
                        true,
                        ACCENT,
                    );
                    ui.add_space(8.);
                    label(
                        ui,
                        format!(
                            "{} channels · {} kb/s",
                            a.format.channels,
                            a.format.encoded_bitrate / 1000
                        ),
                        16.,
                        false,
                        GREY,
                    );
                    label(
                        ui,
                        format!("Windows shared output · {} Hz", a.format.output_rate),
                        16.,
                        false,
                        GREY,
                    );
                    ui.add_space(12.);
                    ui.label(
                        RichText::new(
                            "Source quality is shown above. Windows shared output may resample.",
                        )
                        .font(font(14., false))
                        .color(GREY),
                    );
                } else {
                    label(
                        ui,
                        "Start a track to inspect its audio format.",
                        16.,
                        false,
                        GREY,
                    );
                }
                ui.add_space(12.);
                if ui.button("Audio settings").clicked() {
                    self.page = Page::Settings;
                    ui.close();
                }
            });
    }
    pub(super) fn transport_controls(&mut self, ui: &mut egui::Ui, player: Rect, center: f32) {
        for (index, name) in ["shuffle", "previous", "toggle", "next", "repeat"]
            .iter()
            .enumerate()
        {
            let rect = Rect::from_center_size(
                pos2(center + (index as f32 - 2.) * 62., player.top() + 46.),
                vec2(52., 52.),
            );
            let active = (*name == "shuffle" && self.shuffle_on)
                || (*name == "repeat" && self.repeat_mode > 0);
            let response = ui
                .interact(rect, Id::new(("transport-button", name)), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            let hover = ui.ctx().animate_bool_with_time(
                response.id.with("hover"),
                response.hovered(),
                if self.settings.animated_lyrics {
                    design::FAST / self.settings.motion_speed
                } else {
                    0.
                },
            );
            let hover = layout::ease(hover);
            if (*name != "toggle" && active) || hover > 0. {
                ui.painter().circle_filled(
                    rect.center(),
                    20.,
                    if active {
                        Color32::from_gray(52)
                    } else {
                        design::HOVER.gamma_multiply(hover)
                    },
                );
            }
            let glyph = if *name == "toggle" {
                if self.paused() { "play" } else { "pause" }
            } else {
                name
            };
            let scale = if *name == "toggle" {
                1.
            } else if response.is_pointer_button_down_on() {
                0.94
            } else {
                1. + hover * 0.04
            };
            paint_transport_glyph(
                ui.painter(),
                rect.center(),
                glyph,
                scale,
                if active || matches!(*name, "toggle" | "previous" | "next") {
                    Color32::WHITE
                } else {
                    Color32::from_gray(155)
                },
            );
            if *name == "repeat" && self.repeat_mode == 2 {
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    "1",
                    font(9., true),
                    TEXT,
                );
            }
            let tip = match *name {
                "shuffle" => "Shuffle",
                "previous" => "Previous",
                "next" => "Next",
                "toggle" => {
                    if self.paused() {
                        "Play"
                    } else {
                        "Pause"
                    }
                }
                _ => match self.repeat_mode {
                    0 => "Repeat off",
                    1 => "Repeat queue",
                    _ => "Repeat one",
                },
            };
            if response.on_hover_text(tip).clicked() {
                match *name {
                    "shuffle" => {
                        self.shuffle_on = !self.shuffle_on;
                        if self.shuffle_on {
                            use rand::seq::SliceRandom;
                            let mut q = self.queue.as_ref().clone();
                            let start = (self.index + 1).min(q.len());
                            q[start..].shuffle(&mut rand::thread_rng());
                            self.queue = Arc::new(q);
                        }
                    }
                    "previous" => self.previous(),
                    "next" => self.next(),
                    "toggle" => self.toggle(),
                    _ => self.repeat_mode = (self.repeat_mode + 1) % 3,
                }
            }
        }
    }

    pub(super) fn lyrics_controls(&mut self, ui: &mut egui::Ui, page: Rect) {
        let enabled = self.current.is_some()
            && !self.busy
            && (self.lyric_manual_scroll || !self.lyric_request.pending);
        match lyrics_header(
            ui,
            page,
            self.now_tab,
            self.lyrics_chrome_hidden,
            enabled,
            &self.lyrics_status,
        ) {
            Some(LyricsAction::Tab(i)) if i != self.now_tab => {
                self.now_tab = i;
                self.tab_started = Instant::now();
            }
            Some(LyricsAction::Resync) => {
                self.resync_lyric_view();
            }
            Some(LyricsAction::Hide) => self.lyrics_chrome_hidden = true,
            Some(LyricsAction::Show) => self.lyrics_chrome_hidden = false,
            Some(LyricsAction::Close) => self.page = Page::Home,
            _ => {}
        }
    }

    pub(in crate::ui) fn resync_lyric_view(&mut self) {
        self.now_tab = 2;
        self.tab_started = Instant::now();
        let was_manual = std::mem::take(&mut self.lyric_manual_scroll);
        self.lyric_entry_generation = self.generation;
        self.lyric_jump = (self.settings.animated_lyrics && self.lyrics.synced).then(|| {
            crate::ui::lyric_jump::Jump::new(self.lyric_scroll, 0., self.lyric_position())
        });
        self.lyric_last_position = None;
        self.lyric_velocity = 0.;
        if !was_manual {
            self.request_lyrics(true);
        }
    }

    fn apply_output_volume(&self) {
        if let Some(audio) = &self.audio {
            audio
                .sink
                .set_volume(if self.muted { 0. } else { self.settings.volume });
        }
    }

    pub(super) fn volume_popover(&mut self, button: &egui::Response) {
        let now = Instant::now();
        if button.clicked() {
            self.volume_pinned = !self.volume_pinned;
        }
        let pointer = button.ctx.pointer_hover_pos();
        let mut open = hover_volume_open(
            button.rect,
            self.volume_popup_rect,
            pointer,
            self.volume_pinned,
            button.ctx.input(|i| i.pointer.primary_down()),
            &mut self.volume_hover_deadline,
            now,
        );
        if open && !self.volume_pinned {
            button.ctx.request_repaint_after(Duration::from_millis(220));
        }
        if button.secondary_clicked() {
            self.muted = !self.muted;
            self.apply_output_volume();
        }
        let popup = egui::Popup::from_response(button)
            .open_bool(&mut open)
            .align(egui::emath::RectAlign::TOP)
            .gap(8.)
            .frame(Frame::NONE)
            .width(42.)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                if vertical_volume(ui, &mut self.settings.volume, &mut self.muted) {
                    self.apply_output_volume();
                    self.schedule_settings_save();
                }
            });
        if let Some(popup) = popup {
            self.volume_popup_rect = Some(popup.response.rect);
        } else {
            self.volume_popup_rect = None;
        }
        if !open {
            self.volume_pinned = false;
            self.volume_hover_deadline = now;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lyric_icons_hide_restore_and_refresh_with_real_widget_input() {
        let page = Rect::from_min_size(Pos2::ZERO, vec2(840., 600.));
        let rects = layout::lyrics_control_rects(page);
        let click = |position: Pos2, hidden, enabled| {
            let ctx = egui::Context::default();
            assert!(crate::ui::load_font(&ctx, ""));
            let mut action = None;
            for (frame, pressed) in [None, Some(true), Some(false)].into_iter().enumerate() {
                let mut events = vec![egui::Event::PointerMoved(position)];
                if let Some(pressed) = pressed {
                    events.push(egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(page),
                        time: Some(frame as f64 / 120.),
                        events,
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            action =
                                lyrics_header(ui, page, 2, hidden, enabled, "").or(action.take());
                        });
                    },
                );
            }
            action
        };
        assert_eq!(
            click(rects[4].center(), false, true),
            Some(LyricsAction::Hide)
        );
        assert_eq!(
            click(rects[5].center(), true, true),
            Some(LyricsAction::Show)
        );
        assert_eq!(
            click(rects[3].center(), false, true),
            Some(LyricsAction::Resync)
        );
        assert_eq!(click(rects[3].center(), false, false), None);
        assert_eq!(click(rects[3].center(), true, true), None);
    }

    #[test]
    fn vertical_volume_clicks_go_from_zero_at_bottom_to_one_at_top() {
        let ctx = egui::Context::default();
        let mut volume = 0.4;
        let mut muted = true;
        let mut frame = |events| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(220., 220.))),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default()
                        .frame(Frame::NONE)
                        .show(ctx, |ui| vertical_volume(ui, &mut volume, &mut muted));
                },
            );
            (volume, muted)
        };
        frame(vec![]);
        let mut click = |y, button| {
            let pos = pos2(21., y);
            for pressed in [true, false] {
                let result = frame(vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]);
                if !pressed {
                    return result;
                }
            }
            unreachable!()
        };
        assert_eq!(click(12., egui::PointerButton::Primary), (1., false));
        assert_eq!(click(120., egui::PointerButton::Primary), (0., false));
        assert_eq!(click(60., egui::PointerButton::Secondary), (0., true));
        assert_eq!(click(60., egui::PointerButton::Secondary), (0., false));
    }

    #[test]
    fn volume_hover_crosses_gap_keeps_dragging_and_dismisses_after_leaving() {
        let icon = Rect::from_min_size(pos2(200., 300.), vec2(38., 38.));
        let popup = Rect::from_min_size(pos2(80., 160.), vec2(270., 132.));
        let start = Instant::now();
        let mut deadline = start;
        assert!(hover_volume_open(
            icon,
            None,
            Some(icon.center()),
            false,
            false,
            &mut deadline,
            start
        ));
        assert!(hover_volume_open(
            icon,
            Some(popup),
            Some(pos2(219., 296.)),
            false,
            false,
            &mut deadline,
            start + Duration::from_millis(100)
        ));
        assert!(hover_volume_open(
            icon,
            Some(popup),
            Some(pos2(600., 150.)),
            false,
            true,
            &mut deadline,
            start + Duration::from_secs(1)
        ));
        assert!(!hover_volume_open(
            icon,
            Some(popup),
            Some(pos2(600., 150.)),
            false,
            false,
            &mut deadline,
            start + Duration::from_secs(2)
        ));
        assert!(hover_volume_open(
            icon,
            None,
            None,
            true,
            false,
            &mut deadline,
            start + Duration::from_secs(3)
        ));
    }
}
