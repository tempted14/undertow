use super::*;

impl App {
    pub(super) fn navigation_sidebar(&mut self, ctx: &egui::Context) {
        if !self.settings.show_sidebar || self.page == Page::Lyrics {
            return;
        }
        // Keep the user's full-width preference; narrow windows temporarily use the rail.
        let collapsed = self.settings.sidebar_collapsed || ctx.content_rect().width() < 1100.;
        let maximum = (ctx.content_rect().width() - 680.).clamp(265., 380.);
        let expanded = self.sidebar_expanded_width.min(maximum);
        let progress = ctx.animate_bool_with_time(Id::new("sidebar-motion"), !collapsed, 0.24);
        let width = 76. + (expanded - 76.) * layout::ease(progress);
        let moving = progress > 0. && progress < 1.;
        let panel = egui::SidePanel::left("library-navigation")
            .resizable(!collapsed && !moving)
            .default_width(width)
            .width_range(if moving || collapsed {
                width..=width
            } else {
                265. ..=maximum
            })
            .frame(
                Frame::new()
                    .fill(Color32::from_rgb(10, 10, 10))
                    .inner_margin(Margin::symmetric(if collapsed { 12 } else { 20 }, 0)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = vec2(0., 0.);
                let (header, _) = ui.allocate_exact_size(
                    vec2(ui.available_width(), if collapsed { 104. } else { 76. }),
                    Sense::hover(),
                );
                ui.painter().text(
                    if collapsed {
                        header.center_top() + vec2(0., 30.)
                    } else {
                        header.left_top() + vec2(12., 30.)
                    },
                    if collapsed {
                        Align2::CENTER_CENTER
                    } else {
                        Align2::LEFT_CENTER
                    },
                    if collapsed { "U" } else { "UNDERTOW" },
                    font(if collapsed { 25. } else { 23. }, true),
                    TEXT,
                );
                let toggle = Rect::from_center_size(
                    if collapsed {
                        pos2(header.center().x, header.top() + 76.)
                    } else {
                        pos2(header.right() - 6., header.top() + 31.)
                    },
                    vec2(32., 32.),
                );
                paint_icon(
                    ui.painter(),
                    toggle.shrink(5.),
                    if collapsed {
                        "sidebar-expand"
                    } else {
                        "sidebar-collapse"
                    },
                    GREY,
                );
                if ui
                    .interact(toggle, Id::new("sidebar-toggle"), Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(if collapsed {
                        "Expand sidebar · Ctrl+B"
                    } else {
                        "Collapse sidebar · Ctrl+B"
                    })
                    .clicked()
                {
                    self.settings.sidebar_collapsed = !self.settings.sidebar_collapsed;
                    self.schedule_settings_save();
                }
                for (glyph, title, page) in [
                    ("music", "Music", Some(Page::Home)),
                    ("collection", "Collection", Some(Page::Favorites)),
                ] {
                    let selected = match glyph {
                        "music" => {
                            matches!(
                                self.page,
                                Page::Home
                                    | Page::Discovery(_)
                                    | Page::Mix(_)
                                    | Page::Playlist(_)
                                    | Page::Album(..)
                            )
                        }
                        "collection" => self.page == Page::Favorites,
                        _ => false,
                    };
                    let (r, b) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 52.), Sense::click());
                    if selected || b.hovered() {
                        ui.painter().rect_filled(
                            r,
                            16,
                            if selected {
                                Color32::from_gray(46)
                            } else {
                                Color32::from_gray(24)
                            },
                        );
                    }
                    let c = pos2(
                        if collapsed {
                            r.center().x
                        } else {
                            r.left() + 27.
                        },
                        r.center().y,
                    );
                    paint_icon(
                        ui.painter(),
                        Rect::from_center_size(c, vec2(29., 29.)),
                        glyph,
                        if selected { TEXT } else { GREY },
                    );
                    if !collapsed {
                        ui.painter().text(
                            pos2(r.left() + 53., r.center().y),
                            Align2::LEFT_CENTER,
                            title,
                            font(18., true),
                            if selected { TEXT } else { GREY },
                        );
                    }
                    if b.clicked()
                        && let Some(ref page) = page
                    {
                        self.page = page.clone();
                        self.search_preview = false;
                        if self.connected && *page == Page::Favorites {
                            self.send(Command::Favorites);
                        }
                    }
                    b.on_hover_text(if page.is_some() {
                        title
                    } else {
                        "Cloud uploads are unavailable. Drop local music into the player."
                    });
                }
                ui.add_space(if collapsed { 20. } else { 34. });
                if !collapsed {
                    let (r, _) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 34.), Sense::hover());
                    ui.painter().text(
                        r.left_center() + vec2(3., 0.),
                        Align2::LEFT_CENTER,
                        "All playlists",
                        font(18., true),
                        Color32::from_gray(117),
                    );
                    for (glyph, x) in [("plus", r.right() - 42.), ("sort", r.right() - 5.)] {
                        let rect = Rect::from_center_size(pos2(x, r.center().y), vec2(25., 25.));
                        paint_icon(ui.painter(), rect, glyph, GREY);
                        if ui
                            .interact(rect, Id::new(("navigation-header", glyph)), Sense::click())
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            if glyph == "plus" {
                                self.dialog = Some(Dialog {
                                    kind: "create".into(),
                                    ..Default::default()
                                });
                            } else {
                                self.playlists.reverse();
                            }
                        }
                    }
                    ui.add_space(12.);
                }
                let lists = self.playlists.clone();
                egui::ScrollArea::vertical()
                    .id_salt("library-playlists")
                    .max_height((ui.available_height() - reserved_player(ctx) - 18.).max(80.))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for playlist in lists {
                            let (row, response) = ui.allocate_exact_size(
                                vec2(ui.available_width(), if collapsed { 58. } else { 68. }),
                                Sense::click(),
                            );
                            let selected =
                                matches!(&self.page,Page::Playlist(id)if id==&playlist.id);
                            let playing = self.playing_playlist.as_ref() == Some(&playlist.id)
                                && self.audio.is_some()
                                && !self.paused();
                            if response.hovered() || selected {
                                ui.painter().rect_filled(
                                    row,
                                    7,
                                    Color32::from_gray(if selected { 30 } else { 20 }),
                                );
                            }
                            let size = if collapsed { 42. } else { 48. };
                            let cover = Rect::from_center_size(
                                pos2(
                                    if collapsed {
                                        row.center().x
                                    } else {
                                        row.left() + 27.
                                    },
                                    row.center().y,
                                ),
                                vec2(size, size),
                            );
                            let track = Track {
                                cover: playlist.image.clone(),
                                ..Default::default()
                            };
                            let art = at(ui, cover, ("library-cover", &playlist.id), |ui| {
                                self.cover(ui, &track, size)
                            });
                            if playing {
                                ui.painter()
                                    .rect_filled(cover, 5, Color32::from_black_alpha(100));
                                paint_icon(
                                    ui.painter(),
                                    cover.shrink(12.),
                                    "volume",
                                    Color32::from_rgb(239, 197, 20),
                                );
                            }
                            let text = Rect::from_min_max(
                                row.min + vec2(67., 10.),
                                row.max - vec2(6., 0.),
                            );
                            let text_clicked = if !collapsed {
                                at(ui, text, ("library-title", &playlist.id), |ui| {
                                    label(
                                        ui,
                                        &playlist.title,
                                        18.,
                                        true,
                                        if playing {
                                            Color32::from_rgb(239, 197, 20)
                                        } else {
                                            TEXT
                                        },
                                    );
                                    ui.add_space(3.);
                                    label(
                                        ui,
                                        format!("{} items", playlist.count),
                                        15.,
                                        false,
                                        GREY,
                                    );
                                });
                                ui.interact(
                                    text,
                                    Id::new(("library-open", &playlist.id)),
                                    Sense::click(),
                                )
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            } else {
                                false
                            };
                            let control_clicked = self.card_controls(
                                ui,
                                cover,
                                ui.rect_contains_pointer(row),
                                &playlist.id,
                                true,
                            );
                            if !control_clicked
                                && (response.clicked() || art.clicked() || text_clicked)
                            {
                                self.page = Page::Playlist(playlist.id.clone());
                                self.search_preview = false;
                                self.send(Command::Playlist(playlist.id.clone()));
                            }
                            art.context_menu(|ui| {
                                self.collection_menu(
                                    ui,
                                    &track,
                                    &playlist.title,
                                    &playlist.creator,
                                    &playlist.id,
                                )
                            });
                            response.clone().on_hover_text(&playlist.title);
                            response.clone().context_menu(|ui| {
                                self.collection_menu(
                                    ui,
                                    &track,
                                    &playlist.title,
                                    &playlist.creator,
                                    &playlist.id,
                                );
                                if ui.button("Sync playlists").clicked() {
                                    self.send(Command::Sync);
                                    ui.close();
                                }
                                if playlist.owned && ui.button("Edit playlist").clicked() {
                                    self.dialog = Some(Dialog {
                                        kind: "edit".into(),
                                        id: playlist.id.clone(),
                                        title: playlist.title.clone(),
                                        description: playlist.description.clone(),
                                        public: playlist.public,
                                        cover: playlist.image.clone(),
                                        ..Default::default()
                                    });
                                    ui.close();
                                }
                            });
                        }
                    });
            });
        if !collapsed && !moving {
            self.sidebar_expanded_width = panel.response.rect.width().clamp(265., maximum);
        }
    }
}
