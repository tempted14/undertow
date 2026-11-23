use super::*;
use crate::discovery::Item;

#[derive(Clone, Copy, PartialEq)]
pub(in crate::ui) enum View {
    Mixes,
    Playlists,
    Albums,
}

#[derive(Clone, Copy)]
enum Destination {
    View(View),
    Tracks,
    Later,
}

const ENTRIES: [(Destination, &str, &str); 5] = [
    (Destination::View(View::Mixes), "Mixes & Radio", "radio"),
    (
        Destination::View(View::Playlists),
        "Playlists",
        "list-music",
    ),
    (Destination::View(View::Albums), "Albums", "album"),
    (Destination::Tracks, "Tracks", "music"),
    (Destination::Later, "Save for Later", "bookmark"),
];

#[derive(Default)]
pub(in crate::ui) struct Flyout {
    pub(in crate::ui) open: bool,
    pinned: bool,
    suppress_hover: bool,
    deadline: Option<f64>,
}

impl Flyout {
    pub(in crate::ui) fn pin(&mut self) {
        self.open = true;
        self.pinned = true;
        self.deadline = None;
    }
    fn close(&mut self) {
        *self = Self {
            suppress_hover: true,
            ..Default::default()
        };
    }

    fn hover(&mut self, now: f64, trigger: bool, inside: bool) -> Option<f64> {
        if !trigger {
            self.suppress_hover = false;
        }
        let delay = if self.pinned
            || (self.open && inside)
            || (!self.open && (!trigger || self.suppress_hover))
        {
            self.deadline = None;
            return None;
        } else if self.open {
            0.18
        } else {
            0.10
        };
        let deadline = *self.deadline.get_or_insert(now + delay);
        if now >= deadline {
            self.open = !self.open;
            self.deadline = None;
            None
        } else {
            Some(deadline - now)
        }
    }
}

fn bounds(screen: Rect, trigger: Rect, sidebar_right: f32, footer: f32) -> Rect {
    let top = screen.top() + design::TITLE_BAR + 4.;
    let bottom = (screen.bottom() - footer - 6.).max(top + 1.);
    let height = (ENTRIES.len() as f32 * 52. + 12.).min(bottom - top);
    let width = 254f32.min((screen.right() - sidebar_right - 12.).max(1.));
    Rect::from_min_size(
        pos2(
            sidebar_right + 4.,
            trigger.top().clamp(top, bottom - height),
        ),
        vec2(width, height),
    )
}

fn entry_id(index: usize) -> Id {
    Id::new(("collection-destination", index))
}

