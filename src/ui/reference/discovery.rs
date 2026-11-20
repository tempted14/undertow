use super::interactions::{ListAction, artwork_play_overlay};
use super::*;
use crate::discovery::{Feed, Item, Section};

fn shelf_arrow(ui: &mut egui::Ui, glyph: &str, enabled: bool) -> egui::Response {
    let (r, response) = ui.allocate_exact_size(
        vec2(32., 32.),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    ui.painter().circle_filled(
        r.center(),
        16.,
        if response.hovered() && enabled {
            design::HOVER
        } else {
            design::SURFACE
        },
    );
    paint_icon(
        ui.painter(),
        r.shrink(5.),
        glyph,
        if enabled { TEXT } else { GREY },
    );
    response.on_hover_cursor(if enabled {
        egui::CursorIcon::PointingHand
    } else {
        egui::CursorIcon::Default
    })
}

#[derive(Default)]
pub(in crate::ui) struct State {
    pub feed: Arc<Feed>,
    pub attempted: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub loaded: Option<Instant>,
    pub offsets: HashMap<String, usize>,
    pub expanded: HashMap<String, Arc<Section>>,
    pub pending: HashSet<String>,
    pub errors: HashMap<String, String>,
    pub lists: HashMap<String, Playlist>,
}
impl State {
    pub(in crate::ui) fn remember_list(&mut self, list: Playlist) {
        if self.lists.len() >= 16
            && !self.lists.contains_key(&list.id)
            && let Some(key) = self.lists.keys().next().cloned()
        {
            self.lists.remove(&key);
        }
        self.lists.insert(list.id.clone(), list);
    }
    pub fn home_result(&mut self, result: Result<Feed, String>) {
        self.loading = false;
        self.attempted = true;
        match result {
            Ok(feed) => {
                self.feed = Arc::new(feed);
                self.loaded = Some(Instant::now());
                self.error = None;
                self.offsets.clear();
                self.expanded.clear();
            }
            Err(e) => self.error = Some(e),
        }
    }
    pub fn section_result(&mut self, id: String, result: Result<Section, String>) {
        self.pending.remove(&id);
        match result {
            Ok(section) => {
                self.errors.remove(&id);
                // Only a few expanded pages are retained; Home shares immutable items.
                if self.expanded.len() >= 4
                    && !self.expanded.contains_key(&id)
                    && let Some(old) = self.expanded.keys().next().cloned()
                {
                    self.expanded.remove(&old);
                }
                self.expanded.insert(id, Arc::new(section));
            }
            Err(e) => {
                self.errors.insert(id, e);
            }
        }
    }
}

impl App {
    fn discovery_item_menu(&mut self, ui: &mut egui::Ui, item: &Item) {
        if let Some(track) = &item.catalog.track {
            self.track_menu(ui, track, 0, &Arc::new(vec![track.clone()]), None, false);
        } else if matches!(item.catalog.kind.as_str(), "Album" | "Playlist") {
            self.collection_menu(
                ui,
                &Track {
                    cover: item.catalog.cover.clone(),
                    album: item.catalog.title.clone(),
                    ..Default::default()
                },
                &item.catalog.title,
                &item.catalog.artist,
                &item.key(),
            );
        } else if item.catalog.kind == "Mix" {
            if ui.button("Play now").clicked() {
                self.list_action(item.key(), ListAction::Play);
                ui.close();
            }
            if ui.button("Play next").clicked() {
                self.list_action(item.key(), ListAction::Next);
                ui.close();
            }
        }
    }
    pub(in crate::ui) fn refresh_discovery(&mut self) {
        if !self.connected || self.discovery.loading {
            return;
        }
        self.discovery.attempted = true;
        self.discovery.loading = true;
        self.discovery.error = None;
        self.send(Command::Home);
    }
    fn open_discovery_item(&mut self, item: &Item) {
        if let Some(list) = &item.list {
            self.discovery.remember_list(list.clone());
        }
        match item.catalog.kind.as_str() {
            "My Tracks" => {
                self.page = Page::Favorites;
                self.send(Command::Favorites);
            }
            "Mix" => {
                self.page = Page::Mix(item.catalog.id.clone());
                self.send(Command::Mix(item.catalog.id.clone()));
            }
            _ => self.catalog_activate(&item.catalog),
        }
    }
    fn open_discovery_section(&mut self, source: &Section) {
        self.page = Page::Discovery(source.id.clone());
        if !self.discovery.expanded.contains_key(&source.id)
            && self.discovery.pending.insert(source.id.clone())
        {
            self.discovery.errors.remove(&source.id);
            self.send(Command::DiscoverySection(source.clone()));
        }
    }
    pub(super) fn discovery_home(&mut self, ui: &mut egui::Ui) {
        if !self.discovery.attempted
            || (self.discovery.error.is_none()
                && self
                    .discovery
                    .loaded
                    .is_some_and(|at| at.elapsed() > Duration::from_secs(900)))
        {
            self.refresh_discovery();
        }
        if self.discovery.loading && self.discovery.feed.sections.is_empty() {
            label(ui, "Loading recommendations…", 16., false, GREY);
            ui.add_space(12.);
            design::skeleton(ui, ui.available_width());
        }
        if let Some(error) = self.discovery.error.clone() {
            ui.add_space(12.);
            label(ui, error, 16., false, GREY);
            if small_pill(ui, "Try again").clicked() {
                self.refresh_discovery();
            }
        }
        let feed = self.discovery.feed.clone();
        for section in &feed.sections {
            ui.push_id(&section.id, |ui| {
                if section.shortcuts {
                    self.discovery_rows(ui, &section.items, true, 6);
                } else {
                    let cols = columns(ui.available_width(), 208., 20., 8);
                    let mut offset = *self.discovery.offsets.get(&section.id).unwrap_or(&0);
                    let last = section.items.len().saturating_sub(cols);
                    offset = offset.min(last);
                    ui.horizontal(|ui| {
                        label(ui, &section.title, design::SECTION, true, TEXT);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("View all").font(font(17., true)).color(GREY),
                                    )
                                    .frame(false),
                                )
                                .clicked()
                            {
                                self.open_discovery_section(section);
                            }
                            if !section.rows && section.items.len() > cols {
                                if shelf_arrow(ui, "forward", offset < last).clicked() {
                                    offset = offset.saturating_add(cols).min(last);
                                }
                                if shelf_arrow(ui, "back", offset > 0).clicked() {
                                    offset = offset.saturating_sub(cols);
                                }
                            }
                        });
                    });
                    self.discovery.offsets.insert(section.id.clone(), offset);
                    if !section.subtitle.is_empty() {
                        label(ui, &section.subtitle, 16., false, GREY);
                    }
                    ui.add_space(14.);
                    if section.rows {
                        self.discovery_rows(ui, &section.items, false, 9);
                    } else {
                        let size = (ui.available_width() - 20. * (cols - 1) as f32) / cols as f32;
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 20.;
                            for item in section.items.iter().skip(offset).take(cols) {
                                self.discovery_card(ui, item, size);
                            }
                        });
                    }
                }
            });
            ui.add_space(if section.shortcuts { 46. } else { 48. });
        }
    }
    fn discovery_card(&mut self, ui: &mut egui::Ui, item: &Item, size: f32) {
        ui.push_id(item.key(), |ui| {
            let (r, response) = ui.allocate_exact_size(vec2(size, size + 65.), Sense::click());
            let art = Track {
                cover: item.catalog.cover.clone(),
                ..Default::default()
            };
            if !ui.is_rect_visible(r) {
                self.prefetch_cover_near(ui, r, &art, size);
                return;
            }
            design::row_hover(ui, &response, Color32::TRANSPARENT);
            if item.catalog.kind != "Artist" {
                response.context_menu(|ui| self.discovery_item_menu(ui, item));
            }
            let cover = Rect::from_min_size(r.min, vec2(size, size));
            let art_response = at(ui, cover, "art", |ui| {
                self.cover_with_radius(
                    ui,
                    &art,
                    size,
                    if item.catalog.kind == "Artist" {
                        (size * 0.5) as u8
                    } else {
                        design::ART
                    },
                )
            });
            let controls = if matches!(item.catalog.kind.as_str(), "Album" | "Playlist") {
                self.card_controls(ui, cover, &response, &item.key(), false)
            } else {
                let clicked = artwork_play_overlay(ui, &response, cover, false)
                    .is_some_and(|(play, _)| play.clicked());
                if clicked && item.catalog.kind == "Mix" {
                    self.list_action(item.key(), ListAction::Play);
                } else if clicked {
                    self.open_discovery_item(item);
                }
                clicked
            };
            at(
                ui,
                Rect::from_min_max(r.min + vec2(0., size + 12.), r.max),
                "label",
                |ui| {
                    ui.spacing_mut().item_spacing.y = 3.;
                    ui.horizontal(|ui| {
                        label(ui, &item.catalog.title, 17., true, TEXT);
                        if item.catalog.explicit {
                            self.explicit_badge(ui);
                        }
                    });
                    label(ui, &item.catalog.artist, 15., false, GREY);
                },
            );
            let title = ui
                .interact(
                    Rect::from_min_max(r.min + vec2(0., size), r.max),
                    Id::new(("discovery-title", item.key())),
                    Sense::click(),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if !controls && (response.clicked() || art_response.clicked() || title.clicked()) {
                self.open_discovery_item(item);
            }
        });
    }
    fn discovery_rows(&mut self, ui: &mut egui::Ui, items: &[Item], shortcuts: bool, max: usize) {
        let gap = if shortcuts { 10. } else { 42. };
        let cols = columns(
            ui.available_width(),
            if shortcuts { 198. } else { 285. },
            gap,
            3,
        );
        let width = (ui.available_width() - gap * (cols - 1) as f32) / cols as f32;
        egui::Grid::new(("discovery-rows", shortcuts))
            .num_columns(cols)
            .spacing(vec2(gap, if shortcuts { 10. } else { 8. }))
            .show(ui, |ui| {
                for (i, item) in items.iter().take(max).enumerate() {
                    ui.push_id(item.key(), |ui| {
                        let (r, response) = ui.allocate_exact_size(
                            vec2(width, if shortcuts { 79. } else { 86. }),
                            Sense::click(),
                        );
                        let size = if shortcuts { 58. } else { 74. };
                        let art = Track {
                            cover: item.catalog.cover.clone(),
                            ..Default::default()
                        };
                        if !ui.is_rect_visible(r) {
                            self.prefetch_cover_near(ui, r, &art, size);
                            return;
                        }
                        let has_menu =
                            !matches!(item.catalog.kind.as_str(), "Artist" | "My Tracks");
                        if has_menu {
                            response.context_menu(|ui| self.discovery_item_menu(ui, item));
                        }
                        let hovered = design::row_hover(
                            ui,
                            &response,
                            if shortcuts {
                                design::SURFACE
                            } else {
                                Color32::TRANSPARENT
                            },
                        );
                        let padding = if shortcuts { 10. } else { 0. };
                        let cover = Rect::from_min_size(
                            pos2(r.left() + padding, r.center().y - size * 0.5),
                            vec2(size, size),
                        );
                        let art_clicked = if item.catalog.kind == "My Tracks" {
                            let mut colors = egui::Mesh::default();
                            for (point, color) in [
                                (cover.left_top(), Color32::from_rgb(83, 112, 192)),
                                (cover.right_top(), Color32::from_rgb(76, 171, 223)),
                                (cover.left_bottom(), Color32::from_rgb(115, 94, 190)),
                                (cover.right_bottom(), Color32::from_rgb(85, 30, 112)),
                            ] {
                                colors.colored_vertex(point, color);
                            }
                            colors.add_triangle(0, 1, 2);
                            colors.add_triangle(1, 3, 2);
                            ui.painter().add(egui::Shape::mesh(colors));
                            paint_icon(
                                ui.painter(),
                                Rect::from_min_size(
                                    cover.right_top() - vec2(17., -4.),
                                    vec2(12., 12.),
                                ),
                                "heart",
                                TEXT,
                            );
                            ui.painter().text(
                                cover.left_bottom() + vec2(4., -5.),
                                Align2::LEFT_BOTTOM,
                                "My Tracks",
                                font(9., true),
                                TEXT,
                            );
                            false
                        } else {
                            let art_button = at(ui, cover, "art", |ui| {
                                self.cover_with_radius(
                                    ui,
                                    &art,
                                    size,
                                    if item.catalog.kind == "Artist" {
                                        (size / 2.) as u8
                                    } else {
                                        3
                                    },
                                )
                            });
                            let clicked = art_button.clicked();
                            if has_menu {
                                art_button.context_menu(|ui| self.discovery_item_menu(ui, item));
                            }
                            clicked
                        };
                        let text = Rect::from_min_max(
                            r.min
                                + vec2(
                                    size + padding + 16.,
                                    if item.catalog.artist.is_empty() {
                                        24.
                                    } else {
                                        10.
                                    },
                                ),
                            r.max - vec2(48., 3.),
                        );
                        at(ui, text, "name", |ui| {
                            label(ui, &item.catalog.title, 16., true, TEXT);
                            if !item.catalog.artist.is_empty() {
                                ui.add_space(3.);
                                let subtitle = if shortcuts && item.catalog.kind == "Playlist" {
                                    if self
                                        .playlists
                                        .iter()
                                        .any(|p| p.id == item.catalog.id && p.owned)
                                    {
                                        "Created by me".into()
                                    } else {
                                        format!("Created by {}", item.catalog.artist)
                                    }
                                } else {
                                    item.catalog.artist.clone()
                                };
                                label(ui, subtitle, 14., false, GREY);
                            }
                        });
                        let played = match item.catalog.kind.as_str() {
                            "Album" | "Playlist" | "My Tracks" => {
                                self.card_controls(ui, cover, &response, &item.key(), true)
                            }
                            "Track" | "Mix" => {
                                let clicked = artwork_play_overlay(ui, &response, cover, true)
                                    .is_some_and(|(play, _)| play.clicked());
                                if clicked && item.catalog.kind == "Mix" {
                                    self.list_action(item.key(), ListAction::Play);
                                } else if clicked {
                                    self.open_discovery_item(item);
                                }
                                clicked
                            }
                            _ => false,
                        };
                        let title = ui
                            .interact(
                                text,
                                Id::new(("discovery-row-title", item.key())),
                                Sense::click(),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        let liked = shortcuts
                            && item.catalog.kind == "Playlist"
                            && self.inline_favorite(ui, r, &item.key());
                        let mut menu_clicked = false;
                        if !shortcuts && has_menu {
                            let menu_rect = Rect::from_center_size(
                                r.right_center() - vec2(23., 0.),
                                Vec2::splat(34.),
                            );
                            let more = ui
                                .interact(
                                    menu_rect,
                                    response.id.with("recent-menu"),
                                    Sense::click(),
                                )
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .on_hover_text("More");
                            if hovered {
                                paint_icon(ui.painter(), menu_rect.shrink(7.), "dots", GREY);
                            }
                            menu_clicked = more.clicked();
                            egui::Popup::menu(&more).show(|ui| self.discovery_item_menu(ui, item));
                        }
                        if !played
                            && !liked
                            && !menu_clicked
                            && (response.clicked() || title.clicked() || art_clicked)
                        {
                            self.open_discovery_item(item);
                        }
                    });
                    if (i + 1) % cols == 0 {
                        ui.end_row();
                    }
                }
            });
    }
    pub(in crate::ui) fn reference_discovery(&mut self, ui: &mut egui::Ui, id: &str) {
        let section = self.discovery.expanded.get(id).cloned().or_else(|| {
            self.discovery
                .feed
                .sections
                .iter()
                .find(|s| s.id == id)
                .map(|s| Arc::new(s.clone()))
        });
        let Some(section) = section else {
            label(
                ui,
                "Return to Music to refresh this section.",
                18.,
                false,
                GREY,
            );
            return;
        };
        super::catalog::catalog_canvas(ui, id, 40., 500., |ui| {
            label(ui, &section.title, 34., true, TEXT);
            ui.add_space(10.);
            if self.discovery.pending.contains(id) {
                label(ui, "Loading more from TIDAL…", 16., false, GREY);
            }
            if let Some(error) = self.discovery.errors.get(id).cloned() {
                label(ui, error, 16., false, GREY);
                if small_pill(ui, "Try again").clicked() {
                    self.open_discovery_section(&section);
                }
            }
            ui.add_space(24.);
            if section.rows {
                self.discovery_rows(ui, &section.items, false, section.items.len());
            } else {
                let cols = columns(ui.available_width(), 172., 18., 7);
                let size = (ui.available_width() - 18. * (cols - 1) as f32) / cols as f32;
                egui::Grid::new(("discovery-expanded", id))
                    .num_columns(cols)
                    .spacing(vec2(22., 28.))
                    .show(ui, |ui| {
                        for (i, item) in section.items.iter().enumerate() {
                            self.discovery_card(ui, item, size);
                            if (i + 1) % cols == 0 {
                                ui.end_row();
                            }
                        }
                    });
            }
        });
    }
    pub(in crate::ui) fn reference_mix(&mut self, ui: &mut egui::Ui, id: &str) {
        let key = format!("mix:{id}");
        let tracks = self.tracks.get(&key).cloned().unwrap_or_default();
        let list = self
            .discovery
            .lists
            .get(&key)
            .cloned()
            .unwrap_or_else(|| Playlist {
                id: key.clone(),
                title: "TIDAL Mix".into(),
                ..Default::default()
            });
        super::catalog::catalog_canvas(ui, &key, 40., 700., |ui| {
            ui.horizontal(|ui| {
                self.cover(
                    ui,
                    &Track {
                        cover: list.image.clone(),
                        ..Default::default()
                    },
                    220.,
                );
                ui.vertical(|ui| {
                    label(ui, &list.title, 36., true, TEXT);
                    label(ui, &list.description, 17., false, GREY);
                });
            });
            ui.add_space(30.);
            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 62.), Sense::hover());
            self.large_play_actions(ui, r, &tracks);
            ui.add_space(30.);
            if tracks.is_empty() {
                if let Some(error) = self.discovery.errors.get(&key).cloned() {
                    label(ui, error, 17., false, GREY);
                    if small_pill(ui, "Try again").clicked() {
                        self.discovery.errors.remove(&key);
                        self.send(Command::Mix(id.into()));
                    }
                } else {
                    label(ui, "Loading mix from TIDAL…", 17., false, GREY);
                }
            }
            self.catalog_track_table(ui, tracks, Some(&list), false);
        });
    }
}
