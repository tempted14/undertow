//! Native settings, deliberately sharing the player font, renderer and artwork cache.
use super::*;

const SETTINGS_INK: Color32 = Color32::from_rgb(245, 245, 245);
const SETTINGS_MUTED: Color32 = Color32::from_rgb(165, 165, 174);

fn section(ui: &mut egui::Ui, title: &str) {
    let compact = ui.ctx().content_rect().height() < 700.;
    ui.add_space(if compact { 18. } else { 65. });
    ui.horizontal(|ui| {
        ui.add_space(20.);
        label(ui, title, 21., true, SETTINGS_INK);
    });
    ui.add_space(if compact { 12. } else { 22. });
}

fn group(ui: &mut egui::Ui, body: impl FnOnce(&mut egui::Ui)) {
    Frame::new()
        .fill(Color32::from_rgb(16, 16, 18))
        .corner_radius(21)
        .stroke(Stroke::new(1_f32, Color32::from_gray(22)))
        .inner_margin(Margin::symmetric(22, 0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.;
            body(ui);
        });
}

fn divider(ui: &mut egui::Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.), Sense::hover());
    ui.painter().hline(
        r.x_range(),
        r.top(),
        Stroke::new(1_f32, Color32::from_gray(25)),
    );
}

fn row(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: &str,
    right: f32,
    min_height: f32,
) -> (Rect, Rect) {
    let right = right.min(ui.available_width() * 0.52);
    let text_width = (ui.available_width() - right - 28.).max(120.);
    let title_galley = ui
        .painter()
        .layout(title.into(), font(18., true), SETTINGS_INK, text_width);
    let sub = ui.painter().layout(
        subtitle.into(),
        font(16., false),
        SETTINGS_MUTED,
        text_width,
    );
    let gap = if subtitle.is_empty() { 0. } else { 10. };
    let text_height = title_galley.size().y
        + gap
        + if subtitle.is_empty() {
            0.
        } else {
            sub.size().y
        };
    let height = min_height.max(text_height + 36.);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let text_top = r.center().y - text_height * 0.5;
    let subtitle_top = text_top + title_galley.size().y + gap;
    ui.painter()
        .galley(pos2(r.left(), text_top), title_galley, SETTINGS_INK);
    if !subtitle.is_empty() {
        ui.painter()
            .galley(pos2(r.left(), subtitle_top), sub, SETTINGS_MUTED);
    }
    (
        r,
        Rect::from_center_size(
            pos2(r.right() - right * 0.5, r.center().y),
            vec2(right, 44.),
        ),
    )
}

fn switch(ui: &mut egui::Ui, title: &str, subtitle: &str, value: &mut bool) {
    let (r, right) = row(ui, title, subtitle, 56., 96.);
    let response = ui
        .interact(r, ui.id().with(title), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *value, title)
    });
    if response.clicked() {
        *value = !*value;
    }
    let progress = layout::ease(ui.ctx().animate_bool_with_time(
        response.id.with("switch"),
        *value,
        0.16,
    ));
    let rail = Rect::from_center_size(right.center(), vec2(55., 34.));
    let fill = Color32::from_rgb(48, 48, 54).lerp_to_gamma(Color32::from_gray(229), progress);
    ui.painter().rect_filled(rail, 18, fill);
    let center = pos2(rail.left() + 17. + 21. * progress, rail.center().y);
    ui.painter().circle_filled(
        center,
        if response.is_pointer_button_down_on() {
            12.5
        } else {
            12.
        },
        Color32::BLACK,
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
}

fn slider(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
) {
    let (_, right) = row(
        ui,
        title,
        subtitle,
        230_f32.min(ui.available_width() * 0.42),
        96.,
    );
    at(ui, right, title, |ui| {
        ui.add_sized(right.size(), egui::Slider::new(value, range));
    });
}

