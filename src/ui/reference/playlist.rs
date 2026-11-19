use super::*;

pub(super) const GOLD: Color32 = Color32::from_rgb(235, 196, 27);
pub(super) const AQUA: Color32 = Color32::from_rgb(42, 234, 217);

pub(super) fn date_label(raw: &str) -> String {
    let Some(day) = raw.get(..10) else {
        return String::new();
    };
    let pieces = day.split('-').collect::<Vec<_>>();
    if pieces.len() != 3 {
        return String::new();
    }
    let Ok(month) = pieces[1].parse::<usize>() else {
        return String::new();
    };
    let Ok(day) = pieces[2].parse::<u32>() else {
        return String::new();
    };
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return String::new();
    }
    format!("{} {day}, {}", months[month - 1], pieces[0])
}
pub(super) fn relative_date_label(raw: &str) -> String {
    let today = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
        / 86400;
    relative_day_label(raw, today)
}
fn relative_day_label(raw: &str, today: i64) -> String {
    let parts = raw
        .get(..10)
        .unwrap_or("")
        .split('-')
        .filter_map(|p| p.parse::<i64>().ok())
        .collect::<Vec<_>>();
    if parts.len() != 3 {
        return String::new();
    }
    let (year, month, day) = (parts[0], parts[1], parts[2]);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return String::new();
    }
    let y = year - i64::from(month <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let then = era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468;
    let delta = today - then;
    let weekday = (today + 3).rem_euclid(7);
    if delta == 0 {
        return "Today".into();
    }
    if delta == 1 {
        return "Yesterday".into();
    }
    if delta > 1 && delta <= weekday {
        return "This week".into();
    }
    if delta > weekday && delta <= weekday + 7 {
        return "Last week".into();
    }
    let month_days = match month {
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 31,
    };
    if delta > 0 && today > then - day && today < then - day + 1 + month_days {
        return "This month".into();
    }
    date_label(raw)
}

pub(super) fn sample_label(rate: Option<u32>) -> String {
    match rate {
        Some(rate) if rate > 0 && rate % 1000 == 0 => format!("{}kHz", rate / 1000),
        Some(rate) if rate > 0 => format!("{:.1}kHz", rate as f64 / 1000.),
        _ => String::new(),
    }
}
pub(super) fn bitrate_label(rate: Option<u64>) -> String {
    let Some(rate) = rate.filter(|n| *n > 0) else {
        return String::new();
    };
    let digits = (rate / 1000).to_string();
    let mut result = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(c);
    }
    format!("{result}kbps")
}
pub(super) fn action(
    ui: &mut egui::Ui,
    rect: Rect,
    id: &str,
    glyph: &str,
    text: &str,
    color: Color32,
    scale: f32,
) -> egui::Response {
    let response = ui
        .interact(rect, Id::new(id), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let hover =
        ui.ctx()
            .animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.14);
    ui.painter()
        .rect_filled(rect, 8, Color32::from_white_alpha((hover * 14.) as u8));
    paint_icon(
        ui.painter(),
        Rect::from_center_size(
            rect.center_top() + vec2(0., 18. * scale),
            vec2(28. * scale, 28. * scale),
        ),
        glyph,
        color,
    );
    ui.painter().text(
        rect.center_bottom() - vec2(0., 3. * scale),
        Align2::CENTER_BOTTOM,
        text,
        font(15. * scale, true),
        color,
    );
    response
}
pub(super) fn heart_points(rect: Rect) -> Vec<Pos2> {
    let scale = rect.width().min(rect.height()) * 0.41;
    let curves = [
        [(0., 0.88), (-0.22, 0.64), (-0.96, 0.12), (-0.96, -0.43)],
        [(-0.96, -0.43), (-0.96, -1.02), (-0.30, -1.12), (0., -0.55)],
        [(0., -0.55), (0.30, -1.12), (0.96, -1.02), (0.96, -0.43)],
        [(0.96, -0.43), (0.96, 0.12), (0.22, 0.64), (0., 0.88)],
    ];
    curves
        .into_iter()
        .flat_map(|curve| {
            (0..12).map(move |i| {
                let t = i as f32 / 12.;
                let u = 1. - t;
                let weights = [u * u * u, 3. * u * u * t, 3. * u * t * t, t * t * t];
                let point = curve
                    .into_iter()
                    .zip(weights)
                    .fold(Vec2::ZERO, |v, ((x, y), w)| v + vec2(x, y) * w);
                rect.center() + point * scale
            })
        })
        .collect()
}
pub(super) fn outline_heart(p: &egui::Painter, rect: Rect, color: Color32) {
    p.add(egui::Shape::closed_line(
        heart_points(rect),
        Stroke::new(1.7_f32, color),
    ));
}

