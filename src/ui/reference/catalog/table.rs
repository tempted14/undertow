use super::*;

impl App {
    pub(in crate::ui) fn catalog_track_table(
        &mut self,
        ui: &mut egui::Ui,
        tracks: Arc<Vec<Track>>,
        playlist: Option<&Playlist>,
        album: bool,
    ) {
        let key = if album {
            format!("album:{}", tracks.first().map(|t| t.album_id).unwrap_or(0))
        } else {
            playlist
                .map(|p| p.id.clone())
                .unwrap_or_else(|| "favorites".into())
        };
        if self.playlist_filter_id != key {
            self.playlist_filter_id = key.clone();
            self.playlist_index.invalidate();
            self.playlist_scroll_reset = true;
            self.playlist_filter.clear();
            self.playlist_sort = None;
        }
        if !self.playlist_index.matches(&tracks) {
            self.rebuild_playlist_filter(&tracks);
        }
        let height = ui.available_height().max(100.);
        // Preserve readable type and all signal columns when the window gets narrower.
        egui::ScrollArea::horizontal()
            .id_salt(("track-columns", &key))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let w = ui.available_width().max(if album { 1200. } else { 1180. });
                ui.set_min_width(w);
                let mut columns: Vec<(usize, &str, f32)> = if album {
                    vec![
                        (0, "#", 0.),
                        (1, "TITLE", 0.032),
                        (2, "ARTIST", 0.47),
                        (5, "BITRATE", 0.645),
                        (6, "SAMPLE RATE", 0.722),
                        (7, "DEPTH", 0.81),
                        (8, "TIME", 0.86),
                    ]
                } else {
                    vec![
                        (0, "#", 0.),
                        (1, "TITLE", 0.032),
                        (2, "ARTIST", 0.265),
                        (3, "ALBUM", 0.43),
                        (4, "DATE ADDED", 0.54),
                        (5, "BITRATE", 0.645),
                        (6, "SAMPLE RATE", 0.72),
                        (7, "DEPTH", 0.81),
                        (8, "TIME", 0.86),
                    ]
                };
                if !self.settings.show_quality {
                    columns.retain(|(column, _, _)| !(5..=7).contains(column));
                }
                let collection = playlist.is_none() && !album;
                let (header, _) = ui.allocate_exact_size(vec2(w, 44.), Sense::hover());
                for (position, (col, name, fraction)) in columns.iter().enumerate() {
                    let right = columns.get(position + 1).map_or(w - 92., |(_, _, f)| w * f);
                    let rect = Rect::from_min_size(
                        header.min + vec2(w * fraction, 0.),
                        vec2(right - w * fraction, 40.),
                    );
                    ui.painter().text(
                        rect.min + vec2(if *col == 0 { 13. } else { 0. }, 10.),
                        Align2::LEFT_TOP,
                        *name,
                        font(14., true),
                        GREY,
                    );
                    if self.playlist_sort.is_some_and(|(c, _)| c == *col) {
                        paint_icon(
                            ui.painter(),
                            Rect::from_min_size(
                                pos2(rect.right() - 18., rect.top() + 8.),
                                vec2(16., 16.),
                            ),
                            if self.playlist_sort.is_some_and(|(_, d)| d) {
                                "down"
                            } else {
                                "up"
                            },
                            AQUA,
                        );
                    }
                    if ui
                        .interact(rect, Id::new(("track-sort", &key, col)), Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        self.playlist_sort = Some((
                            *col,
                            self.playlist_sort.is_some_and(|(c, d)| c == *col && !d),
                        ));
                        self.rebuild_playlist_filter(&tracks);
                        self.playlist_scroll_reset = true;
                    }
                }
                // Apply a header click immediately, without painting one stale frame.
                let indices = self.playlist_filtered.clone();
                ui.spacing_mut().item_spacing.y = 0.;
                if indices.is_empty() {
                    ui.add_space(36.);
                    ui.vertical_centered(|ui| {
                        let filtering = !self.playlist_filter.trim().is_empty();
                        label(
                            ui,
                            if filtering {
                                "No matching tracks"
                            } else {
                                "No tracks to display"
                            },
                            22.,
                            true,
                            TEXT,
                        );
                        ui.add_space(8.);
                        label(
                            ui,
                            if filtering {
                                "Try another title, artist or album."
                            } else {
                                "Tracks will appear here when available."
                            },
                            16.,
                            false,
                            GREY,
                        );
                        if !self.playlist_filter.is_empty() {
                            ui.add_space(18.);
                            if pill(ui, "Clear filter", false).clicked() {
                                self.playlist_filter.clear();
                                self.rebuild_playlist_filter(&tracks);
                                self.playlist_scroll_reset = true;
                            }
                        }
                    });
                    ui.add_space(36.);
                    return;
                }
                let mut rows_area = egui::ScrollArea::vertical().id_salt(("readable-tracks", &key));
                if std::mem::take(&mut self.playlist_scroll_reset) {
                    rows_area = rows_area.vertical_scroll_offset(0.);
                }
                if let Some(offset) = self.diagnostic_scroll_offset(indices.len()) {
                    rows_area = rows_area.vertical_scroll_offset(offset);
                }
                rows_area
                    .max_height((height - 52.).max(90.))
                    .auto_shrink([false, false])
                    .show_rows(ui, 64., indices.len(), |ui, range| {
                        if !album {
                            for display in
                                range.start.saturating_sub(6)..(range.end + 10).min(indices.len())
                            {
                                if !range.contains(&display)
                                    && let Some(track) =
                                        indices.get(display).and_then(|i| tracks.get(*i))
                                {
                                    self.prefetch_cover(track, 54., ui.ctx().pixels_per_point());
                                }
                            }
                        }
                        for display in range {
                            let index = indices[display];
                            let t = &tracks[index];
                            let (row, response) =
                                ui.allocate_exact_size(vec2(w, 64.), Sense::click());
                            if !ui.is_rect_visible(row) {
                                if !album {
                                    self.prefetch_cover(t, 54., ui.ctx().pixels_per_point());
                                }
                                continue;
                            }
                            let playing = self.current.as_ref().is_some_and(|c| c.id == t.id)
                                && self.audio.is_some();
                            if response.hovered() || playing {
                                ui.painter().rect_filled(
                                    row,
                                    4,
                                    Color32::from_white_alpha(if playing { 14 } else { 8 }),
                                );
                            }
                            let color = if playing { AQUA } else { TEXT };
                            let leading_play = collection || album;
                            let show_leading_play = response.hovered() || playing;
                            if !leading_play || !show_leading_play {
                                ui.painter().text(
                                    row.min + vec2(15., 32.),
                                    Align2::CENTER_CENTER,
                                    (if collection { display + 1 } else { index + 1 }).to_string(),
                                    font(17., false),
                                    GREY,
                                );
                            }
                            let title_x = if album {
                                w * 0.032
                            } else {
                                w * 0.032 + if collection { 68. } else { 104. }
                            };
                            let art_click = if !album {
                                let art = at(
                                    ui,
                                    Rect::from_min_size(
                                        row.min
                                            + vec2(
                                                w * 0.032 + if collection { 0. } else { 32. },
                                                5.,
                                            ),
                                        vec2(54., 54.),
                                    ),
                                    ("track-art", &key, index),
                                    |ui| self.cover(ui, t, 54.),
                                );
                                let clicked = art.clicked();
                                art.context_menu(|ui| {
                                    self.track_menu(
                                        ui,
                                        t,
                                        index,
                                        &tracks,
                                        playlist.map(|p| p.id.as_str()),
                                        playlist.is_some_and(|p| p.owned),
                                    )
                                });
                                clicked
                            } else {
                                false
                            };
                            at(
                                ui,
                                Rect::from_min_max(
                                    row.min + vec2(title_x, 17.),
                                    pos2(
                                        row.left() + w * if album { 0.35 } else { 0.255 },
                                        row.bottom() - 4.,
                                    ),
                                ),
                                ("track-title", &key, index),
                                |ui| self.track_title(ui, t, color),
                            );
                            let actual = self.audio.as_ref().filter(|_| playing);
                            let depth = actual.map(|a| a.format.bit_depth).or(t.bit_depth);
                            let sample = actual.map(|a| a.format.sample_rate).or(t.sample_rate);
                            let rate = actual
                                .map(|a| a.format.encoded_bitrate)
                                .filter(|n| *n > 0)
                                .or(t.bitrate);
                            let tint = if depth.is_some_and(|n| n > 16) {
                                GOLD
                            } else {
                                AQUA
                            };
                            for (position, (col, _, f)) in columns.iter().enumerate() {
                                if *col < 2 {
                                    continue;
                                }
                                let text = match col {
                                    2 => t.artist.clone(),
                                    3 => t.album.clone(),
                                    4 => {
                                        if collection {
                                            super::super::playlist::relative_date_label(
                                                &t.date_added,
                                            )
                                        } else {
                                            date_label(&t.date_added)
                                        }
                                    }
                                    5 => bitrate_label(rate),
                                    6 => sample_label(sample),
                                    7 => depth.map(|n| format!("{n}bit")).unwrap_or_default(),
                                    8 => clock(t.duration as f64),
                                    _ => String::new(),
                                };
                                // The rightmost cell ends before the fixed-position add button.
                                let right = columns
                                    .get(position + 1)
                                    .map_or(w - 92., |(_, _, f)| w * f - 12.);
                                at(
                                    ui,
                                    Rect::from_min_size(
                                        row.min + vec2(w * f, 19.),
                                        vec2(right - w * f, 30.),
                                    ),
                                    ("track-cell", &key, index, col),
                                    |ui| {
                                        let response = if *col == 2 || *col == 3 {
                                            text_link(
                                                ui,
                                                text,
                                                18.,
                                                true,
                                                TEXT,
                                                *col == 2 || t.album_id > 0,
                                            )
                                        } else {
                                            label(
                                                ui,
                                                text,
                                                18.,
                                                true,
                                                if (5..=7).contains(col) { tint } else { TEXT },
                                            )
                                        };
                                        if *col == 2 {
                                            if response.on_hover_text("Open artist").clicked() {
                                                self.open_track_artist(t);
                                            }
                                        } else if *col == 3
                                            && response.on_hover_text("Open album").clicked()
                                        {
                                            self.open_album(t.album_id, t.album.clone());
                                        }
                                    },
                                );
                            }
                            let play = Rect::from_center_size(
                                pos2(
                                    if leading_play {
                                        row.left() + 15.
                                    } else {
                                        row.left() + 44.
                                    },
                                    row.center().y,
                                ),
                                vec2(30., 34.),
                            );
                            if !leading_play || show_leading_play {
                                paint_icon(
                                    ui.painter(),
                                    play.shrink(2.),
                                    if playing
                                        && self.audio.as_ref().is_some_and(|a| !a.sink.is_paused())
                                    {
                                        "pause"
                                    } else {
                                        "play"
                                    },
                                    if playing { AQUA } else { TEXT },
                                );
                            }
                            let play_clicked = ui
                                .interact(play, Id::new(("row-play", &key, index)), Sense::click())
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .on_hover_text("Play this track")
                                .clicked();
                            let plus = at(
                                ui,
                                Rect::from_center_size(
                                    pos2(row.right() - 73., row.center().y),
                                    vec2(30., 30.),
                                ),
                                ("row-add", &key, index),
                                |ui| icon(ui, "plus", 28., GREY),
                            );
                            egui::Popup::menu(&plus).show(|ui| {
                                ui.label("Add to playlist");
                                for p in self.playlists.clone().iter().filter(|p| p.owned) {
                                    if ui.button(&p.title).clicked() {
                                        self.send(Command::Add(p.id.clone(), t.id));
                                        ui.close();
                                    }
                                }
                            });
                            let heart = Rect::from_center_size(
                                pos2(row.right() - 28., row.center().y),
                                vec2(25., 25.),
                            );
                            let saved = self.favorite_ids.contains(&t.id);
                            if saved {
                                paint_icon(ui.painter(), heart, "heart", TEXT);
                            } else {
                                outline_heart(ui.painter(), heart, GREY);
                            }
                            if ui
                                .interact(
                                    heart,
                                    Id::new(("row-heart", &key, index)),
                                    Sense::click(),
                                )
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                self.send(Command::Favorite(t.id, !saved));
                                self.send(Command::Favorites);
                            }
                            if play_clicked && playing {
                                if let Some(a) = &self.audio {
                                    if a.sink.is_paused() {
                                        a.sink.play();
                                    } else {
                                        a.sink.pause();
                                    }
                                }
                            } else if play_clicked || art_click || response.double_clicked() {
                                self.play_list(
                                    Arc::new(indices.iter().map(|&i| tracks[i].clone()).collect()),
                                    display,
                                );
                            }
                            response.context_menu(|ui| {
                                self.track_menu(
                                    ui,
                                    t,
                                    index,
                                    &tracks,
                                    playlist.map(|p| p.id.as_str()),
                                    playlist.is_some_and(|p| p.owned),
                                )
                            });
                        }
                    });
            });
    }
}
