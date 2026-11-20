use super::*;

#[derive(Clone, Copy)]
enum List {
    History,
    Upcoming,
    Later,
}

impl App {
    pub(in crate::ui) fn reference_queue(&mut self, ctx: &egui::Context) {
        let progress = layout::ease(ctx.animate_bool_with_time(
            Id::new("queue-panel-motion"),
            self.queue_open,
            if self.settings.animated_lyrics {
                0.2 / self.settings.motion_speed
            } else {
                0.
            },
        ));
        if progress <= 0. {
            return;
        }
        let screen = ctx.content_rect();
        let rect = layout::queue_rect(screen, reserved_player(ctx));
        egui::Area::new(Id::new("play-queue-panel"))
            .fixed_pos(rect.min + vec2(26. * (1. - progress), 0.))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.set_opacity(progress);
                if !self.queue_open {
                    ui.disable();
                }
                Frame::new()
                    .fill(Color32::from_gray(22))
                    .corner_radius(design::PANEL)
                    .stroke(Stroke::new(1_f32, design::SEPARATOR))
                    .inner_margin(22)
                    .shadow(egui::epaint::Shadow {
                        offset: [-4, 8],
                        blur: 28,
                        spread: 0,
                        color: Color32::from_black_alpha(100),
                    })
                    .show(ui, |ui| {
                        let inner = rect.size() - vec2(44., 44.);
                        ui.set_min_size(inner);
                        ui.set_max_size(inner);
                        ui.spacing_mut().item_spacing = vec2(8., 8.);
                        let (header, header_response) =
                            ui.allocate_exact_size(vec2(inner.x, 42.), Sense::hover());
                        header_response.on_hover_text(format!(
                            "{} remaining · {} tracks",
                            clock(crate::model::queue_remaining(
                                &self.queue,
                                self.index,
                                self.position()
                            )),
                            self.queue.len().saturating_sub(self.index)
                        ));
                        ui.painter().text(
                            header.left_center(),
                            Align2::LEFT_CENTER,
                            if self.show_saved_later {
                                "Saved for Later"
                            } else {
                                "Play queue"
                            },
                            font(design::SECTION, true),
                            Color32::WHITE,
                        );
                        for (name, dx, tip) in [
                            ("music-plus", 66., "Saved for Later"),
                            ("close", 14., "Close queue"),
                        ] {
                            let area = Rect::from_center_size(
                                header.right_center() - vec2(dx, 0.),
                                vec2(30., 30.),
                            );
                            paint_icon(
                                ui.painter(),
                                area.shrink(5.),
                                name,
                                if name == "music-plus" && self.show_saved_later {
                                    TEXT
                                } else {
                                    GREY
                                },
                            );
                            if ui
                                .interact(area, Id::new(("queue-header", name)), Sense::click())
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .on_hover_text(tip)
                                .clicked()
                            {
                                if name == "close" {
                                    self.queue_open = false;
                                } else {
                                    self.show_saved_later = !self.show_saved_later;
                                }
                            }
                        }
                        ui.add_space(14.);
                        egui::ScrollArea::vertical()
                            .id_salt("queue-panel-body")
                            .max_height((inner.y - 72.).max(80.))
                            .show(ui, |ui| {
                                if self.show_saved_later {
                                    let later = self.saved_later.clone();
                                    self.queue_rows(
                                        ui,
                                        later.clone(),
                                        0..later.len(),
                                        List::Later,
                                        (inner.y - 60.).max(80.),
                                    );
                                } else {
                                    let (heading, _) =
                                        ui.allocate_exact_size(vec2(inner.x, 30.), Sense::hover());
                                    ui.painter().text(
                                        heading.left_center(),
                                        Align2::LEFT_CENTER,
                                        "History",
                                        font(18., true),
                                        TEXT,
                                    );
                                    let clear = Rect::from_min_size(
                                        heading.right_top() - vec2(96., 0.),
                                        vec2(96., 30.),
                                    );
                                    let clear_b = ui
                                        .interact(
                                            clear,
                                            Id::new("clear-queue-history"),
                                            Sense::click(),
                                        )
                                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                                    if clear_b.hovered() {
                                        ui.painter().text(
                                            clear.right_center(),
                                            Align2::RIGHT_CENTER,
                                            "Clear history",
                                            font(13., false),
                                            GREY,
                                        );
                                    }
                                    if clear_b.clicked() {
                                        self.play_history = Arc::new(Vec::new());
                                    }
                                    let history_id = Id::new("queue-history-expanded");
                                    let expanded = ui
                                        .ctx()
                                        .data(|data| data.get_temp::<bool>(history_id))
                                        .unwrap_or(inner.y > 450.);
                                    let heading_button = ui
                                        .interact(
                                            Rect::from_min_max(
                                                heading.min,
                                                heading.max - vec2(108., 0.),
                                            ),
                                            history_id,
                                            Sense::click(),
                                        )
                                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                                        .on_hover_text(if expanded {
                                            "Collapse history"
                                        } else {
                                            "Show history"
                                        });
                                    if heading_button.clicked() {
                                        ui.ctx().data_mut(|data| {
                                            data.insert_temp(history_id, !expanded)
                                        });
                                    }
                                    paint_icon(
                                        ui.painter(),
                                        Rect::from_center_size(
                                            heading.left_center() + vec2(87., 0.),
                                            vec2(15., 15.),
                                        ),
                                        if expanded { "down" } else { "forward" },
                                        GREY,
                                    );
                                    if self.play_history.is_empty() {
                                        label(ui, "Songs you play appear here", 15., false, GREY);
                                        ui.add_space(16.);
                                    } else if expanded {
                                        let history = self.play_history.clone();
                                        let height = (inner.y * 0.46)
                                            .min(history.len() as f32 * 80.)
                                            .max(80.);
                                        self.queue_rows(
                                            ui,
                                            history.clone(),
                                            0..history.len(),
                                            List::History,
                                            height,
                                        );
                                        ui.add_space(16.);
                                    }
                                    let (source, _) =
                                        ui.allocate_exact_size(vec2(inner.x, 30.), Sense::hover());
                                    let playlist = self
                                        .playing_playlist
                                        .as_ref()
                                        .and_then(|id| self.playlists.iter().find(|p| p.id == *id))
                                        .cloned();
                                    let title = playlist
                                        .as_ref()
                                        .map(|p| p.title.as_str())
                                        .or_else(|| self.current.as_ref().map(|t| t.album.as_str()))
                                        .filter(|s| !s.is_empty())
                                        .unwrap_or("Your queue");
                                    let next_heading = format!("Next Up from: {title}");
                                    at(
                                        ui,
                                        Rect::from_min_max(source.min, source.max - vec2(56., 0.)),
                                        "queue-playing-from",
                                        |ui| {
                                            label(
                                                ui,
                                                format!("Playing from: {title}"),
                                                17.,
                                                true,
                                                TEXT,
                                            );
                                        },
                                    );
                                    if let Some(p) = playlist
                                        && ui
                                            .interact(
                                                source.shrink2(vec2(56., 0.)),
                                                Id::new("queue-source"),
                                                Sense::click(),
                                            )
                                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                                            .clicked()
                                    {
                                        self.page = Page::Playlist(p.id.clone());
                                        self.send(Command::Playlist(p.id));
                                        self.queue_open = false;
                                    }
                                    let clear = Rect::from_min_size(
                                        source.right_top() - vec2(48., 0.),
                                        vec2(48., 30.),
                                    );
                                    ui.painter().text(
                                        clear.center(),
                                        Align2::CENTER_CENTER,
                                        "Clear",
                                        font(17., true),
                                        GREY,
                                    );
                                    if ui
                                        .interact(
                                            clear,
                                            Id::new("clear-upcoming-queue"),
                                            Sense::click(),
                                        )
                                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                                        .clicked()
                                    {
                                        let keep = (self.index + 1).min(self.queue.len());
                                        Arc::make_mut(&mut self.queue).truncate(keep);
                                    }
                                    if let Some(current) = self.current.clone() {
                                        self.queue_row(ui, &current, "current", 0, None, true);
                                    }
                                    ui.add_space(44.);
                                    let start = (self.index + 1).min(self.queue.len());
                                    let queue = self.queue.clone();
                                    if start < queue.len() {
                                        label(ui, next_heading, 17., true, TEXT);
                                        let height =
                                            (rect.bottom() - 22. - ui.cursor().top()).max(48.);
                                        self.queue_rows(
                                            ui,
                                            queue.clone(),
                                            start..queue.len(),
                                            List::Upcoming,
                                            height,
                                        );
                                    } else if self.current.is_none() {
                                        ui.add_space(22.);
                                        label(ui, "Your queue is empty", 18., true, TEXT);
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(
                                                    "Play a song or choose Play next to add music.",
                                                )
                                                .font(font(16., false))
                                                .color(GREY),
                                            )
                                            .wrap(),
                                        );
                                    }
                                }
                            });
                    });
            });
        if self.queue_open && self.dialog.is_none() && !egui::Popup::is_any_open(ctx) {
            let dismiss =
                ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            let outside = ctx.input(|i| {
                i.pointer.primary_clicked()
                    && i.pointer.interact_pos().is_some_and(|p| {
                        !rect.contains(p) && !self.queue_button_rect.is_some_and(|r| r.contains(p))
                    })
            });
            let docked = self.page != Page::Lyrics && layout::queue_docked(screen);
            if dismiss || (outside && !docked) {
                self.queue_open = false;
            }
        }
    }

    fn queue_rows(
        &mut self,
        ui: &mut egui::Ui,
        tracks: Arc<Vec<Track>>,
        range: std::ops::Range<usize>,
        kind: List,
        height: f32,
    ) {
        let key = match kind {
            List::History => "history",
            List::Upcoming => "upcoming",
            List::Later => "later",
        };
        egui::ScrollArea::vertical()
            .id_salt(("queue-list", key))
            .max_height(height)
            .auto_shrink([false, true])
            .stick_to_bottom(matches!(kind, List::History))
            .show_rows(ui, 80., range.len(), |ui, visible| {
                for relative in visible {
                    let index = range.start + relative;
                    let track = &tracks[index];
                    if self.queue_row(ui, track, key, index, Some(&tracks), false) {
                        match kind {
                            List::Upcoming => {
                                self.index = index;
                                self.play(track.clone());
                            }
                            List::History => self.play(track.clone()),
                            List::Later => self.play_list(tracks.clone(), index),
                        }
                    }
                }
            });
    }

    fn queue_row(
        &mut self,
        ui: &mut egui::Ui,
        track: &Track,
        list: &str,
        index: usize,
        tracks: Option<&Arc<Vec<Track>>>,
        current: bool,
    ) -> bool {
        let (r, response) = ui.allocate_exact_size(vec2(ui.available_width(), 72.), Sense::click());
        if !ui.is_rect_visible(r) {
            self.prefetch_cover(track, 58., ui.ctx().pixels_per_point());
            return false;
        }
        design::row_hover(ui, &response, Color32::TRANSPARENT);
        let cover = Rect::from_min_size(r.min + vec2(2., 6.), vec2(58., 58.));
        let art = at(ui, cover, ("queue-cover", list, index), |ui| {
            self.cover(ui, track, 58.)
        });
        if ui.rect_contains_pointer(cover) {
            ui.painter()
                .rect_filled(cover, 4, Color32::from_black_alpha(85));
            paint_icon(
                ui.painter(),
                cover.shrink(16.),
                if current && !self.paused() {
                    "pause"
                } else {
                    "play"
                },
                TEXT,
            );
        }
        at(
            ui,
            Rect::from_min_max(r.min + vec2(76., 8.), r.max - vec2(38., 0.)),
            ("queue-title", list, index),
            |ui| {
                label(
                    ui,
                    &track.title,
                    18.,
                    true,
                    if current {
                        ACCENT
                    } else if list == "history" {
                        GREY
                    } else {
                        TEXT
                    },
                );
                ui.add_space(3.);
                label(ui, &track.artist, 17., false, GREY);
            },
        );
        let area = Rect::from_center_size(r.right_center() - vec2(11., 0.), vec2(28., 32.));
        paint_icon(
            ui.painter(),
            area.shrink(6.),
            if list == "upcoming" { "close" } else { "dots" },
            if list == "upcoming" { TEXT } else { GREY },
        );
        let menu = ui
            .interact(area, Id::new(("queue-menu", list, index)), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(if list == "upcoming" {
                "Remove from queue"
            } else {
                "More"
            });
        if list == "upcoming" && menu.clicked() {
            Arc::make_mut(&mut self.queue).remove(index);
            return false;
        }
        let one;
        let source = if let Some(source) = tracks {
            source
        } else {
            one = Arc::new(vec![track.clone()]);
            &one
        };
        if list != "upcoming" {
            egui::Popup::menu(&menu).show(|ui| {
                self.track_menu(
                    ui,
                    track,
                    index.min(source.len().saturating_sub(1)),
                    source,
                    None,
                    false,
                );
                if list == "later" && ui.button("Remove from Saved for Later").clicked() {
                    Arc::make_mut(&mut self.saved_later).remove(index);
                    if let Err(error) = crate::config::write_json(
                        &data_dir().join("saved-later.json"),
                        &*self.saved_later,
                    ) {
                        self.toast(error.to_string(), true);
                    }
                    ui.close();
                }
            });
        }
        response.context_menu(|ui| {
            self.track_menu(
                ui,
                track,
                index.min(source.len().saturating_sub(1)),
                source,
                None,
                false,
            )
        });
        let clicked = response.clicked() || art.clicked();
        if current && clicked {
            self.toggle();
            false
        } else {
            clicked
        }
    }
}