impl App {
    pub(in crate::ui) fn playlist_background(&mut self, ctx: &egui::Context) {
        let rect = ctx.content_rect();
        let painter = ctx.layer_painter(egui::LayerId::background());
        painter.rect_filled(rect, 0, design::CANVAS);
        if !self.settings.playlist_background {
            return;
        }
        let Page::Playlist(id) = &self.page else {
            return;
        };
        let Some(playlist) = self
            .playlists
            .iter()
            .find(|item| item.id == *id)
            .or_else(|| self.discovery.lists.get(id))
        else {
            return;
        };
        let cover = Track {
            cover: playlist.image.clone(),
            ..Default::default()
        };
        let art = [640, 320, 160, 80]
            .into_iter()
            .filter_map(|size| artwork_key(&cover, size))
            .find(|key| self.artwork.contains_key(key));
        if let Some(art) = art.and_then(|key| self.artwork.get_mut(&key)) {
            art.touched = self.frame;
            let tint = Color32::from_rgb(art.palette[0], art.palette[1], art.palette[2]);
            painter.rect_filled(rect, 0, design::CANVAS.lerp_to_gamma(tint, 0.14));
        }
    }

    pub(super) fn rebuild_playlist_filter(&mut self, tracks: &Arc<Vec<Track>>) {
        self.playlist_filtered =
            self.playlist_index
                .select(tracks, &self.playlist_filter, self.playlist_sort);
    }

