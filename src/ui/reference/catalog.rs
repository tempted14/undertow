use super::playlist::{AQUA, GOLD, action, bitrate_label, date_label, outline_heart, sample_label};
use super::*;
mod album;
mod table;

pub(super) fn catalog_canvas(
    ui: &mut egui::Ui,
    id: &str,
    margin: f32,
    min_height: f32,
    body: impl FnOnce(&mut egui::Ui),
) -> egui::scroll_area::ScrollAreaOutput<()> {
    let height = (ui.available_height() - reserved_player(ui.ctx()) - 18.).max(120.);
    let mut page_scroll = egui::ScrollArea::vertical().id_salt(("catalog-page", id));
    static DIAGNOSTIC: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *DIAGNOSTIC.get_or_init(|| {
        std::env::args().any(|a| a == "--artwork-scroll-benchmark")
            && std::env::args().any(|a| a == "--background-benchmark")
    }) {
        page_scroll = page_scroll.vertical_scroll_offset(min_height * 0.75);
    }
    page_scroll
        .max_height(height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let origin = ui.cursor().min;
            let rect = Rect::from_min_size(origin, vec2(ui.available_width(), height + min_height));
            let mut content = ui.new_child(egui::UiBuilder::new().id_salt(id).max_rect(
                Rect::from_min_max(rect.min + vec2(margin, 0.), rect.max - vec2(margin, 0.)),
            ));
            content.set_clip_rect(ui.clip_rect());
            body(&mut content);
            let actual_bottom = content.min_rect().bottom().max(origin.y + height);
            ui.advance_cursor_after_rect(Rect::from_min_max(
                origin,
                pos2(rect.right(), actual_bottom),
            ));
        })
}