impl App {
    pub(super) fn sidebar_collection(&mut self, trigger: &egui::Response, sidebar_right: f32) {
        let ctx = &trigger.ctx;
        let rect = bounds(
            ctx.content_rect(),
            trigger.rect,
            sidebar_right,
            reserved_player(ctx),
        );
        let bridge = Rect::from_min_max(
            trigger.rect.right_top() - vec2(1., 0.),
            pos2(rect.left() + 2., trigger.rect.bottom()),
        );
        let (pointer, pressed, now) =
            ctx.input(|i| (i.pointer.hover_pos(), i.pointer.any_pressed(), i.time));
        let over_trigger = pointer.is_some_and(|p| trigger.rect.contains(p));
        let over_menu = pointer.is_some_and(|p| rect.expand(4.).contains(p));
        let inside = over_trigger || over_menu || pointer.is_some_and(|p| bridge.contains(p));
        let first = trigger.has_focus()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight));
        let focused = ctx.memory(|m| m.focused());
        let selected = (0..ENTRIES.len()).find(|&index| focused == Some(entry_id(index)));
        let escape = self.collection_flyout.open
            && ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                    || (selected.is_some()
                        && i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft))
            });
        let flyout = &mut self.collection_flyout;
        if escape || (flyout.open && pressed && !over_trigger && !over_menu) {
            flyout.close();
            if escape {
                trigger.request_focus();
            }
        } else if first || trigger.clicked() {
            if flyout.pinned && !first {
                flyout.close();
            } else {
                flyout.pin();
                if first
                    || ctx.input(|i| {
                        i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Space)
                    })
                {
                    ctx.memory_mut(|m| {
                        m.move_focus(egui::FocusDirection::None);
                        m.request_focus(entry_id(0));
                    });
                } else {
                    trigger.request_focus();
                }
            }
        } else if let Some(delay) = flyout.hover(
            now,
            over_trigger && trigger.hovered(),
            inside || selected.is_some(),
        ) {
            ctx.request_repaint_after(Duration::from_secs_f64(delay));
        }
        let open = flyout.open;
        let speed = if self.settings.animated_lyrics {
            self.settings.motion_speed
        } else {
            f32::INFINITY
        };
        let amount = layout::ease(ctx.animate_bool_with_time(
            Id::new("collection-slide"),
            open,
            0.17 / speed,
        ));
        if amount <= 0. {
            return;
        }
        if open && (selected.is_some() || trigger.has_focus()) {
            let step = ctx.input_mut(|i| {
                if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                    1
                } else if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                    -1
                } else {
                    0
                }
            });
            if step != 0 {
                let next = selected.map_or(if step > 0 { 0 } else { ENTRIES.len() - 1 }, |index| {
                    (index as isize + step).rem_euclid(ENTRIES.len() as isize) as usize
                });
                ctx.memory_mut(|m| {
                    m.move_focus(egui::FocusDirection::None);
                    m.request_focus(entry_id(next));
                });
            }
        }
        let area = egui::Area::new(Id::new("sidebar-collection"))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min - vec2(7. * (1. - amount), 0.))
            .constrain(false)
            .movable(false)
            .interactable(open)
            .show(ctx, |ui| {
                ui.set_opacity(amount);
                if !open {
                    ui.disable();
                }
                let (surface, _) = ui.allocate_exact_size(rect.size(), Sense::hover());
                ui.painter()
                    .rect_filled(surface, design::PANEL, design::POPOVER);
                let padding = (surface.height() * 0.1).min(6.);
                let height = (surface.height() - padding * 2.) / ENTRIES.len() as f32;
                let mut chosen = None;
                for (index, (destination, title, icon)) in ENTRIES.iter().copied().enumerate() {
                    let row = Rect::from_min_size(
                        surface.min + vec2(5., padding + index as f32 * height),
                        vec2((surface.width() - 10.).max(1.), height),
                    );
                    let response = ui
                        .interact(row, entry_id(index), Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Button, open, title)
                    });
                    let selected = match destination {
                        Destination::View(view) => self.page == Page::Collection(view),
                        Destination::Tracks => self.page == Page::Favorites,
                        Destination::Later => self.queue_open && self.show_saved_later,
                    };
                    let hover = ctx.animate_bool_with_time(
                        response.id.with("hover"),
                        response.hovered() || response.has_focus(),
                        0.11 / speed,
                    );
                    if selected || hover > 0. {
                        ui.painter().rect_filled(
                            row,
                            9,
                            Color32::from_white_alpha(if selected {
                                29
                            } else {
                                (23. * hover) as u8
                            }),
                        );
                    }
                    let color = Color32::from_gray(181)
                        .lerp_to_gamma(TEXT, if selected { 1. } else { hover });
                    paint_icon(
                        ui.painter(),
                        Rect::from_center_size(
                            row.left_center() + vec2(26., 0.),
                            Vec2::splat(height.min(25.)),
                        ),
                        icon,
                        color,
                    );
                    ui.painter().text(
                        row.left_center() + vec2(53., 0.),
                        Align2::LEFT_CENTER,
                        title,
                        font(17., true),
                        color,
                    );
                    if response.clicked() {
                        chosen = Some(destination);
                    }
                }
                chosen
            });
        if let Some(destination) = area.inner {
            self.collection_flyout.close();
            self.search_preview = false;
            match destination {
                Destination::View(view) => {
                    self.page = Page::Collection(view);
                    match view {
                        View::Albums if self.connected && self.collection_albums.is_none() => {
                            self.send(Command::CollectionState)
                        }
                        View::Mixes if !self.discovery.attempted => self.refresh_discovery(),
                        _ => {}
                    }
                }
                Destination::Tracks => {
                    self.page = Page::Favorites;
                    if self.connected {
                        self.send(Command::Favorites);
                    }
                }
                Destination::Later => {
                    self.show_saved_later = true;
                    self.queue_open = true;
                }
            }
            ctx.request_repaint();
        }
    }

    pub(in crate::ui) fn collection_view(&mut self, ui: &mut egui::Ui, view: View) {
        let (id, title) = match view {
            View::Mixes => ("collection-mixes", "Mixes & Radio"),
            View::Playlists => ("collection-playlists", "Playlists"),
            View::Albums => ("collection-albums", "Albums"),
        };
        let margin = design::browse_margin(ui.available_width());
        super::catalog::catalog_canvas(ui, id, margin, 260., |ui| {
            ui.add_space(12.);
            ui.horizontal(|ui| {
                label(ui, title, design::TITLE, true, TEXT);
                if view == View::Albums && self.connected {
                    ui.add_space(12.);
                    if small_pill(ui, "Refresh").clicked() {
                        self.send(Command::CollectionState);
                    }
                }
            });
            ui.add_space(28.);
            match view {
                View::Mixes => {
                    let mixes = self.discovery.mixes.clone();
                    if !mixes.is_empty() {
                        collection_grid(ui, mixes.len(), |ui, i, size| {
                            self.discovery_card(ui, &mixes[i], size)
                        });
                    } else if let Some(error) = self.discovery.error.clone() {
                        label(ui, error, 16., false, GREY);
                        if small_pill(ui, "Try again").clicked() {
                            self.refresh_discovery();
                        }
                    } else {
                        label(
                            ui,
                            if self.discovery.loading {
                                "Loading mixes…"
                            } else if self.connected {
                                "TIDAL has no mixes for you yet."
                            } else {
                                "Sign in to see your mixes."
                            },
                            16.,
                            false,
                            GREY,
                        );
                    }
                }
                View::Playlists => {
                    let playlists = self.playlists.clone();
                    if playlists.is_empty() {
                        label(
                            ui,
                            if self.connected {
                                "Your playlists will appear here."
                            } else {
                                "Sign in to see your playlists."
                            },
                            16.,
                            false,
                            GREY,
                        );
                        if self.connected && small_pill(ui, "Create playlist").clicked() {
                            self.dialog = Some(Dialog {
                                kind: "create".into(),
                                ..Default::default()
                            });
                        }
                    } else {
                        collection_grid(ui, playlists.len(), |ui, i, size| {
                            let list = &playlists[i];
                            self.discovery_card(
                                ui,
                                &Item {
                                    catalog: CatalogItem {
                                        kind: "Playlist".into(),
                                        id: list.id.clone(),
                                        title: list.title.clone(),
                                        artist: list.creator.clone(),
                                        cover: list.image.clone(),
                                        year: String::new(),
                                        explicit: false,
                                        track: None,
                                    },
                                    list: Some(list.clone()),
                                },
                                size,
                            );
                        });
                    }
                }
                View::Albums => match self.collection_albums.clone() {
                    Some(Ok(albums)) if !albums.is_empty() => {
                        collection_grid(ui, albums.len(), |ui, i, size| {
                            let album = &albums[i];
                            self.discovery_card(
                                ui,
                                &Item {
                                    catalog: CatalogItem {
                                        kind: "Album".into(),
                                        id: album.id.to_string(),
                                        title: if album.title.is_empty() {
                                            "Album".into()
                                        } else {
                                            album.title.clone()
                                        },
                                        artist: album.artist.clone(),
                                        cover: album.cover.clone(),
                                        year: album.year.clone(),
                                        explicit: album.explicit,
                                        track: None,
                                    },
                                    list: None,
                                },
                                size,
                            );
                        })
                    }
                    Some(Err(error)) => {
                        label(ui, error, 16., false, GREY);
                    }
                    state => {
                        label(
                            ui,
                            if !self.connected {
                                "Sign in to see your saved albums."
                            } else if state.is_none() {
                                "Loading saved albums…"
                            } else {
                                "Albums you save will appear here."
                            },
                            16.,
                            false,
                            GREY,
                        );
                    }
                },
            }
        });
    }
}