    fn playlist_menu(&mut self, ui: &mut egui::Ui, playlist: &Playlist, tracks: &Arc<Vec<Track>>) {
        if ui.button("Sync playlist").clicked() {
            self.send(Command::Playlist(playlist.id.clone()));
            ui.close();
        }
        if ui.button("Add playlist to queue").clicked() {
            Arc::make_mut(&mut self.queue).extend(tracks.iter().cloned());
            ui.close();
        }
        if ui
            .add_enabled(
                self.settings.song_downloader && !self.download_active,
                egui::Button::new("Download playlist"),
            )
            .clicked()
        {
            self.download_active = true;
            self.send(Command::DownloadMany(tracks.clone()));
            ui.close();
        }
        if ui.button("Copy playlist link").clicked() {
            ui.ctx()
                .copy_text(format!("https://tidal.com/browse/playlist/{}", playlist.id));
            ui.close();
        }
        if playlist.owned && ui.button("Delete playlist…").clicked() {
            self.dialog = Some(Dialog {
                kind: "delete".into(),
                id: playlist.id.clone(),
                title: playlist.title.clone(),
                description: playlist.description.clone(),
                ..Default::default()
            });
            ui.close();
        }
    }
    pub(in crate::ui) fn reference_playlist(&mut self, ui: &mut egui::Ui, playlist: &Playlist) {
        let compact = ui.ctx().content_rect().height() < 700.;
        let s = (ui.available_width() / 1800.).clamp(0.9, 1.1) * if compact { 0.78 } else { 1. };
        let tracks = self.tracks.get(&playlist.id).cloned().unwrap_or_default();
        if self.playlist_filter_id != playlist.id {
            self.playlist_filter_id = playlist.id.clone();
            self.playlist_filter.clear();
            self.playlist_sort = None;
            self.playlist_index.invalidate();
            self.playlist_scroll_reset = true;
        }
        if !self.playlist_index.matches(&tracks) {
            self.rebuild_playlist_filter(&tracks);
        }
        let margin = (ui.available_width() * 0.04).clamp(24., 80.);
        super::catalog::catalog_canvas(ui, "playlist-body", margin, 850., |ui| {
            ui.spacing_mut().item_spacing.y = 0.;
            ui.add_space(20. * s);
            let (hero, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), 248. * s), Sense::hover());
            let side = (hero.width() * 0.32).clamp(146., hero.height());
            let cover = Rect::from_min_size(hero.min, vec2(side, side));
            let art = Track {
                cover: playlist.image.clone(),
                ..Default::default()
            };
            at(ui, cover, "playlist-hero-cover", |ui| {
                self.cover_with_radius(ui, &art, side, design::ART);
            });
            let text_x = cover.right() + 28. * s;
            ui.painter().text(
                pos2(text_x, hero.top() + 20. * s),
                Align2::LEFT_TOP,
                "Playlist",
                font(14., true),
                design::BRAND,
            );
            let title = Rect::from_min_max(
                pos2(text_x, hero.top() + 50. * s),
                pos2(hero.right(), hero.top() + 108. * s),
            );
            at(ui, title, "playlist-hero-title", |ui| {
                if let Some((before, after)) = playlist.title.split_once(|c: char| {
                    ('\u{1f580}'..='\u{1f58f}').contains(&c) || c == '\u{1f9e8}'
                }) {
                    ui.horizontal(|ui| {
                        label(ui, before.trim_end(), 44. * s, true, TEXT);
                        let (r, _) = ui.allocate_exact_size(vec2(37. * s, 42. * s), Sense::hover());
                        let p = ui.painter();
                        let c = r.center();
                        p.add(egui::Shape::convex_polygon(
                            vec![
                                c + vec2(-15., 9.) * s,
                                c + vec2(7., -16.) * s,
                                c + vec2(17., -6.) * s,
                                c + vec2(-5., 19.) * s,
                            ],
                            Color32::from_rgb(231, 32, 111),
                            Stroke::NONE,
                        ));
                        p.circle_filled(
                            c + vec2(15., -15.) * s,
                            5. * s,
                            Color32::from_rgb(244, 143, 34),
                        );
                        label(
                            ui,
                            after.trim_start_matches('\u{fe0f}'),
                            44. * s,
                            true,
                            TEXT,
                        );
                    });
                } else {
                    label(ui, &playlist.title, 44. * s, true, TEXT);
                }
            });
            at(
                ui,
                Rect::from_min_max(
                    pos2(text_x, hero.top() + 110. * s),
                    pos2(hero.right(), hero.top() + 141. * s),
                ),
                "playlist-creator",
                |ui| {
                    ui.horizontal(|ui| {
                        label(
                            ui,
                            if playlist.creator.is_empty() {
                                if playlist.owned {
                                    "Created by me"
                                } else {
                                    "TIDAL playlist"
                                }
                            } else {
                                &playlist.creator
                            },
                            18. * s,
                            true,
                            Color32::from_rgb(166, 177, 191),
                        );
                        let (r, b) = ui.allocate_exact_size(vec2(30. * s, 30. * s), Sense::click());
                        ui.painter().circle_filled(
                            r.center(),
                            15. * s,
                            Color32::from_white_alpha(20),
                        );
                        paint_icon(ui.painter(), r.shrink(7. * s), "plus", GREY);
                        if b.on_hover_text("Copy playlist link to share").clicked() {
                            ui.ctx().copy_text(format!(
                                "https://tidal.com/browse/playlist/{}",
                                playlist.id
                            ));
                            self.toast("Playlist link copied", false);
                        }
                    });
                },
            );
            at(
                ui,
                Rect::from_min_max(
                    pos2(text_x, hero.top() + 172. * s),
                    pos2(hero.right(), hero.top() + 211. * s),
                ),
                "playlist-description",
                |ui| {
                    label(
                        ui,
                        &playlist.description,
                        18. * s,
                        true,
                        Color32::from_rgb(173, 177, 186),
                    );
                },
            );
            let count = if playlist.count > 0 {
                playlist.count as usize
            } else {
                tracks.len()
            };
            let duration = if playlist.duration > 0 {
                playlist.duration
            } else {
                tracks.iter().map(|t| t.duration).sum()
            };
            ui.painter().text(
                pos2(text_x, hero.top() + 226. * s),
                Align2::LEFT_TOP,
                if self.settings.total_play_time {
                    format!("{count} TRACKS  ({})", clock(duration as f64))
                } else {
                    format!("{count} TRACKS")
                },
                font(14. * s, true),
                Color32::from_rgb(188, 198, 211),
            );
            ui.add_space(if compact { 12. } else { 28. * s });
            let (actions, _) = ui.allocate_exact_size(
                vec2(
                    ui.available_width(),
                    if ui.available_width() < 850. {
                        130. * s
                    } else {
                        58. * s
                    },
                ),
                Sense::hover(),
            );
            let secondary_top = actions.top() + if actions.width() < 850. { 72. * s } else { 0. };
            for (name, x, width, fill, glyph) in [
                ("Play", 0., 193., design::BRAND, "play"),
                ("Shuffle", 213., 193., design::SURFACE, "shuffle"),
            ] {
                let r =
                    Rect::from_min_size(actions.min + vec2(x * s, 0.), vec2(width * s, 58. * s));
                ui.painter().rect_filled(r, design::CONTROL, fill);
                let response = ui
                    .interact(r, Id::new(("playlist-action", name)), Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                let color = if name == "Play" { design::INK } else { TEXT };
                labeled_control(ui.painter(), r, glyph, name, 21. * s, color);
                if response.clicked() && !tracks.is_empty() {
                    let mut selection = self
                        .playlist_filtered
                        .iter()
                        .map(|&i| tracks[i].clone())
                        .collect::<Vec<_>>();
                    if name == "Shuffle" {
                        use rand::seq::SliceRandom;
                        selection.shuffle(&mut rand::thread_rng());
                    }
                    self.play_list(Arc::new(selection), 0);
                }
            }
            let small_w = 83. * s;
            let right = actions.right();
            let edit = action(
                ui,
                Rect::from_min_size(
                    pos2(right - 405. * s, secondary_top),
                    vec2(small_w, 58. * s),
                ),
                "playlist-edit",
                "edit",
                "Edit",
                if playlist.owned { TEXT } else { GREY },
                s,
            );
            if edit.clicked() && playlist.owned {
                self.dialog = Some(Dialog {
                    kind: "edit".into(),
                    id: playlist.id.clone(),
                    title: playlist.title.clone(),
                    description: playlist.description.clone(),
                    public: playlist.public,
                    cover: playlist.image.clone(),
                    ..Default::default()
                });
            }
            let public_label = match playlist.public {
                Some(true) => "Public",
                Some(false) => "Private",
                None => "Visibility",
            };
            let visibility = action(
                ui,
                Rect::from_min_size(
                    pos2(right - 300. * s, secondary_top),
                    vec2(small_w, 58. * s),
                ),
                "playlist-visibility",
                "lock",
                public_label,
                if playlist.public == Some(true) {
                    AQUA
                } else {
                    TEXT
                },
                s,
            );
            egui::Popup::menu(&visibility).show(|ui| {
                ui.label(format!("Playlist visibility: {public_label}"));
                if ui.button("Manage visibility on TIDAL").clicked() {
                    let _ = crate::auth::open_browser(&format!(
                        "https://tidal.com/browse/playlist/{}",
                        playlist.id
                    ));
                    ui.close();
                }
            });
            let share = action(
                ui,
                Rect::from_min_size(
                    pos2(right - 195. * s, secondary_top),
                    vec2(small_w, 58. * s),
                ),
                "playlist-share",
                "share",
                "Share",
                TEXT,
                s,
            );
            if share.clicked() {
                ui.ctx()
                    .copy_text(format!("https://tidal.com/browse/playlist/{}", playlist.id));
                self.toast("Playlist link copied", false);
            }
            let more = action(
                ui,
                Rect::from_min_size(pos2(right - 89. * s, secondary_top), vec2(small_w, 58. * s)),
                "playlist-more",
                "dots",
                "More",
                TEXT,
                s,
            );
            egui::Popup::menu(&more).show(|ui| self.playlist_menu(ui, playlist, &tracks));
            ui.add_space(48. * s);
            let (filter, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), 50. * s), Sense::hover());
            if forms::track_filter(
                ui,
                filter,
                &mut self.playlist_filter,
                "Filter playlist on title, artist or album",
                "playlist-local-filter",
            ) {
                self.playlist_scroll_reset = true;
                self.rebuild_playlist_filter(&tracks);
            }
            if !self.playlist_filter.trim().is_empty() {
                forms::filter_summary(ui, filter, self.playlist_filtered.len(), tracks.len());
            }
            ui.add_space(56. * s);
            self.catalog_track_table(ui, tracks.clone(), Some(playlist), false);
        });
    }
}

#[cfg(test)]
mod date_tests {
    use super::*;
    #[test]
    fn saved_track_dates_use_calendar_weeks() {
        let today = 20753; // Tuesday, 27 October 2026.
        assert_eq!(relative_day_label("2026-10-27T12:00:00Z", today), "Today");
        assert_eq!(relative_day_label("2026-10-26", today), "Yesterday");
        assert_eq!(relative_day_label("2026-10-23", today), "Last week");
        assert_eq!(relative_day_label("2026-10-10", today), "This month");
        assert_eq!(relative_day_label("2024-10-14", today), "Oct 14, 2024");
    }
}
