use super::*;
impl App {
    pub(in crate::ui) fn reference_album(&mut self, ui: &mut egui::Ui, id: u64, title: &str) {
        let tracks = self
            .tracks
            .get(&format!("album:{id}"))
            .cloned()
            .unwrap_or_default();
        let first = tracks.first().cloned().unwrap_or_default();
        let key = format!("album:{id}");
        let info = self.albums.get(&id).cloned().unwrap_or_else(|| Album {
            id,
            title: if title.is_empty() {
                first.album.clone()
            } else {
                title.into()
            },
            artist: first.artist.clone(),
            cover: first.cover.clone(),
            ..Default::default()
        });
        let margin = (ui.available_width() * 0.033).clamp(24., 60.);
        catalog_canvas(ui, "album-body", margin, 740., |ui| {
            ui.add_space(24.);
            let cover_size = (ui.available_width() * 0.18).clamp(220., 315.);
            let (hero, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), cover_size), Sense::hover());
            let art = Track {
                cover: info.cover.clone(),
                ..Default::default()
            };
            at(
                ui,
                Rect::from_min_size(hero.min, vec2(cover_size, cover_size)),
                "album-cover",
                |ui| {
                    let image = self.cover(ui, &art, cover_size);
                    image.context_menu(|ui| {
                        self.collection_menu(ui, &first, &info.title, &info.artist, &key)
                    });
                },
            );
            let x = hero.left() + cover_size + 42.;
            at(
                ui,
                Rect::from_min_max(pos2(x, hero.top() + (cover_size * 0.3 - 44.)), hero.max),
                "album-information",
                |ui| {
                    ui.spacing_mut().item_spacing.y = 0.;
                    label(ui, &info.title, 42., true, TEXT);
                    ui.add_space(12.);
                    ui.horizontal(|ui| {
                        if !info.artist_picture.is_empty() {
                            self.cover(
                                ui,
                                &Track {
                                    cover: info.artist_picture.clone(),
                                    ..Default::default()
                                },
                                28.,
                            );
                        }
                        if ui
                            .add(
                                egui::Label::new(
                                    RichText::new(&info.artist)
                                        .font(font(18., true))
                                        .color(GREY),
                                )
                                .sense(Sense::click()),
                            )
                            .on_hover_text("Open artist")
                            .clicked()
                        {
                            self.open_track_artist(&first);
                        }
                    });
                    ui.add_space(30.);
                    label(
                        ui,
                        if self.settings.total_play_time {
                            format!(
                                "{} TRACKS  ({})",
                                tracks.len(),
                                clock(tracks.iter().map(|t| t.duration).sum::<u64>() as f64)
                            )
                        } else {
                            format!("{} TRACKS", tracks.len())
                        },
                        14.,
                        true,
                        GREY,
                    );
                    ui.add_space(14.);
                    ui.horizontal(|ui| {
                        label(ui, &info.year, 14., true, GREY);
                        if info.quality.contains("HI_RES") || info.quality.contains("HIRES") {
                            Frame::new()
                                .fill(Color32::from_rgb(35, 31, 4))
                                .corner_radius(10)
                                .inner_margin(9)
                                .show(ui, |ui| {
                                    label(ui, "MAX", 12., true, GOLD);
                                });
                        }
                    });
                },
            );
            ui.add_space(52.);
            let (actions, _) = ui.allocate_exact_size(
                vec2(
                    ui.available_width(),
                    if ui.available_width() < 850. {
                        140.
                    } else {
                        64.
                    },
                ),
                Sense::hover(),
            );
            self.large_play_actions(ui, actions, &tracks);
            let secondary_top = actions.top() + if actions.width() < 850. { 76. } else { 0. };
            let step = if ui.available_width() < 1050. {
                78.
            } else {
                116.
            };
            for (i, (name, glyph)) in [
                ("Add", "heart-outline"),
                ("Credits", "info"),
                ("Share", "share"),
                ("Report", "warning"),
                ("More", "dots"),
            ]
            .iter()
            .enumerate()
            {
                let rect = Rect::from_min_size(
                    pos2(actions.right() - step * (5 - i) as f32, secondary_top),
                    vec2(step - 8., 64.),
                );
                let response = action(
                    ui,
                    rect,
                    &format!("album-action-{name}"),
                    if *name == "Add" && self.collection.contains(&key) {
                        "heart"
                    } else {
                        glyph
                    },
                    if *name == "Add" && self.collection.contains(&key) {
                        "Added"
                    } else {
                        name
                    },
                    TEXT,
                    1.,
                );
                match *name {
                    "Add" => {
                        if response.clicked() {
                            self.collection_toggle(&key);
                        }
                    }
                    "Share" => {
                        if response.clicked() {
                            ui.ctx()
                                .copy_text(format!("https://tidal.com/browse/album/{id}"));
                            self.toast("Album link copied", false);
                        }
                    }
                    "Report" => {
                        if response.clicked() {
                            let _ = crate::auth::open_browser(
                                "https://support.tidal.com/hc/en-us/requests/new",
                            );
                        }
                    }
                    "Credits" => {
                        egui::Popup::menu(&response).show(|ui| {
                            ui.set_max_width(420.);
                            ui.label(&info.artist);
                            if !info.copyright.is_empty() {
                                ui.label(&info.copyright);
                            }
                            if ui.button("Full credits on TIDAL").clicked() {
                                let _ = crate::auth::open_browser(&format!(
                                    "https://tidal.com/browse/album/{id}"
                                ));
                                ui.close();
                            }
                        });
                    }
                    _ => {
                        egui::Popup::menu(&response).show(|ui| {
                            self.collection_menu(ui, &first, &info.title, &info.artist, &key)
                        });
                    }
                }
            }
            ui.add_space(55.);
            self.catalog_track_table(ui, tracks.clone(), None, true);
        });
    }
    pub(in crate::ui) fn large_play_actions(
        &mut self,
        ui: &mut egui::Ui,
        r: Rect,
        tracks: &Arc<Vec<Track>>,
    ) {
        for (i, name) in ["Play", "Shuffle"].iter().enumerate() {
            let width = if r.width() < 1050. { 160. } else { 210. };
            let rect =
                Rect::from_min_size(r.min + vec2(i as f32 * (width + 20.), 0.), vec2(width, 62.));
            let fill = if i == 0 {
                Color32::WHITE
            } else {
                Color32::from_gray(20)
            };
            let color = if i == 0 { Color32::BLACK } else { TEXT };
            ui.painter().rect_filled(rect, 32, fill);
            labeled_control(
                ui.painter(),
                rect,
                if i == 0 { "play" } else { "shuffle" },
                name,
                22.,
                color,
            );
            if ui
                .interact(rect, Id::new(("large-play", name)), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
                && !tracks.is_empty()
            {
                let mut list = tracks.as_ref().clone();
                if i == 1 {
                    use rand::seq::SliceRandom;
                    list.shuffle(&mut rand::thread_rng());
                }
                self.play_list(Arc::new(list), 0);
            }
        }
    }
}