fn collection_grid(
    ui: &mut egui::Ui,
    count: usize,
    mut card: impl FnMut(&mut egui::Ui, usize, f32),
) {
    let width = ui.available_width();
    let cols = columns(width, 170., 18., 8);
    let size = (width - 18. * (cols - 1) as f32).max(1.) / cols as f32;
    let pitch = size + 85.;
    let rows = count.div_ceil(cols);
    let clip = ui.clip_rect();
    let top = ui.next_widget_position().y;
    let first = (((clip.top() - top) / pitch).floor() as usize).min(rows);
    let end =
        ((((clip.bottom() - top) / pitch).ceil() as usize).saturating_add(1)).clamp(first, rows);
    ui.spacing_mut().item_spacing.y = 0.;
    ui.add_space(pitch * first as f32);
    for row in first..end {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 18.;
            for i in row * cols..((row + 1) * cols).min(count) {
                card(ui, i, size);
            }
        });
        ui.add_space(20.);
    }
    ui.add_space(pitch * (rows - end) as f32);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flyout_stays_open_across_pointer_gaps_and_respects_dismissal() {
        let mut flyout = Flyout::default();
        assert!(flyout.hover(1., true, true).is_some());
        assert!(!flyout.open);
        flyout.hover(1.11, true, true);
        assert!(flyout.open);

        assert!(flyout.hover(1.2, false, false).is_some());
        flyout.hover(1.3, false, true);
        flyout.hover(1.5, false, true);
        assert!(flyout.open);

        flyout.pin();
        assert!(flyout.hover(2., false, false).is_none());
        assert!(flyout.open);
        flyout.close();
        assert!(flyout.hover(3., true, true).is_none());
        assert!(!flyout.open);
        flyout.hover(3.2, false, false);
        assert!(flyout.hover(3.5, true, true).is_some());
    }

    #[test]
    fn flyout_fits_beside_the_sidebar_and_above_the_player() {
        for (size, sidebar, footer) in [
            (vec2(1600., 1000.), 315., 118.),
            (vec2(640., 480.), 68., 160.),
            (vec2(640., 370.), 68., 160.),
        ] {
            let screen = Rect::from_min_size(Pos2::ZERO, size);
            let trigger = Rect::from_min_size(pos2(10., 168.), vec2(sidebar - 20., 52.));
            let flyout = bounds(screen, trigger, sidebar, footer);
            assert!(screen.contains_rect(flyout));
            assert!(flyout.left() >= sidebar);
            assert!(flyout.bottom() <= screen.bottom() - footer);
            assert!(flyout.height() > 0.);
            if size.y == 1000. {
                assert_eq!(flyout.top(), trigger.top());
            }
        }
    }
}
