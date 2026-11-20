use super::*;
impl App {
    pub(in crate::ui) fn reference_album(&mut self, ui: &mut egui::Ui, id: u64, title: &str) {
        let key = format!("album:{id}");
        let tracks = self.tracks.get(&key).cloned().unwrap_or_default();
        let first = tracks.first().cloned().unwrap_or_default();
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
        let room = ui.available_rect_before_wrap();
        let margin = (room.width() * 0.034).clamp(24., 65.);
        let compact = ui.ctx().content_rect().height() < 700.;
        let top = design::TOOLBAR + if compact { 12. } else { 28. };
        let output = catalog_canvas(ui, &key, margin, 740., |ui| {
            ui.add_space(top);
            let cover_size = if compact {
                (ui.available_width() * 0.23).clamp(132., 174.)
            } else {
                (ui.available_width() * 0.26).clamp(172., 315.)
            };
            let (hero, _) = ui.allocate_exact_size(
                vec2(ui.available_width(), cover_size.max(154.)),
                Sense::hover(),
            );
            let art = Track {
                cover: info.cover.clone(),
                ..Default::default()
            };
            let backdrop = Rect::from_min_max(
                pos2(hero.left() - margin, hero.top() - top),
                pos2(hero.right() + margin, hero.bottom() + 115.),
            );
            if self.settings.show_covers
                && self.settings.cover_theme
                && self.settings.frosted_glass
                && ui.is_rect_visible(backdrop)
                && let Some(key) = [640, 320, 160, 80]
                    .into_iter()
                    .filter_map(|side| artwork_key(&art, side))
                    .find(|key| self.artwork.contains_key(key))
                && let Some(image) = self.artwork.get_mut(&key)
            {
                image.touched = self.frame;
                // Shade the resident cover in one draw. Center its crop behind the header.
                let mut fade = egui::Mesh::with_texture(image.texture.id());
                let opacity = (self.settings.effect_strength * 123.) as u8;
                let crop = (backdrop.height() / backdrop.width()).min(1.) * 0.5;
                for (pos, uv, alpha) in [
                    (backdrop.left_top(), pos2(0., 0.5 - crop), opacity),
                    (backdrop.right_top(), pos2(1., 0.5 - crop), opacity),
                    (backdrop.left_bottom(), pos2(0., 0.5 + crop), 0),
                    (backdrop.right_bottom(), pos2(1., 0.5 + crop), 0),
                ] {
                    fade.vertices.push(egui::epaint::Vertex {
                        pos,
                        uv,
                        color: Color32::from_white_alpha(alpha),
                    });
                }
                fade.add_triangle(0, 1, 2);
                fade.add_triangle(1, 3, 2);
                ui.painter().add(egui::Shape::mesh(fade));
            }
            at(
                ui,
                Rect::from_min_size(hero.min, vec2(cover_size, cover_size)),
                "album-cover",
                |ui| {
                    let image = self.cover_with_radius(ui, &art, cover_size, design::ART);
                    image.context_menu(|ui| {
                        self.collection_menu(ui, &first, &info.title, &info.artist, &key)
                    });
                },
            );
            let x = hero.left() + cover_size + if compact { 22. } else { 42. };
            at(
                ui,
                Rect::from_min_max(
                    pos2(x, hero.top() + if compact { 0. } else { cover_size * 0.21 }),
                    hero.max,
                ),
                "album-information",
                |ui| {
                    ui.spacing_mut().item_spacing.y = 0.;
                    label(
                        ui,
                        &info.title,
                        if compact { 29. } else { design::TITLE },
                        true,
                        TEXT,
                    );
                    ui.add_space(if compact { 6. } else { 12. });
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
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text("Open artist")
                            .clicked()
                        {
                            self.open_track_artist(&first);
                        }
                    });
                    ui.add_space(if compact { 12. } else { 30. });
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
                    ui.add_space(if compact { 5. } else { 14. });
                    ui.horizontal(|ui| {
                        label(ui, &info.year, 14., true, GREY);
                        if info.quality.contains("HI_RES") || info.quality.contains("HIRES") {
                            Frame::new()
                                .fill(Color32::from_rgb(35, 31, 4))
                                .corner_radius(10)
                                .inner_margin(if compact { 6 } else { 9 })
                                .show(ui, |ui| {
                                    label(ui, "MAX", 12., true, GOLD);
                                });
                        }
                    });
                },
            );
            ui.add_space(if compact { 14. } else { 54. });
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
                ("More", "dots"),
            ]
            .iter()
            .enumerate()
            {
                let rect = Rect::from_min_size(
                    pos2(actions.right() - step * (4 - i) as f32, secondary_top),
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
                            self.collection_menu(ui, &first, &info.title, &info.artist, &key);
                            if ui.button("Report album").clicked() {
                                let _ = crate::auth::open_browser(
                                    "https://support.tidal.com/hc/en-us/requests/new",
                                );
                                ui.close();
                            }
                        });
                    }
                }
            }
            ui.add_space(32.);
            ui.separator();
            ui.add_space(12.);
            self.catalog_track_table(ui, tracks.clone(), None, Some(id));
        });
        let toolbar = Rect::from_min_size(room.min, vec2(room.width(), design::TOOLBAR));
        let opacity = (output.state.offset.y / 120.).clamp(0., 1.);
        ui.painter().rect_filled(
            toolbar,
            0,
            Color32::from_black_alpha((opacity * 255.) as u8),
        );
        at(ui, toolbar, "album-toolbar", |ui| {
            self.reference_toolbar(ui)
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
                design::BRAND
            } else {
                design::SURFACE
            };
            let color = if i == 0 { design::INK } else { TEXT };
            let button = ui
                .interact(rect, Id::new(("large-play", name)), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            let body = if button.is_pointer_button_down_on() {
                rect.shrink(1.)
            } else {
                rect
            };
            ui.painter().rect_filled(
                body,
                31,
                if button.hovered() {
                    fill.lerp_to_gamma(design::HOVER, 0.18)
                } else {
                    fill
                },
            );
            labeled_control(
                ui.painter(),
                body,
                if i == 0 { "play" } else { "shuffle" },
                name,
                22.,
                color,
            );
            if button.clicked() && !tracks.is_empty() {
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
