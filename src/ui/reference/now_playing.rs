use super::*;

impl App {
    pub(super) fn now_playing_panel(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let Some(track) = self.current.clone() else {
            return;
        };
        let progress = if self.settings.animated_lyrics {
            (self.tab_started.elapsed().as_secs_f32() / 0.24).clamp(0., 1.)
        } else {
            1.
        };
        let eased = layout::ease(progress);
        let area = rect.translate(vec2(22. * (1. - eased), 0.));
        let mut panel = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("now-panel", self.now_tab))
                .max_rect(area),
        );
        panel.set_clip_rect(rect.intersect(ui.clip_rect()));
        panel.set_opacity(eased);
        if progress < 1. {
            ui.ctx().request_repaint();
        }
        if self.now_tab == 1 {
            if track.id > 0 && self.credits_requested.insert(track.id) {
                self.send(Command::Credits(track.clone()));
            }
            let result = self.credits.get(&track.id).cloned();
            egui::ScrollArea::vertical()
                .id_salt(("track-credits", track.id))
                .auto_shrink([false, false])
                .show(&mut panel, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.;
                    let content_height = result
                        .as_ref()
                        .and_then(|r| r.as_ref().ok())
                        .map(|groups| {
                            groups
                                .iter()
                                .map(|group| {
                                    21. + 7.
                                        + ui.painter()
                                            .layout(
                                                group.names.clone(),
                                                font(17., false),
                                                GREY,
                                                ui.available_width(),
                                            )
                                            .size()
                                            .y
                                        + 30.
                                })
                                .sum::<f32>()
                                - 30.
                        })
                        .unwrap_or(180.);
                    ui.add_space(((area.height() - content_height) * 0.5).max(24.));
                    match result {
                        Some(Ok(groups)) if !groups.is_empty() => {
                            for group in groups {
                                label(ui, &group.role, 17., true, TEXT);
                                ui.add_space(7.);
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(group.names)
                                            .font(font(17., false))
                                            .color(Color32::from_gray(180)),
                                    )
                                    .wrap(),
                                );
                                ui.add_space(30.);
                            }
                        }
                        Some(Ok(_)) => {
                            label(ui, "TIDAL has no credits for this track.", 18., false, GREY);
                        }
                        Some(Err(error)) => {
                            label(ui, "Credits could not be loaded", 20., true, TEXT);
                            ui.label(error);
                            if ui.button("Try again").clicked() {
                                self.credits_requested.remove(&track.id);
                            }
                        }
                        None => {
                            label(ui, "Loading credits…", 18., false, GREY);
                        }
                    }
                });
        } else {
            if track.id > 0 && self.related_requested.insert(track.id) {
                self.send(Command::Similar(track.id));
            }
            let result = self.related_tracks.get(&track.id).cloned();
            egui::ScrollArea::vertical()
                .id_salt(("similar-tracks", track.id))
                .auto_shrink([false, false])
                .show(&mut panel, |ui| {
                    ui.add_space(24.);
                    match result {
                        Some(Ok(tracks)) => {
                            for (i, t) in tracks.iter().enumerate() {
                                let (r, response) = ui.allocate_exact_size(
                                    vec2(ui.available_width(), 86.),
                                    Sense::click(),
                                );
                                if !ui.is_rect_visible(r) {
                                    continue;
                                }
                                design::row_hover(ui, &response, Color32::TRANSPARENT);
                                let art = at(
                                    ui,
                                    Rect::from_min_size(r.min + vec2(8., 8.), vec2(64., 64.)),
                                    ("similar-art", t.id),
                                    |ui| self.cover(ui, t, 64.),
                                );
                                at(
                                    ui,
                                    Rect::from_min_max(
                                        r.min + vec2(90., 17.),
                                        r.max - vec2(48., 0.),
                                    ),
                                    ("similar-title", t.id),
                                    |ui| {
                                        label(ui, &t.title, 18., true, TEXT);
                                        label(ui, &t.artist, 16., false, GREY);
                                    },
                                );
                                paint_icon(
                                    ui.painter(),
                                    Rect::from_center_size(
                                        r.right_center() - vec2(22., 0.),
                                        vec2(30., 30.),
                                    ),
                                    "play",
                                    TEXT,
                                );
                                if response.clicked() || art.clicked() {
                                    self.play_list(tracks.clone(), i);
                                }
                                response.context_menu(|ui| {
                                    self.track_menu(ui, t, i, &tracks, None, false)
                                });
                            }
                        }
                        Some(Err(error)) => {
                            label(ui, "Similar tracks could not be loaded", 20., true, TEXT);
                            ui.label(error);
                            if ui.button("Try again").clicked() {
                                self.related_requested.remove(&track.id);
                            }
                        }
                        None => {
                            label(ui, "Loading similar tracks…", 18., false, GREY);
                        }
                    }
                });
        }
    }
}
