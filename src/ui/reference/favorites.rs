use super::*;

impl App {
    pub(in crate::ui) fn reference_favorites(&mut self, ui: &mut egui::Ui) {
        let tracks = self.tracks.get("favorites").cloned().unwrap_or_default();
        let margin = (ui.available_width() * 0.045).clamp(28., 64.);
        super::catalog::catalog_canvas(ui, "favorites-body", margin, 350., |ui| {
            ui.add_space(22.);
            label(ui, "Tracks", design::TITLE, true, TEXT);
            ui.add_space(22.);
            let (actions, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), 64.), Sense::hover());
            self.large_play_actions(ui, actions, &tracks);
            ui.add_space(22.);
            if self.playlist_filter_id != "favorites" {
                self.playlist_filter_id = "favorites".into();
                self.playlist_filter.clear();
                self.playlist_sort = Some((4, true));
                self.playlist_index.invalidate();
                self.playlist_scroll_reset = true;
            }
            if !self.playlist_index.matches(&tracks) {
                self.rebuild_playlist_filter(&tracks);
            }
            let (filter, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), 52.), Sense::hover());
            let changed = forms::track_filter(
                ui,
                filter,
                &mut self.playlist_filter,
                "Filter tracks",
                "favorites-filter",
            );
            if changed {
                self.playlist_scroll_reset = true;
                self.rebuild_playlist_filter(&tracks);
            }
            if !self.playlist_filter.trim().is_empty() {
                forms::filter_summary(ui, filter, self.playlist_filtered.len(), tracks.len());
            }
            ui.add_space(22.);
            if tracks.is_empty() {
                label(
                    ui,
                    if self.busy {
                        "Loading your tracks…"
                    } else {
                        "Tracks you add to My Collection appear here."
                    },
                    18.,
                    false,
                    GREY,
                );
            } else {
                self.catalog_track_table(ui, tracks, None, false);
            }
        });
    }
}
