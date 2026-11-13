use super::*;

#[derive(Clone)]
pub(in crate::ui) enum ListAction {
    Play,
    Next,
    Download,
    Later,
    Add(String),
}

fn menu_style(ui: &mut egui::Ui) {
    ui.set_width(314.);
    ui.spacing_mut().item_spacing = Vec2::ZERO;
    ui.style_mut().visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    ui.style_mut().visuals.widgets.hovered.weak_bg_fill = Color32::from_gray(61);
    ui.style_mut().visuals.widgets.active.weak_bg_fill = Color32::from_gray(68);
    ui.style_mut().spacing.interact_size.y = 54.;
    ui.style_mut().spacing.button_padding = vec2(14., 16.);
    ui.style_mut().override_font_id = Some(font(16., true));
}

fn menu_row(ui: &mut egui::Ui, title: &str, enabled: bool) -> egui::Response {
    let (r, response) = ui.allocate_exact_size(
        vec2(314., 55.),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let hover = ui.ctx().animate_bool_with_time(
        response.id.with("menu-hover"),
        response.hovered() && enabled,
        0.12,
    );
    let hover = layout::ease(hover);
    ui.painter()
        .rect_filled(r, 4, Color32::from_white_alpha((hover * 20.) as u8));
    ui.painter().text(
        r.left_center() + vec2(14., 0.),
        Align2::LEFT_CENTER,
        title,
        font(16., true),
        if enabled { TEXT } else { GREY },
    );
    response
}

impl App {
    pub(in crate::ui) fn list_action(&mut self, key: String, action: ListAction) {
        self.pending_list = None;
        if let Some(tracks) = self.tracks.get(&key).cloned() {
            self.apply_list_action(action, tracks);
        } else {
            self.pending_list = Some((key.clone(), action));
            if let Some(id) = key.strip_prefix("album:").and_then(|s| s.parse().ok()) {
                self.send(Command::Album(id));
            } else if let Some(id) = key.strip_prefix("mix:") {
                self.send(Command::Mix(id.into()));
            } else if key == "favorites" {
                self.send(Command::Favorites);
            } else {
                self.send(Command::Playlist(key));
            }
        }
    }

    pub(in crate::ui) fn apply_list_action(&mut self, action: ListAction, tracks: Arc<Vec<Track>>) {
        if tracks.is_empty() {
            self.toast("There are no tracks available", true);
            return;
        }
        match action {
            ListAction::Play => self.play_list(tracks, 0),
            ListAction::Next => {
                if self.current.is_none() {
                    self.play_list(tracks, 0);
                } else {
                    let at = (self.index + 1).min(self.queue.len());
                    Arc::make_mut(&mut self.queue).splice(at..at, tracks.iter().cloned());
                    self.toast("Playing next", false);
                }
            }
            ListAction::Download => {
                self.download_active = true;
                self.send(Command::DownloadMany(tracks));
            }
            ListAction::Later => {
                let list = Arc::make_mut(&mut self.saved_later);
                for track in tracks.iter() {
                    if !list
                        .iter()
                        .any(|t| t.id == track.id && t.local_path == track.local_path)
                    {
                        list.push(track.clone());
                    }
                }
                match crate::config::write_json(&data_dir().join("saved-later.json"), &*list) {
                    Ok(()) => {
                        self.toast("Saved for later on this device · find it in Queue", false)
                    }
                    Err(e) => self.toast(format!("Could not save for later: {e}"), true),
                }
            }
            ListAction::Add(id) => self.send(Command::AddMany(id, tracks)),
        }
    }

    pub(in crate::ui) fn collection_toggle(&mut self, key: &str) {
        if self.collection_pending.contains(key) {
            return;
        }
        self.collection_pending.insert(key.to_owned());
        self.send(Command::Collection(
            key.to_owned(),
            !self.collection.contains(key),
        ));
    }

    fn menu_header(&mut self, ui: &mut egui::Ui, art: &Track, title: &str, subtitle: &str) {
        menu_style(ui);
        let (r, _) = ui.allocate_exact_size(vec2(314., 92.), Sense::hover());
        at(
            ui,
            Rect::from_min_size(r.min + vec2(14., 14.), vec2(48., 48.)),
            ("menu-art", art.id),
            |ui| {
                self.cover(ui, art, 48.);
            },
        );
        at(
            ui,
            Rect::from_min_max(r.min + vec2(79., 13.), r.max - vec2(10., 20.)),
            "menu-heading",
            |ui| {
                label(ui, title, 18., true, TEXT);
                ui.add_space(4.);
                label(ui, subtitle, 16., true, Color32::from_gray(174));
            },
        );
    }

    fn add_to_playlist_menu(
        &mut self,
        ui: &mut egui::Ui,
        key: Option<&str>,
        tracks: &Arc<Vec<Track>>,
    ) {
        egui::containers::menu::SubMenuButton::from_button(
            egui::Button::new("Add to playlist")
                .min_size(vec2(314., 55.))
                .right_text("›"),
        )
        .ui(ui, |ui| {
            menu_style(ui);
            let lists = self
                .playlists
                .iter()
                .filter(|p| p.owned)
                .cloned()
                .collect::<Vec<_>>();
            if lists.is_empty() {
                menu_row(ui, "No editable playlists", false);
            }
            egui::ScrollArea::vertical()
                .max_height(390.)
                .show(ui, |ui| {
                    for playlist in lists {
                        if menu_row(ui, &playlist.title, true).clicked() {
                            if let Some(key) = key {
                                self.list_action(key.into(), ListAction::Add(playlist.id));
                            } else {
                                self.apply_list_action(
                                    ListAction::Add(playlist.id),
                                    tracks.clone(),
                                );
                            }
                            ui.close();
                        }
                    }
                });
        });
    }

    fn share_menu(&mut self, ui: &mut egui::Ui, url: String) {
        egui::containers::menu::SubMenuButton::from_button(
            egui::Button::new("Share")
                .min_size(vec2(314., 55.))
                .right_text("›"),
        )
        .ui(ui, |ui| {
            menu_style(ui);
            if menu_row(ui, "Copy link", true).clicked() {
                ui.ctx().copy_text(url.clone());
                self.toast("Link copied", false);
                ui.close();
            }
            if menu_row(ui, "Open on TIDAL", true).clicked() {
                let _ = crate::auth::open_browser(&url);
                ui.close();
            }
        });
    }

    pub(in crate::ui) fn reference_track_menu(
        &mut self,
        ui: &mut egui::Ui,
        t: &Track,
        i: usize,
        tracks: &Arc<Vec<Track>>,
        playlist: Option<&str>,
        owned: bool,
    ) {
        self.menu_header(ui, t, &t.title, &t.artist);
        let single = Arc::new(vec![t.clone()]);
        if menu_row(ui, "Play now", true).clicked() {
            self.play_list(tracks.clone(), i);
            ui.close();
        }
        if menu_row(ui, "Play next", true).clicked() {
            self.apply_list_action(ListAction::Next, single.clone());
            ui.close();
        }
        if t.id > 0 {
            self.add_to_playlist_menu(ui, None, &single);
            let saved = self.favorite_ids.contains(&t.id);
            if menu_row(
                ui,
                if saved {
                    "Remove from My Collection"
                } else {
                    "Add to My Collection"
                },
                true,
            )
            .clicked()
            {
                self.send(Command::Favorite(t.id, !saved));
                ui.close();
            }
        }
        if menu_row(ui, "Save for Later", true).clicked() {
            self.apply_list_action(ListAction::Later, single);
            ui.close();
        }
        if menu_row(ui, "Go to artist", !t.artist.is_empty()).clicked() {
            self.open_track_artist(t);
            ui.close();
        }
        if t.id > 0 {
            self.share_menu(ui, format!("https://tidal.com/browse/track/{}", t.id));
            if self.settings.song_downloader
                && menu_row(ui, "Download track", !self.download_active).clicked()
            {
                self.download_active = true;
                self.send(Command::Download(t.clone()));
                ui.close();
            }
        }
        if self.settings.cover_fetcher
            && menu_row(ui, "Download Cover Art", !t.cover.is_empty()).clicked()
        {
            self.send(Command::SaveCover(t.clone()));
            ui.close();
        }
        if self.settings.cover_fetcher
            && menu_row(ui, "Copy Cover Art Link", !t.cover.is_empty()).clicked()
        {
            if let Some(url) = t.cover_url(1280) {
                ui.ctx().copy_text(url);
            }
            ui.close();
        }
        if owned && let Some(id) = playlist {
            ui.separator();
            if i > 0 && menu_row(ui, "Move up", true).clicked() {
                self.send(Command::Move(id.into(), i, i - 1, tracks.clone()));
                ui.close();
            }
            if i + 1 < tracks.len() && menu_row(ui, "Move down", true).clicked() {
                self.send(Command::Move(id.into(), i, i + 1, tracks.clone()));
                ui.close();
            }
            if menu_row(ui, "Remove from playlist", true).clicked() {
                self.send(Command::Remove(id.into(), i, t.clone()));
                ui.close();
            }
        }
    }

    pub(in crate::ui) fn collection_menu(
        &mut self,
        ui: &mut egui::Ui,
        art: &Track,
        title: &str,
        subtitle: &str,
        key: &str,
    ) {
        self.menu_header(ui, art, title, subtitle);
        if menu_row(ui, "Play now", true).clicked() {
            self.list_action(key.into(), ListAction::Play);
            ui.close();
        }
        if menu_row(ui, "Play next", true).clicked() {
            self.list_action(key.into(), ListAction::Next);
            ui.close();
        }
        self.add_to_playlist_menu(ui, Some(key), &Arc::default());
        let owned = self.playlists.iter().any(|p| p.id == key && p.owned);
        if menu_row(
            ui,
            if owned {
                "In My Collection"
            } else if self.collection.contains(key) {
                "Remove from My Collection"
            } else {
                "Add to My Collection"
            },
            !owned && !self.collection_pending.contains(key),
        )
        .clicked()
        {
            self.collection_toggle(key);
            ui.close();
        }
        if menu_row(ui, "Save for Later", true).clicked() {
            self.list_action(key.into(), ListAction::Later);
            ui.close();
        }
        if menu_row(ui, "Go to artist", !art.artist.is_empty()).clicked() {
            self.open_track_artist(art);
            ui.close();
        }
        let url = if let Some(id) = key.strip_prefix("album:") {
            format!("https://tidal.com/browse/album/{id}")
        } else {
            format!("https://tidal.com/browse/playlist/{key}")
        };
        self.share_menu(ui, url);
        let count = self.tracks.get(key).map(|t| t.len()).or_else(|| {
            self.playlists
                .iter()
                .find(|p| p.id == key)
                .map(|p| p.count as usize)
        });
        let download = count
            .map(|c| format!("Download {c} tracks"))
            .unwrap_or_else(|| "Download album".into());
        if self.settings.song_downloader && menu_row(ui, &download, !self.download_active).clicked()
        {
            self.list_action(key.into(), ListAction::Download);
            ui.close();
        }
        if self.settings.cover_fetcher
            && menu_row(ui, "Download Cover Art", !art.cover.is_empty()).clicked()
        {
            self.send(Command::SaveCover(art.clone()));
            ui.close();
        }
        if self.settings.cover_fetcher
            && menu_row(ui, "Copy Cover Art Link", !art.cover.is_empty()).clicked()
        {
            if let Some(u) = art.cover_url(1280) {
                ui.ctx().copy_text(u);
            }
            ui.close();
        }
    }

    // Overlay controls stay separate from the artwork's navigation hit area.
    pub(super) fn inline_favorite(&mut self, ui: &mut egui::Ui, row: Rect, key: &str) -> bool {
        if !ui.rect_contains_pointer(row) {
            return false;
        }
        let rect = Rect::from_center_size(row.right_center() - vec2(26., 0.), vec2(34., 34.));
        let response = ui
            .interact(rect, Id::new(("inline-favorite", key)), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let saved = self.collection.contains(key);
        let owned = self.playlists.iter().any(|p| p.id == key && p.owned);
        paint_icon(
            ui.painter(),
            rect.shrink(8.),
            if saved { "heart" } else { "heart-outline" },
            TEXT,
        );
        if response
            .on_hover_text(if owned {
                "In My Collection"
            } else if saved {
                "Remove from My Collection"
            } else {
                "Add to My Collection"
            })
            .clicked()
        {
            if !owned {
                self.collection_toggle(key);
            }
            return true;
        }
        false
    }

    pub(super) fn card_controls(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        hovered: bool,
        key: &str,
        compact: bool,
    ) -> bool {
        let amount =
            ui.ctx()
                .animate_bool_with_time(Id::new(("card-hover", key, compact)), hovered, 0.16);
        let amount = layout::ease(amount);
        if amount < 0.01 {
            return false;
        }
        let play = if compact {
            Rect::from_center_size(rect.center(), vec2(36., 36.))
        } else {
            Rect::from_min_size(
                pos2(rect.left() + 12., rect.bottom() - 60. + (1. - amount) * 8.),
                vec2(48., 48.),
            )
        };
        ui.painter()
            .rect_filled(rect, 5, Color32::from_black_alpha((amount * 38.) as u8));
        let response = ui
            .interact(play, Id::new(("card-play", key, compact)), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let scale =
            ui.ctx()
                .animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.12);
        let visual = play.expand(layout::ease(scale) * 2.);
        if !compact {
            ui.painter().circle_filled(
                visual.center(),
                visual.width() * 0.5,
                Color32::from_white_alpha((amount * 255.) as u8),
            );
        }
        paint_icon(
            ui.painter(),
            visual.shrink(if compact { 4. } else { 9. }),
            "play",
            if compact { TEXT } else { Color32::BLACK },
        );
        let mut clicked = response.on_hover_text("Play now").clicked();
        if clicked {
            self.list_action(key.into(), ListAction::Play);
        }
        if !compact {
            let heart =
                Rect::from_center_size(pos2(rect.right() - 34., play.center().y), vec2(40., 40.));
            let response = ui
                .interact(heart, Id::new(("card-heart", key)), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            ui.painter().circle_filled(
                heart.center(),
                20.,
                Color32::from_black_alpha((amount * 180.) as u8),
            );
            let saved = self.collection.contains(key);
            paint_icon(
                ui.painter(),
                heart.shrink(10.),
                if saved { "heart" } else { "heart-outline" },
                TEXT,
            );
            if response
                .on_hover_text(if saved {
                    "Remove from My Collection"
                } else {
                    "Add to My Collection"
                })
                .clicked()
            {
                self.collection_toggle(key);
                clicked = true;
            }
        }
        clicked
    }
}
