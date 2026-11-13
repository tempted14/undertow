use super::*;

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
                24,
                Color32::from_white_alpha((fill * 255. + (1. - fill) * hover * 12.) as u8),
            );
            let color = Color32::from_gray(
                ((if response.hovered() { 240. } else { 150. }) * (1. - fill)).round() as u8,
            );
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
        if response.on_hover_text(*name).clicked() {
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
                0.14,
            );
            let hover = layout::ease(hover);
            if active || hover > 0. {
                ui.painter().circle_filled(
                    rect.center(),
                    20.,
                    Color32::from_white_alpha(if active { 20 } else { (hover * 15.) as u8 }),
                );
            }
            let glyph = if *name == "toggle" {
                if self.paused() { "play" } else { "pause" }
            } else {
                name
            };
            let visual = Rect::from_center_size(
                rect.center(),
                if *name == "toggle" {
                    vec2(52., 52.)
                } else {
                    vec2(34., 34.)
                },
            )
            .expand(if response.is_pointer_button_down_on() {
                -1.5
            } else {
                hover * 0.6
            });
            paint_icon(
                ui.painter(),
                visual,
                glyph,
                if active || matches!(*name, "toggle" | "previous" | "next") {
                    TEXT
                } else {
                    GREY
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
        match lyrics_header(ui, page, self.now_tab, self.lyrics_chrome_hidden, enabled) {
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
        self.lyric_entry_generation = u64::MAX;
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
        let frame = Frame::new()
            .fill(Color32::from_rgb(23, 25, 29))
            .corner_radius(18)
            .stroke(Stroke::new(1_f32, Color32::from_white_alpha(24)))
            .inner_margin(18);
        let popup = egui::Popup::from_response(button)
            .open_bool(&mut open)
            .align(egui::emath::RectAlign::TOP)
            .gap(8.)
            .frame(frame)
            .width(232.)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_width(232.);
                ui.spacing_mut().item_spacing.y = 10.;
                let (head, _) = ui.allocate_exact_size(vec2(232., 30.), Sense::hover());
                let mute = Rect::from_center_size(
                    pos2(head.left() + 15., head.center().y),
                    vec2(30., 30.),
                );
                paint_icon(
                    ui.painter(),
                    mute.shrink(4.),
                    "volume",
                    if self.muted { GREY } else { TEXT },
                );
                if self.muted {
                    ui.painter().line_segment(
                        [
                            mute.left_top() + vec2(4., 4.),
                            mute.right_bottom() - vec2(4., 4.),
                        ],
                        Stroke::new(1.5_f32, TEXT),
                    );
                }
                if ui
                    .interact(mute, Id::new("volume-popup-mute"), Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(if self.muted { "Unmute" } else { "Mute" })
                    .clicked()
                {
                    self.muted = !self.muted;
                    self.apply_output_volume();
                }
                ui.painter().text(
                    pos2(head.left() + 42., head.center().y),
                    Align2::LEFT_CENTER,
                    "Volume",
                    font(16., true),
                    TEXT,
                );
                ui.painter().text(
                    head.right_center(),
                    Align2::RIGHT_CENTER,
                    format!(
                        "{:.0}%",
                        if self.muted {
                            0.
                        } else {
                            self.settings.volume * 100.
                        }
                    ),
                    font(14., true),
                    GREY,
                );
                let (track, response) =
                    ui.allocate_exact_size(vec2(232., 32.), Sense::click_and_drag());
                let rail = Rect::from_center_size(track.center(), vec2(216., 5.));
                let old = self.settings.volume;
                if response.clicked() || response.dragged() {
                    response.request_focus();
                    if let Some(pointer) = response.interact_pointer_pos() {
                        self.settings.volume =
                            ((pointer.x - rail.left()) / rail.width()).clamp(0., 1.);
                        self.muted = false;
                    }
                }
                if response.has_focus() {
                    let delta = ui.input_mut(|i| {
                        if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft) {
                            -0.02
                        } else if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight) {
                            0.02
                        } else {
                            0.
                        }
                    });
                    if delta != 0. {
                        self.settings.volume = (self.settings.volume + delta).clamp(0., 1.);
                        self.muted = false;
                    }
                }
                let value = if self.muted { 0. } else { self.settings.volume };
                ui.painter().rect_filled(rail, 3, Color32::from_gray(60));
                ui.painter().rect_filled(
                    Rect::from_min_max(
                        rail.min,
                        pos2(rail.left() + rail.width() * value, rail.bottom()),
                    ),
                    3,
                    ACCENT,
                );
                let thumb = pos2(rail.left() + rail.width() * value, rail.center().y);
                ui.painter()
                    .circle_filled(thumb, if response.dragged() { 8. } else { 6.5 }, TEXT);
                if (self.settings.volume - old).abs() > f32::EPSILON {
                    self.apply_output_volume();
                }
                if response.drag_stopped() || response.clicked() {
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
                            action = lyrics_header(ui, page, 2, hidden, enabled).or(action.take());
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