impl App {
    pub(in crate::ui) fn profile_button(&mut self, ui: &mut egui::Ui, rect: Rect) {
        self.paint_avatar(ui, rect);
        let response = ui
            .interact(rect, ui.id().with("profile-menu"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let hover = layout::ease(ui.ctx().animate_bool_with_time(
            response.id.with("hover"),
            response.hovered(),
            0.14,
        ));
        if hover > 0. {
            ui.painter().circle_stroke(
                rect.center(),
                rect.width() * 0.5 + 2.,
                Stroke::new(1_f32, Color32::from_white_alpha((hover * 100.) as u8)),
            );
        }
        egui::Popup::from_toggle_button_response(&response)
            .align(egui::emath::RectAlign::BOTTOM_END)
            .gap(12.)
            .frame(
                Frame::new()
                    .fill(design::POPOVER)
                    .corner_radius(17)
                    .inner_margin(7),
            )
            .width(300.)
            .show(|ui| {
                ui.set_width(300.);
                ui.spacing_mut().item_spacing.y = 0.;
                for (name, page) in [
                    ("Profile", Some(Page::Profile)),
                    ("Settings", Some(Page::Settings)),
                    ("Log Out", None),
                ] {
                    let (r, b) = ui.allocate_exact_size(vec2(300., 55.), Sense::click());
                    if b.hovered() {
                        ui.painter()
                            .rect_filled(r, 8, Color32::from_white_alpha(15));
                    }
                    ui.painter().text(
                        r.left_center() + vec2(14., 0.),
                        Align2::LEFT_CENTER,
                        name,
                        font(16., true),
                        if page.is_none() {
                            Color32::from_rgb(255, 75, 78)
                        } else {
                            TEXT
                        },
                    );
                    if b.clicked() {
                        if let Some(page) = page {
                            self.page = page;
                        } else if self.connected {
                            self.send(Command::SignOut);
                        } else {
                            self.toast("You are already signed out", false);
                        }
                        ui.close();
                    }
                }
            });
    }

    fn paint_avatar(&self, ui: &mut egui::Ui, rect: Rect) {
        ui.painter().circle_filled(
            rect.center(),
            rect.width() * 0.5,
            Color32::from_rgb(36, 37, 42),
        );
        if let Some(texture) = &self.avatar {
            // Circle geometry masks the existing texture without retaining a second bitmap.
            let mut mesh = egui::Mesh::with_texture(texture.id());
            mesh.vertices.push(egui::epaint::Vertex {
                pos: rect.center(),
                uv: pos2(0.5, 0.5),
                color: Color32::WHITE,
            });
            for i in 0..=48 {
                let a = i as f32 * std::f32::consts::TAU / 48.;
                let d = vec2(a.cos(), a.sin());
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: rect.center() + d * (rect.width() * 0.5),
                    uv: pos2(0.5 + d.x * 0.5, 0.5 + d.y * 0.5),
                    color: Color32::WHITE,
                });
                if i > 0 {
                    mesh.add_triangle(0, i, i + 1);
                }
            }
            ui.painter().add(egui::Shape::mesh(mesh));
        } else {
            paint_icon(
                ui.painter(),
                rect.shrink(rect.width() * 0.22),
                "user",
                SETTINGS_MUTED,
            );
        }
    }