impl App {
    pub(in crate::ui) fn open_album(&mut self, id: u64, title: String) {
        if id == 0 {
            return;
        }
        self.page = Page::Album(id, title);
        self.search_preview = false;
        self.send(Command::Album(id));
    }
    pub(in crate::ui) fn request_catalog(&mut self) {
        if self.query.trim().is_empty() {
            return;
        }
        self.search_sent = self.query.clone();
        self.send(Command::CatalogSearch(self.query.clone()));
    }
    pub(super) fn search_input_response(
        &mut self,
        ui: &mut egui::Ui,
        response: &egui::Response,
        anchor: Rect,
    ) {
        self.search_anchor = Some(anchor);
        if !self.query.is_empty() {
            let clear =
                Rect::from_center_size(anchor.right_center() - vec2(20., 0.), vec2(24., 26.));
            paint_icon(ui.painter(), clear.shrink(3.), "close", TEXT);
            if ui
                .interact(
                    clear,
                    Id::new(("clear-search", anchor.left().to_bits())),
                    Sense::click(),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                self.query.clear();
                self.search_sent.clear();
                self.catalog = Arc::new(Vec::new());
                self.catalog_query.clear();
                self.search_preview = false;
                response.request_focus();
            }
        }
        if response.changed() {
            self.search_changed = Instant::now();
            self.search_preview = !self.query.trim().is_empty();
        }
        if response.has_focus() && response.clicked() {
            self.search_preview = !self.query.trim().is_empty();
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.search_preview = false;
        }
        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            self.page = Page::Search;
            self.search_preview = false;
            self.request_catalog();
        }
        if self.search_preview && self.query != self.search_sent {
            if self.search_changed.elapsed() >= Duration::from_millis(180) {
                self.request_catalog();
            } else {
                ui.ctx().request_repaint_after(Duration::from_millis(30));
            }
        }
    }
    pub(super) fn catalog_activate(&mut self, item: &CatalogItem) {
        self.search_preview = false;
        match item.kind.as_str() {
            "Track" => {
                if let Some(t) = &item.track {
                    self.play_list(Arc::new(vec![t.clone()]), 0);
                }
            }
            "Artist" => {
                if let Ok(id) = item.id.parse() {
                    self.open_artist(id);
                }
            }
            "Album" => {
                if let Ok(id) = item.id.parse() {
                    self.open_album(id, item.title.clone());
                }
            }
            "Playlist" => {
                if !self.discovery.lists.contains_key(&item.id) {
                    self.discovery.remember_list(Playlist {
                        id: item.id.clone(),
                        title: item.title.clone(),
                        image: item.cover.clone(),
                        creator: item.artist.clone(),
                        ..Default::default()
                    });
                }
                self.page = Page::Playlist(item.id.clone());
                self.send(Command::Playlist(item.id.clone()));
            }
            kind => {
                let _ = crate::auth::open_browser(&format!(
                    "https://tidal.com/browse/{}/{}",
                    kind.to_lowercase(),
                    item.id
                ));
            }
        }
    }
    fn catalog_result(
        &mut self,
        ui: &mut egui::Ui,
        item: &CatalogItem,
        compact: bool,
        index: usize,
    ) {
        let art_size = if compact { 62. } else { 84. };
        let height = if compact { 88. } else { 110. };
        let (r, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
        if !ui.is_rect_visible(r) {
            return;
        }
        if response.hovered() {
            ui.painter()
                .rect_filled(r, 6, Color32::from_white_alpha(12));
        }
        let art = Track {
            cover: item.cover.clone(),
            ..Default::default()
        };
        let cover = Rect::from_min_size(r.min + vec2(10., 9.), vec2(art_size, art_size));
        let art_response = at(ui, cover, ("catalog-art", compact, index), |ui| {
            self.cover_with_radius(
                ui,
                &art,
                art_size,
                if item.kind == "Artist" {
                    (art_size * 0.5) as u8
                } else {
                    3
                },
            )
        });
        let collection_key = if item.kind == "Album" {
            Some(format!("album:{}", item.id))
        } else if item.kind == "Playlist" {
            Some(item.id.clone())
        } else {
            None
        };
        let card_clicked = if let Some(key) = &collection_key {
            self.card_controls(ui, cover, ui.rect_contains_pointer(r), key, true)
        } else {
            false
        };
        let art_clicked = art_response.clicked() && !card_clicked;
        let text = Rect::from_min_max(
            r.min
                + vec2(
                    art_size + 30.,
                    if item.kind == "Artist" { 30. } else { 15. },
                ),
            r.max - vec2(45., 8.),
        );
        at(ui, text, ("catalog-label", compact, index), |ui| {
            ui.horizontal(|ui| {
                label(ui, &item.title, 18., true, TEXT);
                if item.explicit {
                    self.explicit_badge(ui);
                }
            });
            if item.kind != "Artist" {
                label(
                    ui,
                    format!("{}  ·  {}", item.kind, item.artist),
                    17.,
                    false,
                    GREY,
                );
            }
            if !compact && !item.year.is_empty() {
                ui.add_space(9.);
                label(ui, &item.year, 13., true, GREY);
            }
        });
        let menu = at(
            ui,
            Rect::from_min_size(pos2(r.right() - 43., r.top() + 22.), vec2(32., 32.)),
            ("catalog-more", compact, index),
            |ui| icon(ui, "dots", 30., GREY),
        );
        let label_response = ui
            .interact(
                text,
                Id::new(("catalog-open", compact, index)),
                Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let label_clicked = label_response.clicked();
        let mut show_menu = |ui: &mut egui::Ui| {
            if let Some(t) = &item.track {
                self.track_menu(ui, t, 0, &Arc::new(vec![t.clone()]), None, false);
            } else if let Some(key) = &collection_key {
                self.collection_menu(
                    ui,
                    &Track {
                        cover: item.cover.clone(),
                        artist: item.artist.clone(),
                        album: item.title.clone(),
                        ..Default::default()
                    },
                    &item.title,
                    &item.artist,
                    key,
                );
            } else if ui.button("Copy link").clicked() {
                ui.ctx().copy_text(format!(
                    "https://tidal.com/browse/{}/{}",
                    item.kind.to_lowercase(),
                    item.id
                ));
                ui.close();
            }
        };
        egui::Popup::menu(&menu).show(&mut show_menu);
        response.clone().context_menu(&mut show_menu);
        art_response.context_menu(&mut show_menu);
        label_response.clone().context_menu(show_menu);
        if !card_clicked && (response.clicked() || art_clicked || label_clicked) {
            self.catalog_activate(item);
        }
    }
    pub(in crate::ui) fn search_dropdown(&mut self, ctx: &egui::Context) {
        if !self.search_preview || self.query.trim().is_empty() {
            return;
        }
        let Some(anchor) = self.search_anchor else {
            return;
        };
        let width = 520_f32.min(ctx.content_rect().width() - 24.);
        let left = (anchor.right() - width).max(12.);
        let top = anchor.bottom() + 12.;
        let preview_height =
            (ctx.content_rect().bottom() - top - reserved_player(ctx) - 30.).max(160.);
        let mut bounds = Rect::NOTHING;
        egui::Area::new(Id::new("catalog-preview"))
            .order(egui::Order::Foreground)
            .movable(false)
            .default_size(vec2(width, preview_height))
            .fixed_pos(pos2(left, top))
            .show(ctx, |ui| {
                let response = Frame::new()
                    .fill(Color32::from_rgb(23, 24, 27))
                    .stroke(Stroke::new(1_f32, Color32::from_white_alpha(32)))
                    .corner_radius(15)
                    .inner_margin(14)
                    .show(ui, |ui| {
                        ui.set_width(width - 28.);
                        ui.set_height(preview_height - 28.);
                        ui.spacing_mut().item_spacing.y = 0.;
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .max_height(
                                (ctx.content_rect().bottom() - top - reserved_player(ctx) - 30.)
                                    .max(120.),
                            )
                            .show(ui, |ui| {
                                if ui
                                    .add_sized(
                                        [ui.available_width(), 54.],
                                        egui::Button::new(
                                            RichText::new(format!("Search for “{}”", self.query))
                                                .size(18.),
                                        )
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    self.page = Page::Search;
                                    self.search_preview = false;
                                    self.request_catalog();
                                }
                                if self.catalog_query != self.query {
                                    label(ui, "Searching TIDAL…", 17., false, GREY);
                                } else {
                                    let items = self.catalog.clone();
                                    for (i, item) in items.iter().take(24).enumerate() {
                                        self.catalog_result(ui, item, true, i);
                                    }
                                    if items.is_empty() {
                                        label(ui, "No results", 17., false, GREY);
                                    }
                                }
                            });
                    });
                bounds = response.response.rect;
            });
        if ctx.input(|i| {
            i.pointer.any_pressed()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| !bounds.contains(p) && !anchor.contains(p))
        }) {
            self.search_preview = false;
        }
    }
    pub(in crate::ui) fn reference_search(&mut self, ui: &mut egui::Ui) {
        let margin = 46_f32.min(ui.available_width() * 0.05);
        let r = Rect::from_min_max(
            pos2(ui.max_rect().left() + margin, ui.cursor().top() + 24.),
            ui.max_rect().max - vec2(margin, reserved_player(ui.ctx()) + 18.),
        );
        let view_revision = self.catalog_view.revision();
        at(ui, r, "catalog-results", |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 10.;
                for tab in ["Top results", "Profiles", "Albums", "Tracks", "Playlists"] {
                    let active = self.search_tab == tab;
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(tab).font(font(18., true)).color(if active {
                                    Color32::BLACK
                                } else {
                                    TEXT
                                }),
                            )
                            .fill(if active {
                                Color32::WHITE
                            } else {
                                Color32::from_gray(19)
                            })
                            .corner_radius(28)
                            .min_size(vec2(104., 52.)),
                        )
                        .clicked()
                    {
                        self.search_tab = tab.into();
                    }
                }
            });
            ui.add_space(26.);
            let result_count = self
                .catalog_view
                .get(&self.catalog, &self.search_tab, self.search_sort)
                .len();
            ui.horizontal(|ui| {
                label(ui, format!("{result_count} results"), 14., false, GREY);
                egui::ComboBox::from_id_salt("catalog-sort")
                    .selected_text(
                        ["Relevance", "Title A–Z", "Artist A–Z", "Newest first"][self.search_sort],
                    )
                    .show_ui(ui, |ui| {
                        for (i, text) in ["Relevance", "Title A–Z", "Artist A–Z", "Newest first"]
                            .iter()
                            .enumerate()
                        {
                            ui.selectable_value(&mut self.search_sort, i, *text);
                        }
                    });
            });
            ui.add_space(20.);
            if self.catalog_query != self.query {
                label(ui, "Searching TIDAL…", 18., false, GREY);
                return;
            }
            let items = self.catalog.clone();
            let indices = self
                .catalog_view
                .get(&items, &self.search_tab, self.search_sort);
            if indices.is_empty() {
                label(ui, "No matching results", 18., false, GREY);
            }
            ui.spacing_mut().item_spacing.y = 0.;
            let mut scroll = egui::ScrollArea::vertical().id_salt("catalog-list");
            if self.catalog_view.revision() != view_revision {
                scroll = scroll.vertical_scroll_offset(0.);
            }
            scroll
                .max_height(ui.available_height())
                .auto_shrink([false, false])
                .show_rows(ui, 110., indices.len(), |ui, range| {
                    for i in range {
                        self.catalog_result(ui, &items[indices[i]], false, indices[i]);
                    }
                });
        });
    }
    pub(super) fn explicit_badge(&self, ui: &mut egui::Ui) {
        Frame::new()
            .fill(TEXT)
            .corner_radius(3)
            .inner_margin(Margin::symmetric(5, 2))
            .show(ui, |ui| {
                label(ui, "E", 11., true, Color32::BLACK);
            });
    }
    pub(super) fn track_title(&self, ui: &mut egui::Ui, t: &Track, color: Color32) {
        let hires = self.settings.show_quality
            && (t.quality.contains("HI_RES") || t.quality.contains("HIRES"));
        let reserved = if t.explicit { 26. } else { 0. } + if hires { 64. } else { 0. };
        let natural = ui
            .painter()
            .layout_no_wrap(t.title.clone(), font(18., true), color)
            .size()
            .x;
        let title_width = natural.min((ui.available_width() - reserved - 2.).max(24.));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.;
            ui.add_sized(
                vec2(title_width, 30.),
                egui::Label::new(RichText::new(&t.title).font(font(18., true)).color(color))
                    .truncate(),
            );
            if t.explicit {
                self.explicit_badge(ui);
            }
            if hires {
                Frame::new()
                    .fill(Color32::from_gray(30))
                    .corner_radius(7)
                    .inner_margin(Margin::symmetric(10, 6))
                    .show(ui, |ui| {
                        label(ui, "HiRes", 13., true, GOLD);
                    });
            }
        });
    }
    pub(in crate::ui) fn resize_edges(&self, ctx: &egui::Context) {
        Self::window_resize_edges(ctx);
    }
    fn window_resize_edges(ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
            return;
        }
        let r = ctx.content_rect();
        ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            Id::new("resize-grip"),
        ))
        .line_segment(
            [r.max - vec2(21., 7.), r.max - vec2(7., 21.)],
            Stroke::new(1_f32, GREY),
        );
        if let Some((direction, cursor)) = ctx
            .pointer_latest_pos()
            .and_then(|pointer| layout::resize_hit(r, pointer))
        {
            ctx.set_cursor_icon(cursor);
            if ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary)) {
                ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
            }
        }
    }
}

