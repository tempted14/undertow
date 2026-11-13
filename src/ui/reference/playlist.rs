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
    pub(in crate::ui) fn playlist_background(&self, ctx: &egui::Context) {
        let rect = ctx.content_rect();
        let p = ctx.layer_painter(egui::LayerId::background());
        // Sampled from the supplied reference's unobstructed background. This is
        // a small native gradient mesh, not a full-resolution blurred bitmap.
        let colors = [
            [[17, 37, 63], [60, 50, 60], [123, 78, 75]],
            [[37, 65, 100], [73, 79, 90], [116, 91, 73]],
            [[54, 75, 105], [70, 77, 77], [93, 85, 60]],
            [[85, 70, 65], [52, 51, 40], [28, 42, 21]],
        ];
        let mut mesh = egui::Mesh::default();
        for (y, row) in colors.iter().enumerate() {
            for (x, rgb) in row.iter().enumerate() {
                mesh.colored_vertex(
                    rect.min + vec2(rect.width() * x as f32 / 2., rect.height() * y as f32 / 3.),
                    Color32::from_rgb(rgb[0], rgb[1], rgb[2]),
                );
            }
        }
        for y in 0..3 {
            for x in 0..2 {
                let a = (y * 3 + x) as u32;
                mesh.add_triangle(a, a + 1, a + 3);
                mesh.add_triangle(a + 1, a + 4, a + 3);
            }
        }
        p.add(egui::Shape::mesh(mesh));
    }
    pub(in crate::ui) fn playlist_toolbar(&mut self, ui: &mut egui::Ui) {
        let s = (ui.available_width() / 1800.).clamp(0.9, 1.1);
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 76. * s), Sense::hover());
        let arrows = Rect::from_min_size(r.min + vec2(20. * s, 16. * s), vec2(84. * s, 44. * s));
        ui.painter()
            .rect_filled(arrows, 24, Color32::from_black_alpha(40));
        for (name, x) in [
            ("back", arrows.left() + 24. * s),
            ("forward", arrows.right() - 24. * s),
        ] {
            let rect = Rect::from_center_size(pos2(x, arrows.center().y), vec2(29. * s, 29. * s));
            let enabled = if name == "back" {
                !self.back_history.is_empty()
            } else {
                !self.forward_history.is_empty()
            };
            paint_icon(
                ui.painter(),
                rect,
                name,
                if enabled {
                    TEXT
                } else {
                    Color32::from_white_alpha(50)
                },
            );
            if ui
                .interact(rect, Id::new(("playlist-history", name)), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                if name == "back" {
                    if let Some(page) = self.back_history.pop() {
                        self.forward_history.push(self.page.clone());
                        self.page = page.clone();
                        self.history_page = page;
                    }
                } else if let Some(page) = self.forward_history.pop() {
                    self.back_history.push(self.page.clone());
                    self.page = page.clone();
                    self.history_page = page;
                }
            }
        }
        let avatar = Rect::from_center_size(
            pos2(r.right() - 35. * s, r.top() + 37. * s),
            vec2(42. * s, 42. * s),
        );
        self.profile_button(ui, avatar);
        let search = Rect::from_min_size(
            pos2(avatar.left() - 24. * s - 336. * s, r.top() + 15. * s),
            vec2(336. * s, 44. * s),
        );
        ui.painter()
            .rect_filled(search, 24, Color32::from_black_alpha(110));
        paint_icon(
            ui.painter(),
            Rect::from_min_size(search.min + vec2(13. * s, 11. * s), vec2(23. * s, 23. * s)),
            "search",
            GREY,
        );
        at(
            ui,
            Rect::from_min_max(
                search.min + vec2(52. * s, 7. * s),
                search.max - vec2(42. * s, 6. * s),
            ),
            "playlist-global-search",
            |ui| {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .font(font(18. * s, false))
                        .hint_text("Search")
                        .frame(false)
                        .desired_width(f32::INFINITY),
                );
                self.search_input_response(ui, &response, search);
            },
        );
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
        let s = (ui.available_width() / 1800.).clamp(0.9, 1.1);
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
                ui.allocate_exact_size(vec2(ui.available_width(), 288. * s), Sense::hover());
            let cover = Rect::from_min_size(hero.min, vec2(288. * s, 288. * s));
            let art = Track {
                cover: playlist.image.clone(),
                ..Default::default()
            };
            at(ui, cover, "playlist-hero-cover", |ui| {
                self.cover(ui, &art, 288. * s);
            });
            let text_x = cover.right() + 38. * s;
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
            ui.add_space(58. * s);
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
                ("Play", 0., 193., Color32::WHITE, "play"),
                (
                    "Shuffle",
                    213.,
                    193.,
                    Color32::from_white_alpha(24),
                    "shuffle",
                ),
            ] {
                let r =
                    Rect::from_min_size(actions.min + vec2(x * s, 0.), vec2(width * s, 58. * s));
                ui.painter().rect_filled(r, 32, fill);
                let response = ui
                    .interact(r, Id::new(("playlist-action", name)), Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                let color = if name == "Play" {
                    Color32::from_rgb(17, 19, 22)
                } else {
                    TEXT
                };
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