    pub(in crate::ui) fn reference_profile(&mut self, ui: &mut egui::Ui) {
        let height = (ui.available_height() - reserved_player(ui.ctx()) - 12.).max(80.);
        egui::ScrollArea::vertical()
            .id_salt("profile-scroll")
            .max_height(height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width().min(1072.));
                self.profile_body(ui);
            });
    }
    fn profile_body(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width().min(1072.);
        ui.add_space(26.);
        label(ui, "Profile", 38., true, SETTINGS_INK);
        ui.add_space(32.);
        group(ui, |ui| {
            let (r, _) = ui.allocate_exact_size(vec2(width - 44., 190.), Sense::hover());
            self.paint_avatar(
                ui,
                Rect::from_center_size(r.left_center() + vec2(78., 0.), vec2(136., 136.)),
            );
            at(
                ui,
                Rect::from_min_max(r.min + vec2(180., 48.), r.max - vec2(12., 0.)),
                "profile-edit",
                |ui| {
                    label(
                        ui,
                        if self.settings.profile_name.is_empty() {
                            "Your Undertow profile"
                        } else {
                            &self.settings.profile_name
                        },
                        28.,
                        true,
                        TEXT,
                    );
                    ui.add_space(10.);
                    ui.horizontal(|ui| {
                        if pill(ui, "Change picture", false).clicked() {
                            self.send(Command::PickAvatar);
                        }
                        if self.avatar.is_some() && small_pill(ui, "Remove").clicked() {
                            self.avatar = None;
                            self.settings.profile_picture = false;
                            self.schedule_settings_save();
                        }
                    });
                },
            );
            divider(ui);
            ui.add_space(22.);
            let before = self.settings.profile_name.clone();
            field(ui, "Display name", &mut self.settings.profile_name);
            if before != self.settings.profile_name {
                self.settings_changed_at = Instant::now();
                self.settings_dirty = true;
            }
            ui.add_space(14.);
            ui.label(
                RichText::new(
                    "Your picture and display name are saved on this device in Undertow.",
                )
                .font(font(15., false))
                .color(SETTINGS_MUTED),
            );
            ui.add_space(22.);
        });
        section(ui, "TIDAL account");
        group(ui, |ui| {
            let (_, right) = row(
                ui,
                if self.connected {
                    "Connected to TIDAL"
                } else {
                    "Connect your music"
                },
                &self.account,
                170.,
                108.,
            );
            at(ui, right, "profile-account", |ui| {
                if pill(
                    ui,
                    if self.connected {
                        "Account settings"
                    } else {
                        "Sign in"
                    },
                    !self.connected,
                )
                .clicked()
                {
                    self.settings_tab = 1;
                    self.page = Page::Settings;
                }
            });
        });
    }

    pub(in crate::ui) fn reference_settings(&mut self, ui: &mut egui::Ui) {
        let before = self.settings.clone();
        let compact = ui.ctx().content_rect().height() < 700.;
        let tab_height = if compact { 42. } else { 54. };
        let margin = ((ui.available_width() - 1072.) * 0.5).max(24.);
        let panel = [
            "settings-general",
            "settings-account",
            "settings-plugins",
            "settings-about",
        ][self.settings_tab.min(3)];
        super::catalog::catalog_canvas(ui, panel, margin, 0., |ui| {
            ui.spacing_mut().item_spacing = vec2(12., 0.);
            ui.add_space(if compact { 8. } else { 25. });
            label(
                ui,
                "Settings",
                if compact { 28. } else { design::TITLE },
                true,
                TEXT,
            );
            ui.add_space(if compact { 12. } else { 26. });
            let (tabs, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), tab_height), Sense::hover());
            ui.painter().hline(
                tabs.x_range(),
                tabs.bottom(),
                Stroke::new(1_f32, design::SEPARATOR),
            );
            let mut x = tabs.left();
            for (index, title) in [(0, "General"), (1, "Account"), (3, "About"), (2, "Plugins")] {
                let r = Rect::from_min_size(
                    pos2(x, tabs.top()),
                    vec2(if index == 0 { 96. } else { 100. }, tab_height),
                );
                x = r.right();
                let b = ui
                    .interact(r, Id::new(("settings-tab", index)), Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                let selected = self.settings_tab == index;
                ui.painter().text(
                    r.left_center(),
                    Align2::LEFT_CENTER,
                    title,
                    font(18., true),
                    if selected || b.hovered() { TEXT } else { GREY },
                );
                if selected {
                    ui.painter().hline(
                        r.left()..=r.left() + 65.,
                        tabs.bottom(),
                        Stroke::new(1_f32, TEXT),
                    );
                }
                if b.clicked() {
                    self.settings_tab = index;
                }
            }
            match self.settings_tab {
                0 => self.settings_general(ui),
                1 => self.settings_account(ui),
                2 => self.settings_plugins(ui),
                _ => self.settings_about(ui),
            }
            ui.add_space(45.);
        });
        if before.font_enabled != self.settings.font_enabled {
            self.apply_settings_font(ui.ctx());
        }
        if before != self.settings {
            self.settings_dirty = true;
            self.settings_changed_at = Instant::now();
            self.last_presence = Instant::now() - Duration::from_secs(6);
        }
    }

    fn settings_general(&mut self, ui: &mut egui::Ui) {
        section(ui, "Audio quality");
        group(ui, |ui| {
            for (i, title, description) in [
                (0, "Low", "Balance audio quality and data consumption"),
                (1, "High", "16-bit, 44.1 kHz"),
                (2, "Max", "Up to 24-bit, 192 kHz"),
            ] {
                if i > 0 {
                    divider(ui);
                }
                let selected = match i {
                    0 => matches!(self.settings.quality.as_str(), "LOW" | "HIGH"),
                    1 => self.settings.quality == "LOSSLESS",
                    _ => self.settings.quality == "HI_RES_LOSSLESS",
                };
                let (r, b) =
                    ui.allocate_exact_size(vec2(ui.available_width(), 97.), Sense::click());
                let c = pos2(r.left() + 11., r.center().y);
                if selected && i == 2 {
                    ui.painter().rect_filled(
                        r,
                        12,
                        Color32::from_rgba_unmultiplied(230, 188, 24, 8),
                    );
                }
                ui.painter().circle_filled(
                    c,
                    16.,
                    if selected && i == 2 {
                        Color32::from_rgb(56, 47, 12)
                    } else {
                        Color32::from_gray(27)
                    },
                );
                if selected {
                    ui.painter()
                        .circle_filled(c, 6.5, if i == 2 { playlist::GOLD } else { TEXT });
                }
                ui.painter().text(
                    pos2(r.left() + 50., r.center().y - 14.),
                    Align2::LEFT_CENTER,
                    title,
                    font(18., true),
                    SETTINGS_INK,
                );
                ui.painter().text(
                    pos2(r.left() + 50., r.center().y + 17.),
                    Align2::LEFT_CENTER,
                    description,
                    font(16., false),
                    SETTINGS_MUTED,
                );
                if i == 0 && r.width() > 640. {
                    let bitrate =
                        Rect::from_center_size(r.right_center() - vec2(54., 0.), vec2(108., 42.));
                    ui.painter().rect_filled(bitrate, 9, design::SURFACE);
                    ui.painter().text(
                        bitrate.center(),
                        Align2::CENTER_CENTER,
                        if self.settings.quality == "LOW" {
                            "96 kbps"
                        } else {
                            "320 kbps"
                        },
                        font(16., true),
                        TEXT,
                    );
                }
                if b.clicked() {
                    self.settings.quality = ["HIGH", "LOSSLESS", "HI_RES_LOSSLESS"][i].into();
                    self.settings.strict_lossless = i > 0;
                }
            }
        });
        section(ui, "Playback");
        group(ui, |ui| {
            switch(
                ui,
                "Lossless only",
                "Refuse a lossy fallback when the account cannot stream lossless audio.",
                &mut self.settings.strict_lossless,
            );
            divider(ui);
            switch(
                ui,
                "Stream recovery",
                "Recover a stalled connection with bounded retries, while keeping audio buffers small.",
                &mut self.settings.no_buffer,
            );
            divider(ui);
            let (_, right) = row(
                ui,
                "Sound output",
                "Windows shared output · follows your default audio device on the next track.",
                140.,
                98.,
            );
            at(ui, right, "open-sound", |ui| {
                if pill(ui, "Settings", false).clicked() {
                    let _ = std::process::Command::new("explorer.exe")
                        .arg("ms-settings:sound")
                        .spawn();
                }
            });
        });
        section(ui, "Display");
        group(ui, |ui| {
            let (_, right) = row(
                ui,
                "Player background",
                "Choose your preferred look for the player background.",
                230.,
                154.,
            );
            at(
                ui,
                right.expand2(vec2(0., 25.)),
                "background-options",
                |ui| {
                    ui.horizontal(|ui| {
                        for (blur, title) in [(false, "Color"), (true, "Blur")] {
                            ui.vertical(|ui| {
                                let (r, b) = ui.allocate_exact_size(vec2(98., 66.), Sense::click());
                                ui.painter().rect_filled(
                                    r.shrink(3.),
                                    13,
                                    if blur {
                                        Color32::from_rgb(108, 67, 84)
                                    } else {
                                        Color32::from_rgb(159, 59, 50)
                                    },
                                );
                                if self.settings.frosted_glass == blur {
                                    ui.painter().rect_stroke(
                                        r,
                                        16,
                                        Stroke::new(2_f32, TEXT),
                                        egui::StrokeKind::Inside,
                                    );
                                }
                                label(
                                    ui,
                                    title,
                                    17.,
                                    true,
                                    if self.settings.frosted_glass == blur {
                                        TEXT
                                    } else {
                                        SETTINGS_MUTED
                                    },
                                );
                                if b.clicked() {
                                    self.settings.cover_theme = true;
                                    self.settings.frosted_glass = blur;
                                }
                            });
                        }
                    });
                },
            );
            divider(ui);
            switch(
                ui,
                "Audio metadata",
                "Show bitrate, sample rate, depth and quality badges where available.",
                &mut self.settings.show_quality,
            );
            divider(ui);
            switch(
                ui,
                "Show artwork",
                "Display album and playlist cover art throughout Undertow.",
                &mut self.settings.show_covers,
            );
        });
        ui.add_space(20.);
        group(ui, |ui| {
            switch(
                ui,
                "Show sidebar",
                "Your music, collection and playlists. Ctrl+B expands or collapses it.",
                &mut self.settings.show_sidebar,
            );
            divider(ui);
            switch(
                ui,
                "Playlist background",
                "Use artwork colors behind playlists.",
                &mut self.settings.playlist_background,
            );
        });
        section(ui, "Connect");
        group(ui, |ui| {
            switch(
                ui,
                "Discord Rich Presence",
                "Share your current track, cover and playback time. Configure it in Plugins.",
                &mut self.settings.discord,
            );
        });
        section(ui, "Preferences");
        group(ui, |ui| {
            switch(
                ui,
                "Animations",
                "Fluid page transitions, lyric following and scrolling. Disable for reduced motion.",
                &mut self.settings.animated_lyrics,
            );
            divider(ui);
            switch(
                ui,
                "Synchronize to display",
                "Use your monitor's refresh rate instead of a fixed animation timer.",
                &mut self.settings.display_sync,
            );
            divider(ui);
            slider(
                ui,
                "Animation speed",
                "Adjust how quickly transitions and lyric motion respond.",
                &mut self.settings.motion_speed,
                0.5..=2.,
            );
        });
    }

    fn settings_account(&mut self, ui: &mut egui::Ui) {
        section(ui, "Account");
        group(ui, |ui| {
            ui.add_space(24.);
            label(ui, &self.account, 19., true, TEXT);
            ui.add_space(18.);
            ui.horizontal(|ui| {
                if pill(ui, "View profile", false).clicked() {
                    self.page = Page::Profile;
                }
                if self.connected && pill(ui, "Sync library", false).clicked() {
                    self.send(Command::Sync);
                }
            });
            ui.add_space(24.);
        });
        section(ui, "Sign in with TIDAL");
        group(ui, |ui| {
            ui.add_space(24.);
            label(ui, "Choose how to connect", 20., true, TEXT);
            ui.add_space(12.);
            ui.label(RichText::new("Keep your subscription, playlists and saved music. Undertow remembers your account and refreshes your session automatically.").font(font(16., false)).color(SETTINGS_MUTED));
            ui.add_space(22.);
            ui.horizontal_wrapped(|ui| {
                if pill(ui, "Lossless sign-in", true).clicked() && !self.signing_in {
                    self.settings.compatibility = true;
                    self.settings.paste_login = true;
                    self.settings.device_login = false;
                    self.send(Command::Login(self.settings.clone()));
                }
                if pill(ui, "Quick code sign-in", false).clicked() && !self.signing_in {
                    self.settings.compatibility = true;
                    self.settings.paste_login = false;
                    self.settings.device_login = true;
                    self.send(Command::Login(self.settings.clone()));
                }
                if self.connected && pill(ui, "Log Out", false).clicked() {
                    self.send(Command::SignOut);
                }
            });
            ui.add_space(16.);
            ui.label(RichText::new("Lossless: approve on TIDAL, then paste the final page address here once. Quick code: automatic approval return, but TIDAL may limit this client to standard quality. Your password is entered only on TIDAL's website.").font(font(15., false)).color(SETTINGS_MUTED));
            ui.add_space(22.);
            egui::CollapsingHeader::new("Advanced connection options").show(ui, |ui| {
                let mut registered = !self.settings.compatibility;
                ui.checkbox(&mut registered, "Use my registered developer application");
                self.settings.compatibility = !registered;
                if registered {
                    field(ui, "Public client ID", &mut self.settings.client_id);
                    field(ui, "Registered redirect", &mut self.settings.redirect_uri);
                    field(ui, "Scopes", &mut self.settings.scopes);
                    field(ui, "Token broker URL", &mut self.settings.token_broker);
                    if pill(ui, "Connect registered app", false).clicked() {
                        self.send(Command::Login(self.settings.clone()));
                    }
                }
                field(ui, "Country", &mut self.settings.country);
            });
            ui.add_space(24.);
        });
    }

    fn settings_plugins(&mut self, ui: &mut egui::Ui) {
        section(ui, "Lyrics");
        group(ui, |ui| {
            switch(
                ui,
                "One Click Lyrics",
                "Show the one-click lyrics control in the player. Ctrl+L remains available.",
                &mut self.settings.one_click_lyrics,
            );
            divider(ui);
            switch(
                ui,
                "Lyrics Everywhere",
                "Show the current synchronized line above the player.",
                &mut self.settings.everywhere,
            );
            divider(ui);
            switch(
                ui,
                "Floating lyrics",
                "A resizable, always-on-top window in the same lightweight process.",
                &mut self.settings.floating_lyrics,
            );
            divider(ui);
            switch(
                ui,
                "Colorama Lyrics",
                "Soft glow around the bright active line. Cover tint is optional.",
                &mut self.settings.lyric_glow,
            );
            divider(ui);
            switch(
                ui,
                "Cover-colored glow",
                "Tint the halo using the current cover. The active line uses your text color.",
                &mut self.settings.lyric_cover_color,
            );
            divider(ui);
            switch(
                ui,
                "Copy Lyrics",
                "Copy the full song or a single line to the clipboard.",
                &mut self.settings.copy_lyrics,
            );
            divider(ui);
            switch(
                ui,
                "Motion blur",
                "Soften inactive lyrics; the currently sung line stays sharp.",
                &mut self.settings.lyric_motion_blur,
            );
            divider(ui);
            slider(
                ui,
                "Blur strength",
                "Adjust the inactive lyric blur without blurring the whole window.",
                &mut self.settings.lyric_blur_strength,
                0.0..=1.0,
            );
            divider(ui);
            slider(
                ui,
                "Lyric size",
                "Larger type preserves accurate wrapping and following.",
                &mut self.settings.lyric_size,
                24.0..=56.0,
            );
            divider(ui);
            let (_, right) = row(
                ui,
                "Lyric color",
                "Near-white uses a subtle hint of the cover's color.",
                70.,
                90.,
            );
            at(ui, right, "lyric-color", |ui| {
                ui.color_edit_button_srgb(&mut self.settings.lyric_color);
            });
            divider(ui);
            let (_, right) = row(
                ui,
                "Timing offset",
                "Positive values bring the lyrics forward. Applies to every lyric view.",
                220.,
                96.,
            );
            at(ui, right, "lyric-timing", |ui| {
                ui.add(
                    egui::Slider::new(&mut self.settings.lyric_offset_ms, -3000..=3000)
                        .suffix(" ms"),
                );
            });
            ui.add_space(20.);
            if self.settings.copy_lyrics && pill(ui, "Copy lyrics", false).clicked() {
                ui.ctx().copy_text(self.lyrics.plain());
            }
            ui.add_space(20.);
        });
        section(ui, "Custom Fonts");
        group(ui, |ui| {
            switch(
                ui,
                "Custom Fonts",
                "Use a local TTF/OTF font. Disable to restore Undertow's system type.",
                &mut self.settings.font_enabled,
            );
            divider(ui);
            ui.add_space(22.);
            field(
                ui,
                "Font file (.ttf / .otf, up to 8 MiB)",
                &mut self.settings.font_path,
            );
            ui.add_space(14.);
            ui.horizontal(|ui| {
                if pill(ui, "Choose font", false).clicked() {
                    self.send(Command::PickFont);
                }
                if pill(ui, "Apply font", false).clicked() {
                    self.apply_settings_font(ui.ctx());
                }
                if pill(ui, "Reset", false).clicked() {
                    self.settings.font_path.clear();
                    self.apply_settings_font(ui.ctx());
                }
            });
            ui.add_space(24.);
        });
        section(ui, "Library & playback");
        group(ui, |ui| {
            switch(
                ui,
                "Total Play Time",
                "Show the total duration in playlist and album headers.",
                &mut self.settings.total_play_time,
            );
            divider(ui);
            switch(
                ui,
                "RealMAX",
                "Prefer a higher-quality version matched by the same ISRC.",
                &mut self.settings.real_max,
            );
            divider(ui);
            switch(
                ui,
                "NoBuffer",
                "Recover stalled streams with bounded retries; audio buffers remain bounded.",
                &mut self.settings.no_buffer,
            );
            divider(ui);
            switch(
                ui,
                "Song Downloader",
                "Show download actions. Downloads require TIDAL offline entitlement.",
                &mut self.settings.song_downloader,
            );
            divider(ui);
            switch(
                ui,
                "Cover Fetcher",
                "Show Download Cover Art and Copy Cover Art Link actions.",
                &mut self.settings.cover_fetcher,
            );
            divider(ui);
            switch(
                ui,
                "Tidal Tags",
                "Display verified quality, bit depth, sample rate and bitrate when available.",
                &mut self.settings.show_quality,
            );
            divider(ui);
            ui.add_space(20.);
            field(
                ui,
                "Download folder (empty uses Undertow's folder)",
                &mut self.settings.downloads,
            );
            ui.add_space(24.);
        });
        section(ui, "Theme & element visibility");
        group(ui, |ui| {
            switch(
                ui,
                "CoverTheme",
                "Tint the player and lyric atmosphere using the current cover.",
                &mut self.settings.cover_theme,
            );
            divider(ui);
            let mut clean = !self.settings.playlist_background;
            switch(
                ui,
                "NoPlaylistBackgroundImage",
                "Keep playlist pages clear of artwork backgrounds.",
                &mut clean,
            );
            self.settings.playlist_background = !clean;
            divider(ui);
            switch(
                ui,
                "Element Hider · sidebar",
                "Choose whether the navigation sidebar is visible.",
                &mut self.settings.show_sidebar,
            );
            divider(ui);
            switch(
                ui,
                "Element Hider · artwork",
                "Choose whether covers are visible.",
                &mut self.settings.show_covers,
            );
            divider(ui);
            switch(
                ui,
                "Frosted artwork",
                "Use a small preblurred cover texture for the atmosphere.",
                &mut self.settings.frosted_glass,
            );
            divider(ui);
            slider(
                ui,
                "Atmosphere",
                "Adjust background tint and active lyric glow.",
                &mut self.settings.effect_strength,
                0.0..=1.0,
            );
        });
        section(ui, "TidalLyricsTranslate");
        group(ui, |ui| {
            ui.add_space(24.);
            field(
                ui,
                "Translation language",
                &mut self.settings.translation_language,
            );
            switch(
                ui,
                "Romanization",
                "Romanize instead of translating when you request a translation.",
                &mut self.settings.romanize,
            );
            ui.horizontal_wrapped(|ui| {
                if pill(
                    ui,
                    if self.translated {
                        "Show original"
                    } else {
                        "Translate lyrics"
                    },
                    false,
                )
                .clicked()
                {
                    if self.translated {
                        self.translated = false;
                    } else {
                        self.translated = true;
                        let request = self.lyric_request.begin();
                        self.send(Command::Translate(
                            self.generation,
                            request,
                            self.lyrics.as_ref().clone(),
                            self.settings.translation_language.clone(),
                            self.settings.romanize,
                        ));
                    }
                }
            });
            ui.add_space(14.);
            ui.label(RichText::new("Translation sends the current lyric text to Google's unofficial translation endpoint only when requested.").font(font(14., false)).color(SETTINGS_MUTED));
            ui.add_space(24.);
        });
        section(ui, "Discord Rich Presence");
        group(ui, |ui| {
            switch(
                ui,
                "Discord Rich Presence",
                "Show the current track, cover and playback progress in Discord.",
                &mut self.settings.discord,
            );
            divider(ui);
            ui.add_space(20.);
            field(
                ui,
                "Discord application ID (public)",
                &mut self.settings.discord_client_id,
            );
            label(ui, &self.presence_status, 15., false, SETTINGS_MUTED);
            ui.add_space(24.);
        });
    }

    fn apply_settings_font(&mut self, ctx: &egui::Context) {
        let path = self.settings.effective_font().to_owned();
        if load_font(ctx, &path) {
            self.font_loaded = path;
            self.lyric_layout_key = None;
        } else {
            self.toast("Could not load this font (maximum 8 MiB)", true);
        }
    }

    fn settings_about(&mut self, ui: &mut egui::Ui) {
        section(ui, "UNDERTOW");
        group(ui, |ui| {
            ui.add_space(28.);
            label(
                ui,
                concat!("Undertow ", env!("CARGO_PKG_VERSION")),
                25.,
                true,
                TEXT,
            );
            ui.add_space(12.);
            label(
                ui,
                "An independent, native TIDAL music player.",
                17.,
                false,
                SETTINGS_MUTED,
            );
            ui.add_space(24.);
        });
        section(ui, "Performance");
        group(ui, |ui| {
            ui.add_space(24.);
            label(
                ui,
                format!(
                    "RAM {:.1} MB · private memory {:.1} MB",
                    self.metrics.current.working_set_bytes as f64 / 1_000_000.,
                    self.metrics.current.private_bytes as f64 / 1_000_000.
                ),
                17.,
                true,
                TEXT,
            );
            ui.add_space(14.);
            ui.add(
                egui::Slider::new(&mut self.settings.artwork_cache_mb, 8..=128)
                    .text("Artwork disk cache (MiB)"),
            );
            if !self.settings.display_sync {
                ui.add(
                    egui::Slider::new(&mut self.settings.fps, 30..=240)
                        .text("Animation frame limit"),
                );
            }
            ui.add_space(16.);
            if pill(ui, "Save diagnostics report", false).clicked() {
                self.save_report();
                self.toast("Diagnostics report saved", false);
            }
            ui.add_space(24.);
        });
        section(ui, "Local music");
        group(ui, |ui| {
            ui.add_space(24.);
            field(ui, "Audio file path", &mut self.local_path);
            if pill(ui, "Open audio", false).clicked() {
                self.open_local(self.local_path.trim_matches('"').to_owned());
            }
            ui.add_space(24.);
        });
    }
}

#[cfg(test)]
mod row_tests {
    use super::*;
    #[test]
    fn long_settings_labels_wrap_without_overlapping_controls() {
        for width in [270., 340., 520., 1072.] {
            let ctx = egui::Context::default();
            assert!(crate::ui::load_font(&ctx, ""));
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(width + 40., 700.))),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.set_width(width);
                        let (r, control) = row(
                            ui,
                            "A deliberately long playback and appearance setting label",
                            "A complete description remains readable when the window is narrow.",
                            170.,
                            96.,
                        );
                        ui.ctx()
                            .data_mut(|d| d.insert_temp(Id::new("row-bounds"), (r, control)));
                    });
                },
            );
            let (r, control) = ctx
                .data(|d| d.get_temp::<(Rect, Rect)>(Id::new("row-bounds")))
                .unwrap();
            for shape in output.shapes {
                if let egui::Shape::Text(t) = shape.shape {
                    let text = t.galley.rect.translate(t.pos.to_vec2());
                    assert!(!text.intersects(control));
                    assert!(text.bottom() <= r.bottom());
                }
            }
        }
    }
}