#[cfg(test)]
mod resize_tests {
    use super::*;
    #[test]
    fn resizing_starts_on_the_first_edge_press_without_a_previous_hover_frame() {
        let ctx = egui::Context::default();
        let pointer = pos2(5., 240.);
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(640., 480.))),
                events: vec![
                    egui::Event::PointerMoved(pointer),
                    egui::Event::PointerButton {
                        pos: pointer,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            App::window_resize_edges,
        );
        assert!(
            output
                .viewport_output
                .values()
                .flat_map(|v| v.commands.iter())
                .any(|command| matches!(
                    command,
                    egui::ViewportCommand::BeginResize(egui::ResizeDirection::West)
                ))
        );
    }
    #[test]
    fn tall_catalog_content_remains_reachable_in_short_and_wide_windows() {
        let ctx = egui::Context::default();
        for size in [
            vec2(640., 480.),
            vec2(840., 600.),
            vec2(1100., 700.),
            vec2(1600., 1000.),
            vec2(2560., 1440.),
        ] {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let result = catalog_canvas(ui, "resize-test", 24., 740., |ui| {
                            ui.allocate_exact_size(
                                vec2(ui.available_width(), 2000.),
                                Sense::hover(),
                            );
                        });
                        assert!(
                            result.content_size.y >= 2000.,
                            "content must not be stranded beyond a fixed canvas"
                        );
                        assert!(result.inner_rect.bottom() <= size.y - reserved_player(ctx));
                    });
                },
            );
        }
    }
}
