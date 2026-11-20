use super::*;

impl App {
    pub(super) fn navigation_sidebar(&mut self, ctx: &egui::Context) {
        if !self.settings.show_sidebar || self.page == Page::Lyrics {
            return;
        }
        // Keep the user's full-width preference; narrow windows temporarily use the rail.
        let collapsed = self.settings.sidebar_collapsed || ctx.content_rect().width() < 980.;
        let maximum = (ctx.content_rect().width() - 680.).clamp(234., 390.);
        let expanded = self.sidebar_expanded_width.min(maximum);
        let progress = ctx.animate_bool_with_time(
            Id::new("sidebar-motion"),
            !collapsed,
            if self.settings.animated_lyrics {
                0.22 / self.settings.motion_speed
            } else {
                0.
            },
        );
        let width = 68. + (expanded - 68.) * layout::ease(progress);
        let moving = progress > 0. && progress < 1.;
        let panel = egui::SidePanel::left("library-navigation")
            .resizable(!collapsed && !moving)
            .default_width(width)
            .width_range(if moving || collapsed {
                width..=width
            } else {
                234. ..=maximum
            })
            .frame(
                Frame::new()
                    .fill(design::SIDEBAR)
                    .inner_margin(Margin::symmetric(if collapsed { 9 } else { 21 }, 0)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = vec2(0., 0.);
                let (header, _) =
                    ui.allocate_exact_size(vec2(ui.available_width(), 78.), Sense::hover());
                if !collapsed {
                    design::mark(
                        ui.painter(),
                        Rect::from_center_size(
                            header.left_center() + vec2(27., 0.),
                            vec2(36., 24.),
                        ),
                    );
                }
                let toggle = Rect::from_center_size(
                    if collapsed {
                        header.center()
                    } else {
                        header.right_center() - vec2(5., 0.)
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
                    ("music", "Music", Page::Home),
                    ("collection", "Collection", Page::Favorites),
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
                    let b = b.on_hover_cursor(egui::CursorIcon::PointingHand);
                    if selected || b.hovered() {
                        ui.painter().rect_filled(
                            r,
                            16,
                            if selected {
                                design::SELECTED
                            } else {
                                design::HOVER
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
                        Rect::from_center_size(c, vec2(26., 26.)),
                        glyph,
                        if selected { design::BRAND } else { GREY },
                    );
                    if !collapsed {
                        ui.painter().text(
                            pos2(r.left() + 53., r.center().y),
                            Align2::LEFT_CENTER,
                            title,
                            font(17., true),
                            if selected { TEXT } else { GREY },
                        );
                        if glyph == "collection" {
                            paint_icon(
                                ui.painter(),
                                Rect::from_center_size(
                                    r.right_center() - vec2(20., 0.),
                                    vec2(18., 18.),
                                ),
                                "forward",
                                GREY,
                            );
                        }
                    }
                    if b.clicked() {
                        self.page = page.clone();
                        self.search_preview = false;
                        if self.connected && page == Page::Favorites {
                            self.send(Command::Favorites);
                        }
                    }
                    b.on_hover_text(title);
                }
                ui.add_space(if collapsed { 16. } else { 26. });
                if !collapsed {
                    let (r, _) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 34.), Sense::hover());
                    ui.painter().text(
                        r.left_center() + vec2(3., 0.),
                        Align2::LEFT_CENTER,
                        "All playlists",
                        font(17., true),
                        GREY,
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
                                Arc::make_mut(&mut self.playlists).reverse();
                            }
                        }
                    }
                    ui.add_space(12.);
                }
                let lists = self.playlists.clone();
                let row_height = if collapsed { 52. } else { 68. };
                egui::ScrollArea::vertical()
                    .id_salt("library-playlists")
                    .max_height((ui.available_height() - reserved_player(ctx) - 18.).max(80.))
                    .auto_shrink([false, false])
                    .show_rows(ui, row_height, lists.len(), |ui, range| {
                        for playlist in &lists[range] {
                            let (row, response) = ui.allocate_exact_size(
                                vec2(ui.available_width(), row_height),
                                Sense::click(),
                            );
                            let selected =
                                matches!(&self.page,Page::Playlist(id)if id==&playlist.id);
                            let playing = self.playing_playlist.as_ref() == Some(&playlist.id)
                                && self.audio.is_some()
                                && !self.paused();
                            design::row_hover(
                                ui,
                                &response,
                                if selected {
                                    design::SELECTED
                                } else {
                                    Color32::TRANSPARENT
                                },
                            );
                            let size = if collapsed { 38. } else { 47. };
                            let cover = Rect::from_center_size(
                                pos2(
                                    if collapsed {
                                        row.center().x
                                    } else {
                                        row.left() + 27.5
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
                                self.cover_with_radius(ui, &track, size, design::ART)
                            });
                            if playing {
                                ui.painter()
                                    .rect_filled(cover, 5, Color32::from_black_alpha(100));
                                paint_icon(ui.painter(), cover.shrink(12.), "volume", ACCENT);
                            }
                            let text = Rect::from_min_max(
                                row.min + vec2(65., 13.),
                                row.max - vec2(6., 0.),
                            );
                            let text_clicked = if !collapsed {
                                at(ui, text, ("library-title", &playlist.id), |ui| {
                                    label(
                                        ui,
                                        &playlist.title,
                                        16.,
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
                                        13.,
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
                            let control_clicked =
                                self.card_controls(ui, cover, &response, &playlist.id, true);
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
            self.sidebar_expanded_width = panel.response.rect.width().clamp(234., maximum);
        }
    }
}
