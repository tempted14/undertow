use super::*;

fn hero_height(width: f32) -> f32 {
    (width * 0.54).clamp(500., 740.)
}
fn inset(width: f32) -> f32 {
    (width * 0.047).clamp(28., 88.)
}
fn fan_label(fans: u64) -> String {
    if fans >= 1_000_000 {
        format!("{:.1}M fans", fans as f64 / 1_000_000.)
    } else if fans >= 1000 {
        format!("{:.1}K fans", fans as f64 / 1000.)
    } else {
        format!("{fans} fans")
    }
}
fn release_label(album: &Album, singles: bool) -> String {
    let kind = match album.release_type.as_str() {
        "SINGLE" => "Single",
        "EP" => "EP",
        "ALBUM" => "Album",
        _ if singles => "EP / Single",
        _ => "Album",
    };
    if album.year.is_empty() {
        kind.into()
    } else {
        format!("{} · {kind}", album.year)
    }
}
fn shade(p: &egui::Painter, rect: Rect, corners: [u8; 4]) {
    let mut mesh = egui::Mesh::default();
    for (point, alpha) in [
        rect.left_top(),
        rect.right_top(),
        rect.left_bottom(),
        rect.right_bottom(),
    ]
    .into_iter()
    .zip(corners)
    {
        mesh.colored_vertex(point, Color32::from_black_alpha(alpha));
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    p.add(egui::Shape::mesh(mesh));
}
fn circle_action(
    ui: &mut egui::Ui,
    rect: Rect,
    id: Id,
    glyph: &str,
    primary: bool,
    enabled: bool,
    tip: &str,
) -> egui::Response {
    let response = ui.interact(
        rect,
        id,
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let hover = layout::ease(ui.ctx().animate_bool_with_time(
        id.with("hover"),
        response.hovered() && enabled,
        0.14,
    ));
    let r = rect.expand(if response.is_pointer_button_down_on() {
        -1.
    } else {
        hover
    });
    if primary || matches!(glyph, "plus" | "check" | "share") {
        ui.painter().circle_filled(
            r.center(),
            r.width() * 0.5,
            if primary {
                Color32::WHITE
            } else {
                Color32::from_gray((19. + 12. * hover) as u8)
            },
        );
    }
    paint_icon(
        ui.painter(),
        Rect::from_center_size(r.center(), vec2(32., 32.)),
        glyph,
        if !enabled {
            Color32::from_gray(70)
        } else if primary {
            Color32::BLACK
        } else {
            TEXT
        },
    );
    response
        .on_hover_text(tip)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

impl App {
    pub(in crate::ui) fn cache_artist(&mut self, id: u64, data: Result<Arc<Artist>, String>) {
        if !self.artists.contains_key(&id) && self.artists.len() >= 6 {
            let visible = if let Page::Artist(id) = self.page {
                Some(id)
            } else {
                None
            };
            if let Some(old) = self
                .artists
                .keys()
                .filter(|key| Some(**key) != visible)
                .min_by_key(|key| self.artist_access.get(key).copied().unwrap_or(0))
                .copied()
            {
                self.artists.remove(&old);
                self.artist_access.remove(&old);
            }
        }
        self.artists.insert(id, data);
        self.artist_access.insert(id, self.frame);
    }
    pub(in crate::ui) fn open_artist(&mut self, id: u64) {
        if id == 0 {
            return;
        }
        self.page = Page::Artist(id);
        self.artist_tab = 0;
        self.artist_release_view = 0;
        self.artist_album_offset = 0.;
        self.artist_single_offset = 0.;
        self.search_preview = false;
        if self.benchmark.is_some() && std::env::args().any(|a| a == "--background-benchmark") {
            self.artist_tab = match std::env::args()
                .skip_while(|a| a != "--artist-tab")
                .nth(1)
                .as_deref()
            {
                Some("music") => 1,
                Some("about") => 2,
                _ => 0,
            };
            self.artist_release_view = match std::env::args()
                .skip_while(|a| a != "--artist-releases")
                .nth(1)
                .as_deref()
            {
                Some("albums") => 1,
                Some("singles") => 2,
                Some("tracks") => 3,
                _ => 0,
            };
        }
    }
    pub(in crate::ui) fn open_track_artist(&mut self, track: &Track) {
        if track.artist_id > 0 {
            self.open_artist(track.artist_id);
        } else {
            self.query = track.artist.clone();
            self.search_tab = "Profiles".into();
            self.page = Page::Search;
            self.request_catalog();
        }
    }
    pub(in crate::ui) fn reference_artist(&mut self, ui: &mut egui::Ui, id: u64) {
        self.artist_access.insert(id, self.frame);
        if !self.artists.contains_key(&id) && self.artists_pending.insert(id) {
            self.send(Command::Artist(id));
        }
        let data = self.artists.get(&id).cloned();
        let Some(Ok(artist)) = data else {
            self.reference_toolbar(ui);
            ui.add_space(40.);
            if let Some(Err(message)) = data {
                label(ui, "Artist could not be loaded", 28., true, TEXT);
                ui.label(message);
                if pill(ui, "Try again", false).clicked() {
                    self.artists.remove(&id);
                }
            } else {
                label(ui, "Loading artist…", 24., true, GREY);
            }
            return;
        };
        let room = ui.available_rect_before_wrap();
        let height = (room.height() - reserved_player(ui.ctx())).max(160.);
        let mut scroll = egui::ScrollArea::vertical()
            .id_salt(("artist-page", id))
            .max_height(height)
            .auto_shrink([false, false]);
        if let Some(value) = self.artist_diagnostic_scroll {
            scroll = scroll.vertical_scroll_offset(value.max(0.));
        }

        let output = scroll.show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.;
            let width = ui.available_width();
            let (hero, _) = ui.allocate_exact_size(vec2(width, hero_height(width)), Sense::hover());
            self.artist_hero(ui, hero, &artist);
            let margin = inset(width);
            let body_width = width - margin * 2.;
            let origin = ui.cursor().min;
            let mut body = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt(("artist-content", id))
                    .max_rect(Rect::from_min_size(
                        origin + vec2(margin, 0.),
                        vec2(body_width, 10000.),
                    )),
            );
            body.set_clip_rect(ui.clip_rect());
            self.artist_body(&mut body, &artist);
            ui.advance_cursor_after_rect(Rect::from_min_max(
                origin,
                pos2(origin.x + width, body.min_rect().bottom()),
            ));
        });
        // Keep scrolled text and controls out from under the fixed navigation.
        let opacity = (output.state.offset.y / 120.).clamp(0., 1.);
        let toolbar = Rect::from_min_size(room.min, vec2(room.width(), 76.));
        ui.painter().rect_filled(
            toolbar,
            0,
            Color32::from_black_alpha((opacity * 255.) as u8),
        );
        at(ui, toolbar, "artist-toolbar", |ui| {
            self.reference_toolbar(ui)
        });
    }
    fn artist_hero(&mut self, ui: &mut egui::Ui, hero: Rect, artist: &Artist) {
        if !ui.is_rect_visible(hero) {
            return;
        }
        let portrait = Track {
            cover: artist.picture.clone(),
            ..Default::default()
        };
        let side = hero.height().min(hero.width() * 0.60);
        let photo = Rect::from_center_size(
            pos2(hero.center().x, hero.top() + side * 0.5),
            vec2(side, side),
        );
        let key = [640, 320, 160, 80]
            .into_iter()
            .filter_map(|s| artwork_key(&portrait, s))
            .find(|k| self.artwork.contains_key(k));
        let p = ui.painter().clone();
        p.rect_filled(hero, 0, Color32::from_rgb(16, 17, 18));
        if self.settings.show_covers
            && let Some(art) = key.and_then(|key| self.artwork.get_mut(&key))
        {
            art.touched = self.frame;
            p.image(
                art.blur.id(),
                hero,
                Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
                Color32::from_white_alpha(170),
            );
        }
        at(ui, photo, ("artist-portrait", artist.id), |ui| {
            self.cover_image(ui, &portrait, side, 0, false);
        });
        let half = hero.width() * 0.5;
        shade(
            &p,
            Rect::from_min_size(hero.min, vec2(half, hero.height())),
            [85, 15, 250, 160],
        );
        shade(
            &p,
            Rect::from_min_size(hero.min + vec2(half, 0.), vec2(half, hero.height())),
            [15, 85, 160, 250],
        );
        shade(
            &p,
            Rect::from_min_max(hero.min + vec2(0., hero.height() * 0.48), hero.max),
            [0, 0, 255, 255],
        );
        let left = hero.left() + inset(hero.width());
        let name = Rect::from_min_max(
            pos2(left, hero.bottom() - 224.),
            pos2(hero.right() - 28., hero.bottom() - 164.),
        );
        at(ui, name, ("artist-name", artist.id), |ui| {
            label(ui, &artist.name, 44., true, Color32::WHITE);
        });
        if let Some(fans) = artist.fans {
            p.text(
                pos2(left, hero.bottom() - 145.),
                Align2::LEFT_CENTER,
                fan_label(fans),
                font(17., true),
                Color32::from_gray(185),
            );
        }
        let y = hero.bottom() - 70.;
        let bounds =
            |i: usize| Rect::from_center_size(pos2(left + 32. + i as f32 * 84., y), vec2(64., 64.));
        if circle_action(
            ui,
            bounds(0),
            Id::new(("artist-play", artist.id)),
            "play",
            true,
            !artist.tracks.is_empty(),
            "Play top tracks",
        )
        .clicked()
        {
            self.play_list(artist.tracks.clone(), 0);
        }
        let key = format!("artist:{}", artist.id);
        let followed = self.collection.contains(&key);
        let follow = circle_action(
            ui,
            bounds(1),
            Id::new(("artist-follow", artist.id)),
            if followed { "check" } else { "plus" },
            false,
            artist.followed.is_some() && !self.collection_pending.contains(&key),
            if followed {
                "Unfollow artist"
            } else {
                "Follow artist"
            },
        );
        if follow.clicked() {
            self.collection_toggle(&key);
        }
        let share = circle_action(
            ui,
            bounds(2),
            Id::new(("artist-share", artist.id)),
            "share",
            false,
            true,
            "Share artist",
        );
        egui::Popup::menu(&share).show(|ui| {
            if ui.button("Copy artist link").clicked() {
                ui.ctx()
                    .copy_text(format!("https://tidal.com/browse/artist/{}", artist.id));
                ui.close();
            }
        });
        let report = circle_action(
            ui,
            bounds(3),
            Id::new(("artist-report", artist.id)),
            "warning",
            false,
            true,
            "Report artist",
        );
        egui::Popup::menu(&report).show(|ui| {
            ui.label("Report an artist or catalog issue to TIDAL.");
            if ui.button("Open TIDAL support").clicked() {
                let _ = crate::auth::open_browser("https://support.tidal.com/hc/requests/new");
                ui.close();
            }
        });
        let more = circle_action(
            ui,
            bounds(4),
            Id::new(("artist-more", artist.id)),
            "dots",
            false,
            true,
            "More",
        );
        egui::Popup::menu(&more).show(|ui| {
            for (title, action) in [
                ("Play next", interactions::ListAction::Next),
                ("Save for Later", interactions::ListAction::Later),
                ("Download top tracks", interactions::ListAction::Download),
            ] {
                if (title != "Download top tracks" || self.settings.song_downloader)
                    && ui.button(title).clicked()
                {
                    self.apply_list_action(action, artist.tracks.clone());
                    ui.close();
                }
            }
            if ui.button("Shuffle top tracks").clicked() {
                use rand::seq::SliceRandom;
                let mut tracks = artist.tracks.as_ref().clone();
                tracks.shuffle(&mut rand::thread_rng());
                self.shuffle_on = true;
                self.play_list(Arc::new(tracks), 0);
                ui.close();
            }
            if ui.button("Refresh artist").clicked() {
                if self.artists_pending.insert(artist.id) {
                    self.send(Command::Artist(artist.id));
                }
                ui.close();
            }
        });
    }
    fn artist_body(&mut self, ui: &mut egui::Ui, artist: &Artist) {
        let w = ui.available_width();
        let (tabs, _) = ui.allocate_exact_size(vec2(w, 62.), Sense::hover());
        let mut x = tabs.left();
        for (i, title) in ["All", "Music", "About"].into_iter().enumerate() {
            let r = Rect::from_min_size(
                pos2(x, tabs.top()),
                vec2(if i == 2 { 74. } else { 66. }, 62.),
            );
            let b = ui
                .interact(r, Id::new(("artist-tab", artist.id, i)), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            ui.painter().text(
                r.left_center(),
                Align2::LEFT_CENTER,
                title,
                font(18., true),
                if self.artist_tab == i { TEXT } else { GREY },
            );
            if self.artist_tab == i {
                ui.painter().line_segment(
                    [
                        r.left_bottom(),
                        r.left_bottom() + vec2(if i == 0 { 23. } else { 46. }, 0.),
                    ],
                    Stroke::new(2_f32, TEXT),
                );
            }
            if b.clicked() {
                self.artist_tab = i;
                self.artist_release_view = 0;
            }
            x += r.width();
        }
        ui.painter().line_segment(
            [tabs.left_bottom(), tabs.right_bottom()],
            Stroke::new(1_f32, Color32::from_gray(38)),
        );
        ui.add_space(42.);
        if self.artists_pending.contains(&artist.id) && artist.tracks.is_empty() {
            label(ui, "Loading music…", 18., false, GREY);
            return;
        }
        for notice in &artist.notices {
            label(ui, notice, 15., false, GREY);
        }
        if self.artist_tab == 2 {
            label(ui, "About", 28., true, TEXT);
            ui.add_space(22.);
            ui.add(
                egui::Label::new(
                    RichText::new(if artist.biography.trim().is_empty() {
                        "No biography is available from TIDAL."
                    } else {
                        &artist.biography
                    })
                    .font(font(18., false))
                    .color(TEXT),
                )
                .wrap(),
            );
        } else if self.artist_release_view == 1 || self.artist_release_view == 2 {
            if small_pill(ui, "‹ All music").clicked() {
                self.artist_release_view = 0;
            }
            ui.add_space(24.);
            self.artist_releases(ui, artist, self.artist_release_view == 2, true);
        } else {
            if self.artist_tab == 0 || self.artist_release_view == 3 {
                self.artist_top_tracks(ui, artist, self.artist_release_view == 3);
                ui.add_space(42.);
            }
            if self.artist_release_view != 3 {
                self.artist_releases(ui, artist, false, false);
                ui.add_space(42.);
                self.artist_releases(ui, artist, true, false);
            }
        }
        ui.add_space(36.);
    }
    fn artist_top_tracks(&mut self, ui: &mut egui::Ui, artist: &Artist, all: bool) {
        let w = ui.available_width();
        let (head, _) = ui.allocate_exact_size(vec2(w, 42.), Sense::hover());
        ui.painter().text(
            head.left_top(),
            Align2::LEFT_TOP,
            "Top Tracks",
            font(24., true),
            TEXT,
        );
        let view = Rect::from_min_size(head.right_top() - vec2(84., 0.), vec2(84., 32.));
        let b = ui
            .interact(
                view,
                Id::new(("artist-all-tracks", artist.id)),
                Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        ui.painter().text(
            view.right_top(),
            Align2::RIGHT_TOP,
            if all { "Show less" } else { "View all" },
            font(17., true),
            if b.hovered() { TEXT } else { GREY },
        );
        if b.clicked() {
            self.artist_release_view = if all { 0 } else { 3 };
        }
        let count = if all {
            artist.tracks.len()
        } else {
            artist.tracks.len().min(4)
        };
        egui::ScrollArea::horizontal()
            .id_salt(("artist-track-columns", artist.id))
            .auto_shrink([false, true])
            .show(ui, |ui| {
                // Reserve room for duration and all three row actions at narrow sizes.
                let width = ui.available_width().max(1100.);
                let (head, _) = ui.allocate_exact_size(vec2(width, 38.), Sense::hover());
                for (title, x) in [
                    ("TITLE", 80.),
                    ("ARTIST", width * 0.345),
                    ("ALBUM", width * 0.595),
                    ("TIME", width * 0.845),
                ] {
                    ui.painter().text(
                        pos2(head.left() + x, head.center().y),
                        Align2::LEFT_CENTER,
                        title,
                        font(14., true),
                        GREY,
                    );
                }
                for (i, track) in artist.tracks.iter().take(count).enumerate() {
                    let (r, response) = ui.allocate_exact_size(vec2(width, 72.), Sense::click());
                    if !ui.is_rect_visible(r) {
                        self.prefetch_cover(track, 54., ui.ctx().pixels_per_point());
                        continue;
                    }
                    if response.hovered() {
                        ui.painter()
                            .rect_filled(r, 4, Color32::from_white_alpha(12));
                    }
                    let cover = Rect::from_min_size(r.min + vec2(0., 8.), vec2(54., 54.));
                    let art = at(ui, cover, ("artist-track-art", artist.id, i), |ui| {
                        self.cover(ui, track, 54.)
                    });
                    if ui.rect_contains_pointer(cover) {
                        ui.painter()
                            .rect_filled(cover, 4, Color32::from_black_alpha(90));
                        paint_icon(ui.painter(), cover.shrink(10.), "play", TEXT);
                    }
                    let active = self.current.as_ref().is_some_and(|t| t.id == track.id);
                    at(
                        ui,
                        Rect::from_min_max(
                            r.min + vec2(80., 22.),
                            pos2(r.left() + width * 0.32, r.bottom()),
                        ),
                        ("artist-track-title", i),
                        |ui| self.track_title(ui, track, if active { ACCENT } else { TEXT }),
                    );
                    let artist_b = at(
                        ui,
                        Rect::from_min_max(
                            r.min + vec2(width * 0.345, 22.),
                            pos2(r.left() + width * 0.58, r.bottom()),
                        ),
                        ("artist-track-artist", i),
                        |ui| text_link(ui, &track.artist, 17., true, TEXT, true),
                    );
                    if artist_b.clicked() {
                        self.open_track_artist(track);
                    }
                    let album_b = at(
                        ui,
                        Rect::from_min_max(
                            r.min + vec2(width * 0.595, 22.),
                            pos2(r.left() + width * 0.83, r.bottom()),
                        ),
                        ("artist-track-album", i),
                        |ui| text_link(ui, &track.album, 17., true, TEXT, track.album_id > 0),
                    );
                    if album_b.clicked() {
                        self.open_album(track.album_id, track.album.clone());
                    }
                    ui.painter().text(
                        r.min + vec2(width * 0.845, 36.),
                        Align2::LEFT_CENTER,
                        clock(track.duration as f64),
                        font(17., true),
                        TEXT,
                    );
                    let plus =
                        Rect::from_center_size(r.right_center() - vec2(64., 0.), vec2(34., 34.));
                    paint_icon(ui.painter(), plus.shrink(5.), "plus", GREY);
                    let add = ui
                        .interact(
                            plus,
                            Id::new(("artist-track-add", artist.id, i)),
                            Sense::click(),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    egui::Popup::menu(&add).show(|ui| {
                        ui.label("Add to playlist");
                        for playlist in self.playlists.clone().into_iter().filter(|p| p.owned) {
                            if ui.button(&playlist.title).clicked() {
                                self.send(Command::Add(playlist.id, track.id));
                                ui.close();
                            }
                        }
                    });
                    let more =
                        Rect::from_center_size(r.right_center() - vec2(106., 0.), vec2(34., 34.));
                    let menu = ui
                        .interact(
                            more,
                            Id::new(("artist-track-menu", artist.id, i)),
                            Sense::click(),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    if ui.rect_contains_pointer(r)
                        || egui::Popup::is_id_open(
                            ui.ctx(),
                            egui::Popup::default_response_id(&menu),
                        )
                    {
                        paint_icon(ui.painter(), more.shrink(5.), "dots", GREY);
                    }
                    egui::Popup::menu(&menu)
                        .show(|ui| self.track_menu(ui, track, i, &artist.tracks, None, false));
                    let heart =
                        Rect::from_center_size(r.right_center() - vec2(22., 0.), vec2(34., 34.));
                    let saved = self.favorite_ids.contains(&track.id);
                    paint_icon(
                        ui.painter(),
                        heart.shrink(6.),
                        if saved { "heart" } else { "heart-outline" },
                        if saved { TEXT } else { GREY },
                    );
                    if ui
                        .interact(
                            heart,
                            Id::new(("artist-track-heart", artist.id, i)),
                            Sense::click(),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                        && self.connected
                    {
                        self.send(Command::Favorite(track.id, !saved));
                    }
                    if response.clicked() || art.clicked() {
                        self.play_list(artist.tracks.clone(), i);
                    }
                    response.context_menu(|ui| {
                        self.track_menu(ui, track, i, &artist.tracks, None, false)
                    });
                }
            });
        if count == 0 {
            label(ui, "No top tracks available", 17., false, GREY);
        }
    }
    fn artist_releases(&mut self, ui: &mut egui::Ui, artist: &Artist, singles: bool, all: bool) {
        let releases = if singles {
            &artist.singles
        } else {
            &artist.albums
        };
        let shelf_indices = if singles {
            &artist.single_shelf
        } else {
            &artist.album_shelf
        };
        let count = if all {
            releases.len()
        } else {
            shelf_indices.len()
        };
        let w = ui.available_width();
        let cols = columns(w, 240., 22., 8);
        let size = (w - 22. * (cols - 1) as f32) / cols as f32;
        let (head, _) = ui.allocate_exact_size(vec2(w, 46.), Sense::hover());
        ui.painter().text(
            head.left_top(),
            Align2::LEFT_TOP,
            if singles { "EP & Singles" } else { "Albums" },
            font(24., true),
            TEXT,
        );
        let view = Rect::from_min_size(head.right_top() - vec2(84., 0.), vec2(84., 32.));
        let b = ui
            .interact(
                view,
                Id::new(("artist-view-releases", artist.id, singles)),
                Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        ui.painter().text(
            view.right_top(),
            Align2::RIGHT_TOP,
            if all { "Show less" } else { "View all" },
            font(17., true),
            if b.hovered() { TEXT } else { GREY },
        );
        if b.clicked() {
            self.artist_release_view = if all {
                0
            } else if singles {
                2
            } else {
                1
            };
        }
        let mut offset = if singles {
            self.artist_single_offset
        } else {
            self.artist_album_offset
        };
        if !all {
            for (name, dx, delta) in [
                ("back", 148., -(cols as f32)),
                ("forward", 110., cols as f32),
            ] {
                let rect = Rect::from_center_size(
                    pos2(head.right() - dx, head.top() + 14.),
                    vec2(30., 30.),
                );
                let enabled = if delta < 0. {
                    offset > 0.
                } else {
                    offset + (cols as f32) < (count as f32)
                };
                let b = ui.interact(
                    rect,
                    Id::new(("artist-shelf-arrow", artist.id, singles, name)),
                    if enabled {
                        Sense::click()
                    } else {
                        Sense::hover()
                    },
                );
                ui.painter().circle_filled(
                    rect.center(),
                    15.,
                    Color32::from_gray(if b.hovered() && enabled { 33 } else { 18 }),
                );
                paint_icon(
                    ui.painter(),
                    rect.shrink(7.),
                    name,
                    if enabled {
                        TEXT
                    } else {
                        Color32::from_gray(65)
                    },
                );
                if b.clicked() {
                    offset += delta;
                }
            }
            offset = offset.clamp(0., count.saturating_sub(cols) as f32);
            if singles {
                self.artist_single_offset = offset;
            } else {
                self.artist_album_offset = offset;
            }
        }
        if count == 0 {
            label(
                ui,
                if self.artists_pending.contains(&artist.id) {
                    "Loading releases…"
                } else {
                    "No releases available"
                },
                17.,
                false,
                GREY,
            );
            return;
        }
        let rows = if all { count.div_ceil(cols) } else { 1 };
        let (shelf, _) =
            ui.allocate_exact_size(vec2(w, (size + 103.) * rows as f32), Sense::hover());
        if !all && ui.rect_contains_pointer(shelf) {
            let delta = ui.input_mut(|i| {
                let d = i.smooth_scroll_delta.x;
                i.smooth_scroll_delta.x = 0.;
                d
            });
            offset = (offset - delta / (size + 22.)).clamp(0., count.saturating_sub(cols) as f32);
            if singles {
                self.artist_single_offset = offset;
            } else {
                self.artist_album_offset = offset;
            }
        }
        let animated = if all {
            0.
        } else {
            ui.ctx().animate_value_with_time(
                Id::new(("artist-shelf-slide", artist.id, singles)),
                offset,
                0.22,
            )
        };
        let first = if all { 0 } else { animated.floor() as usize };
        let last = if all {
            count
        } else {
            (first + cols + 1).min(count)
        };
        let clip = ui.clip_rect().intersect(shelf);
        for i in first..last {
            let album = &releases[if all { i } else { shelf_indices[i] }];
            let x = if all {
                (i % cols) as f32 * (size + 22.)
            } else {
                (i as f32 - animated) * (size + 22.)
            };
            let y = if all {
                (i / cols) as f32 * (size + 103.)
            } else {
                0.
            };
            let card = Rect::from_min_size(shelf.min + vec2(x, y), vec2(size, size + 80.));
            if !card.intersects(clip) {
                continue;
            }
            let mut cell = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt(("artist-release", artist.id, singles, album.id))
                    .max_rect(card),
            );
            cell.set_clip_rect(clip.intersect(card));
            let art = Track {
                cover: album.cover.clone(),
                album: album.title.clone(),
                artist: artist.name.clone(),
                artist_id: artist.id,
                album_id: album.id,
                explicit: album.explicit,
                ..Default::default()
            };
            let cover = Rect::from_min_size(card.min, vec2(size, size));
            let response = at(&mut cell, cover, album.id, |ui| {
                self.cover_with_radius(ui, &art, size, 5)
            });
            let key = format!("album:{}", album.id);
            let controls = self.card_controls(
                &mut cell,
                cover,
                ui.rect_contains_pointer(cover),
                &key,
                false,
            );
            let caption = Rect::from_min_max(card.min + vec2(0., size + 12.), card.max);
            at(&mut cell, caption, "caption", |ui| {
                self.track_title(
                    ui,
                    &Track {
                        title: album.title.clone(),
                        explicit: album.explicit,
                        ..Default::default()
                    },
                    TEXT,
                );
                ui.add_space(5.);
                label(ui, release_label(album, singles), 17., false, GREY);
            });
            let title = cell
                .interact(
                    caption,
                    Id::new(("artist-release-caption", album.id)),
                    Sense::click(),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if !controls && (response.clicked() || title.clicked()) {
                self.open_album(album.id, album.title.clone());
            }
            response.context_menu(|ui| {
                self.collection_menu(ui, &art, &album.title, &artist.name, &key)
            });
        }
    }
}
