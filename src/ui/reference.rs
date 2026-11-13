//! Native reference layout; no web view, DOM, or continuously blurred full-size layers.
use super::*;
mod artist;
mod catalog;
mod controls;
pub(super) mod discovery;
mod favorites;
pub(super) mod forms;
mod home_motion;
pub(in crate::ui) mod layout;
mod lyric_navigation;
mod queue;
use layout::{PlayerLayout, columns, reserved_player};
pub(super) mod interactions;
mod navigation;
mod now_playing;
mod playlist;
mod settings;

const GREY: Color32 = Color32::from_rgb(150, 150, 144);

fn font(size: f32, bold: bool) -> FontId {
    FontId::new(
        size,
        egui::FontFamily::Name(if bold { "Bold" } else { "Medium" }.into()),
    )
}
fn label(
    ui: &mut egui::Ui,
    text: impl Into<String>,
    size: f32,
    bold: bool,
    color: Color32,
) -> egui::Response {
    ui.add(
        egui::Label::new(
            RichText::new(text.into())
                .font(font(size, bold))
                .color(color),
        )
        .truncate(),
    )
}

fn text_link(
    ui: &mut egui::Ui,
    text: impl Into<String>,
    size: f32,
    bold: bool,
    color: Color32,
    enabled: bool,
) -> egui::Response {
    let response = ui.add(
        egui::Label::new(
            RichText::new(text.into())
                .font(font(size, bold))
                .color(color),
        )
        .truncate()
        .selectable(false)
        .sense(if enabled {
            Sense::click()
        } else {
            Sense::hover()
        }),
    );
    if enabled {
        if response.hovered() {
            ui.painter().line_segment(
                [response.rect.left_bottom(), response.rect.right_bottom()],
                Stroke::new(1_f32, color),
            );
        }
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}
fn at<R>(
    ui: &mut egui::Ui,
    rect: Rect,
    id: impl std::hash::Hash,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    // Positioned contents must not consume extra cells in a surrounding Grid.
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(id)
            .max_rect(rect)
            .layout(Layout::top_down(Align::Min)),
    );
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    body(&mut child)
}
fn paint_icon(p: &egui::Painter, rect: Rect, name: &str, color: Color32) {
    let c = rect.center();
    let s = rect.width().min(rect.height()) * 0.34;
    let point = |x: f32, y: f32| c + vec2(x * s, y * s);
    let stroke = Stroke::new((rect.width() / 15.0).clamp(1.4, 2.1), color);
    let line = |a: (f32, f32), b: (f32, f32)| {
        p.line_segment([point(a.0, a.1), point(b.0, b.1)], stroke);
    };
    let path = |pts: &[(f32, f32)]| {
        p.add(egui::Shape::line(
            pts.iter().map(|&(x, y)| point(x, y)).collect(),
            stroke,
        ));
    };
    match name {
        "play" => {
            p.add(egui::Shape::convex_polygon(
                vec![point(-0.55, -0.9), point(0.9, 0.), point(-0.55, 0.9)],
                color,
                Stroke::NONE,
            ));
        }
        "pause" => {
            for x in [-0.55, 0.3] {
                p.rect_filled(
                    Rect::from_min_max(point(x, -0.9), point(x + 0.25, 0.9)),
                    0,
                    color,
                );
            }
        }
        "next" | "previous" => {
            let k = if name == "next" { 1.0 } else { -1.0 };
            p.add(egui::Shape::convex_polygon(
                vec![
                    point(-0.65 * k, -0.8),
                    point(0.45 * k, 0.),
                    point(-0.65 * k, 0.8),
                ],
                color,
                Stroke::NONE,
            ));
            line((0.7 * k, -0.9), (0.7 * k, 0.9));
        }
        "back" => path(&[(0.35, -0.8), (-0.4, 0.), (0.35, 0.8)]),
        "forward" => path(&[(-0.35, -0.8), (0.4, 0.), (-0.35, 0.8)]),
        "down" => path(&[(-0.8, -0.35), (0., 0.4), (0.8, -0.35)]),
        "up" => path(&[(-0.8, 0.35), (0., -0.4), (0.8, 0.35)]),
        "search" => {
            p.circle_stroke(point(-0.2, -0.2), s * 0.65, stroke);
            line((0.3, 0.3), (0.95, 0.95));
        }
        "music" => {
            path(&[(-0.5, 0.5), (-0.5, -0.75), (0.8, -1.), (0.8, 0.25)]);
            p.circle_stroke(point(-0.75, 0.65), s * 0.28, stroke);
            p.circle_stroke(point(0.55, 0.4), s * 0.28, stroke);
        }
        "globe" => {
            p.circle_stroke(c, s, stroke);
            line((-1., 0.), (1., 0.));
            let pts = (0..=24)
                .map(|i| {
                    let a = i as f32 * std::f32::consts::TAU / 24.;
                    point(a.cos() * 0.4, a.sin())
                })
                .collect();
            p.add(egui::Shape::line(pts, stroke));
        }
        "upload" => {
            line((0., 1.), (0., -1.));
            path(&[(-0.7, -0.3), (0., -1.), (0.7, -0.3)]);
        }
        "collection" => {
            for x in [-0.7, 0., 0.7] {
                line((x - 0.15, 0.9), (x + 0.15, -0.9));
            }
        }
        "queue" => {
            for y in [-0.65, 0., 0.65] {
                line((-0.9, y), (0.9, y));
            }
        }
        "music-plus" => {
            path(&[(-0.45, 0.55), (-0.45, -0.85), (0.2, -0.95)]);
            p.circle_stroke(point(-0.7, 0.7), s * 0.27, stroke);
            line((0.35, -0.15), (1., -0.15));
            line((0.675, -0.475), (0.675, 0.175));
        }
        "lyrics" => {
            p.rect_stroke(
                Rect::from_min_max(point(-0.9, -0.85), point(0.9, 0.85)),
                3,
                stroke,
                egui::StrokeKind::Inside,
            );
            line((-0.5, -0.35), (0.5, -0.35));
            line((-0.5, 0.15), (0.25, 0.15));
        }
        "float" => {
            p.rect_stroke(
                Rect::from_min_max(point(-0.9, -0.9), point(0.9, 0.9)),
                2,
                stroke,
                egui::StrokeKind::Inside,
            );
            p.rect_filled(
                Rect::from_min_max(point(-0.1, 0.0), point(0.6, 0.6)),
                1,
                color,
            );
        }
        "volume" => {
            path(&[
                (-0.9, -0.3),
                (-0.5, -0.3),
                (0.1, -0.9),
                (0.1, 0.9),
                (-0.5, 0.3),
                (-0.9, 0.3),
                (-0.9, -0.3),
            ]);
            path(&[(0.45, -0.6), (0.7, -0.25), (0.7, 0.25), (0.45, 0.6)]);
        }
        "eye" | "eye-off" => {
            // Two shallow curves keep the outline legible at the reference's small size.
            path(&[
                (-1., 0.),
                (-0.6, -0.55),
                (0., -0.75),
                (0.6, -0.55),
                (1., 0.),
                (0.6, 0.55),
                (0., 0.75),
                (-0.6, 0.55),
                (-1., 0.),
            ]);
            p.circle_filled(c, s * 0.3, color);
            if name == "eye-off" {
                line((-1.05, -1.), (1.05, 1.));
            }
        }
        "resync" => {
            for (start, end) in [(-2.5_f32, 0.3_f32), (0.65, 3.45)] {
                p.add(egui::Shape::line(
                    (0..=14)
                        .map(|i| {
                            let angle = start + (end - start) * i as f32 / 14.;
                            point(angle.cos() * 0.95, angle.sin() * 0.95)
                        })
                        .collect(),
                    stroke,
                ));
            }
            path(&[(-1., -0.8), (-0.78, -0.36), (-0.34, -0.6)]);
            path(&[(1., 0.8), (0.78, 0.36), (0.34, 0.6)]);
            for x in [-0.45, 0., 0.45] {
                p.circle_filled(point(x, 0.), s * 0.12, color);
            }
        }
        "repeat" => {
            path(&[
                (-0.8, 0.2),
                (-0.8, -0.5),
                (-0.4, -0.8),
                (0.8, -0.8),
                (0.45, -1.1),
            ]);
            path(&[
                (0.8, -0.2),
                (0.8, 0.5),
                (0.4, 0.8),
                (-0.8, 0.8),
                (-0.45, 1.1),
            ]);
        }
        "shuffle" => {
            path(&[
                (-1., -0.7),
                (-0.6, -0.7),
                (0.6, 0.7),
                (1., 0.7),
                (0.7, 0.35),
            ]);
            path(&[(-1., 0.7), (-0.6, 0.7), (0.6, -0.7), (1., -0.7), (0.7, -1.)]);
        }
        "heart" => {
            let points = playlist::heart_points(rect);
            let mut mesh = egui::Mesh::default();
            mesh.colored_vertex(c, color);
            for &point in &points {
                mesh.colored_vertex(point, color);
            }
            for i in 0..points.len() as u32 {
                mesh.add_triangle(0, 1 + i, 1 + (i + 1) % points.len() as u32);
            }
            p.add(egui::Shape::mesh(mesh));
        }
        "dots" => {
            for x in [-0.75, 0., 0.75] {
                p.circle_filled(point(x, 0.), s * 0.14, color);
            }
        }
        "edit" => {
            path(&[
                (-0.85, 0.85),
                (-0.6, 0.1),
                (0.55, -0.95),
                (0.95, -0.55),
                (-0.1, 0.6),
                (-0.85, 0.85),
            ]);
            line((0.25, -0.65), (0.65, -0.25));
        }
        "share" => {
            path(&[(-0.8, -0.1), (-0.8, 0.85), (0.8, 0.85), (0.8, -0.1)]);
            line((0., 0.4), (0., -1.));
            path(&[(-0.45, -0.55), (0., -1.), (0.45, -0.55)]);
        }
        "lock" => {
            path(&[
                (-0.55, -0.05),
                (-0.55, -0.65),
                (-0.3, -0.95),
                (0.3, -0.95),
                (0.6, -0.65),
            ]);
            p.rect_stroke(
                Rect::from_min_max(point(-0.75, -0.05), point(0.75, 0.85)),
                2,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        "sidebar-collapse" | "sidebar-expand" => {
            let k = if name == "sidebar-collapse" { 1. } else { -1. };
            line((-0.9 * k, -0.8), (-0.9 * k, 0.8));
            path(&[(0.55 * k, -0.6), (-0.2 * k, 0.), (0.55 * k, 0.6)]);
        }
        "check" => path(&[(-0.9, 0.), (-0.25, 0.65), (0.95, -0.8)]),
        "plus" => {
            line((-0.8, 0.), (0.8, 0.));
            line((0., -0.8), (0., 0.8));
        }
        "heart-outline" => playlist::outline_heart(p, rect, color),
        "info" => {
            p.circle_stroke(c, s * 0.95, stroke);
            line((0., -0.15), (0., 0.6));
            p.circle_filled(point(0., -0.55), s * 0.1, color);
        }
        "warning" => {
            path(&[(0., -1.), (-1., 0.9), (1., 0.9), (0., -1.)]);
            line((0., -0.4), (0., 0.25));
            p.circle_filled(point(0., 0.6), s * 0.08, color);
        }
        "sort" => {
            line((-0.45, -0.8), (-0.45, 0.8));
            path(&[(-0.8, -0.4), (-0.45, -0.8), (-0.1, -0.4)]);
            line((0.45, -0.8), (0.45, 0.8));
            path(&[(0.1, 0.4), (0.45, 0.8), (0.8, 0.4)]);
        }
        "folder" => path(&[
            (-0.95, 0.7),
            (-0.95, -0.7),
            (-0.25, -0.7),
            (0., -0.4),
            (0.95, -0.4),
            (0.95, 0.7),
            (-0.95, 0.7),
        ]),
        "user" => {
            p.circle_stroke(point(0., -0.4), s * 0.4, stroke);
            path(&[
                (-0.8, 0.85),
                (-0.7, 0.35),
                (-0.3, 0.1),
                (0.3, 0.1),
                (0.7, 0.35),
                (0.8, 0.85),
            ]);
        }
        "sparkle" => {
            let mut mesh = egui::Mesh::default();
            for (dx, dy, radius) in [(-0.2, 0.1, 0.9), (0.85, -0.75, 0.35)] {
                let first = mesh.vertices.len() as u32;
                mesh.colored_vertex(point(dx, dy), color);
                for (x, y) in [
                    (0., -1.),
                    (0.26, -0.26),
                    (1., 0.),
                    (0.26, 0.26),
                    (0., 1.),
                    (-0.26, 0.26),
                    (-1., 0.),
                    (-0.26, -0.26),
                ] {
                    mesh.colored_vertex(point(dx + x * radius, dy + y * radius), color);
                }
                for i in 0..8 {
                    mesh.add_triangle(first, first + 1 + i, first + 1 + (i + 1) % 8);
                }
            }
            p.add(egui::Shape::mesh(mesh));
        }
        "close" => {
            line((-0.7, -0.7), (0.7, 0.7));
            line((0.7, -0.7), (-0.7, 0.7));
        }
        "minimize" => line((-0.8, 0.), (0.8, 0.)),
        "maximize" => {
            p.rect_stroke(
                Rect::from_min_max(point(-0.7, -0.7), point(0.7, 0.7)),
                0,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        _ => {}
    }
}
fn icon(ui: &mut egui::Ui, name: &str, size: f32, color: Color32) -> egui::Response {
    let (r, response) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let hover =
        ui.ctx()
            .animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.14);
    ui.painter().circle_filled(
        r.center(),
        size * 0.5,
        Color32::from_white_alpha((hover * 18.) as u8),
    );
    let pressed = if response.is_pointer_button_down_on() {
        1.5
    } else {
        0.
    };
    paint_icon(
        ui.painter(),
        r.shrink(size * 0.11 + pressed - hover * 0.7),
        name,
        color,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
fn labeled_control(
    p: &egui::Painter,
    rect: Rect,
    glyph: &str,
    text: &str,
    size: f32,
    color: Color32,
) {
    let galley = p.layout_no_wrap(text.into(), font(size, true), color);
    let icon_size = size * 1.6;
    let gap = size * 0.45;
    let width = icon_size + gap + galley.size().x;
    let start = pos2(rect.center().x - width * 0.5, rect.center().y);
    paint_icon(
        p,
        Rect::from_center_size(
            start + vec2(icon_size * 0.5, 0.),
            vec2(icon_size, icon_size),
        ),
        glyph,
        color,
    );
    p.galley(
        pos2(
            start.x + icon_size + gap,
            rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        color,
    );
}

impl App {
    pub(super) fn reference_background(&mut self, ctx: &egui::Context) {
        if matches!(self.page, Page::Playlist(_)) {
            self.playlist_background(ctx);
            return;
        }
        let rect = ctx.content_rect();
        let p = ctx.layer_painter(egui::LayerId::background());
        p.rect_filled(rect, 0, Color32::BLACK);
        if matches!(
            self.page,
            Page::Home
                | Page::Discovery(_)
                | Page::Mix(_)
                | Page::Album(..)
                | Page::Artist(_)
                | Page::Search
                | Page::Favorites
                | Page::Settings
                | Page::Profile
        ) {
            return;
        }
        let key = self.current.as_ref().and_then(|track| {
            [640, 320, 160, 80]
                .into_iter()
                .filter_map(|side| artwork_key(track, side))
                .find(|key| self.artwork.contains_key(key))
        });
        let art = key.and_then(|key| self.artwork.get_mut(&key)).map(|art| {
            art.touched = self.frame;
            &*art
        });
        let atmosphere = self.settings.cover_theme
            && (!matches!(self.page, Page::Playlist(_)) || self.settings.playlist_background);
        if atmosphere
            && self.settings.frosted_glass
            && let Some(a) = art
        {
            p.image(
                a.blur.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
                Color32::from_white_alpha(
                    (self.settings.effect_strength
                        * if self.page == Page::Lyrics {
                            380.
                        } else {
                            210.
                        })
                    .min(255.) as u8,
                ),
            );
            p.rect_filled(
                rect,
                0,
                Color32::from_black_alpha(if self.page == Page::Lyrics { 8 } else { 105 }),
            );
            if self.page == Page::Lyrics {
                let mut shade = egui::Mesh::default();
                for (position, alpha) in [
                    (rect.left_top(), 170),
                    (rect.right_top(), 170),
                    (rect.left_bottom(), 0),
                    (rect.right_bottom(), 0),
                ] {
                    shade.colored_vertex(position, Color32::from_black_alpha(alpha));
                }
                shade.add_triangle(0, 1, 2);
                shade.add_triangle(1, 3, 2);
                p.add(egui::Shape::mesh(shade));
            }
            return;
        }
        if !atmosphere {
            return;
        }
        let nx = 40;
        let ny = 28;
        let mut spots = [
            (0.23, 0.29, [128., 106., 4.], 21.),
            (0.45, 0.2, [0., 143., 192.], 21.),
            (0.54, 0.51, [112., 110., 195.], 8.),
            (0.90, 0.52, [47., 78., 0.], 16.),
            (0.79, 0.87, [110., 35., 6.], 15.),
            (0.27, 0.73, [3., 38., 111.], 14.),
        ];
        if let Some(art) = art {
            spots[0].2 = art.palette.map(|c| c as f32);
        }
        // Vertex colors depend only on the palette, not on the window size. They used to be
        // recomputed (about 7,000 exp() calls) on every frame; now only when the palette changes.
        let palette = spots[0].2.map(|c| c as u8);
        let colors = match &self.backdrop_grid {
            Some((cached, colors)) if *cached == palette => colors.clone(),
            _ => {
                let mut colors = Vec::with_capacity((nx + 1) * (ny + 1));
                for y in 0..=ny {
                    for x in 0..=nx {
                        let u = x as f32 / nx as f32;
                        let v = y as f32 / ny as f32;
                        let mut rgb = [0.0_f32; 3];
                        for (sx, sy, c, k) in spots {
                            let w = (-((u - sx).powi(2) + (v - sy).powi(2)) * k).exp();
                            for j in 0..3 {
                                rgb[j] += c[j] * w;
                            }
                        }
                        colors.push(Color32::from_rgb(
                            (rgb[0] * 0.36).min(255.) as u8,
                            (rgb[1] * 0.36).min(255.) as u8,
                            (rgb[2] * 0.36).min(255.) as u8,
                        ));
                    }
                }
                let colors = Arc::new(colors);
                self.backdrop_grid = Some((palette, colors.clone()));
                colors
            }
        };
        let mut mesh = egui::Mesh::default();
        mesh.vertices.reserve(colors.len());
        mesh.indices.reserve(nx * ny * 6);
        for y in 0..=ny {
            for x in 0..=nx {
                mesh.colored_vertex(
                    rect.min
                        + vec2(
                            x as f32 / nx as f32 * rect.width(),
                            y as f32 / ny as f32 * rect.height(),
                        ),
                    colors[y * (nx + 1) + x],
                );
            }
        }
        for y in 0..ny {
            for x in 0..nx {
                let a = (y * (nx + 1) + x) as u32;
                let b = a + 1;
                let c = a + (nx + 1) as u32;
                mesh.add_triangle(a, b, c);
                mesh.add_triangle(b, c + 1, c);
            }
        }
        p.add(egui::Shape::mesh(mesh));
    }
    pub(super) fn reference_title(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("title")
            .exact_height(32.)
            .frame(Frame::new().fill(Color32::from_black_alpha(210)))
            .show(ctx, |ui| {
                let full = ui.max_rect();
                let p = ui.painter().clone();
                p.text(
                    full.min + vec2(14., 15.),
                    Align2::LEFT_CENTER,
                    "UNDERTOW",
                    font(17., true),
                    TEXT,
                );
                let drag = Rect::from_min_max(
                    full.min + vec2(125., 0.),
                    pos2(full.right() - 180., full.bottom()),
                );
                let response = ui.interact(drag, Id::new("title-drag"), Sense::click_and_drag());
                if response.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if response.double_clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(
                        !ctx.input(|i| i.viewport().maximized.unwrap_or(false)),
                    ));
                }
                for (i, name) in ["close", "maximize", "minimize"].into_iter().enumerate() {
                    let r = Rect::from_min_size(
                        pos2(full.right() - (i + 1) as f32 * 60., full.top()),
                        vec2(60., 32.),
                    );
                    let b = ui
                        .interact(r, Id::new(("window", name)), Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    if b.hovered() {
                        p.rect_filled(
                            r,
                            0,
                            if name == "close" {
                                Color32::from_rgb(180, 35, 35)
                            } else {
                                Color32::from_white_alpha(18)
                            },
                        );
                    }
                    paint_icon(
                        &p,
                        Rect::from_center_size(r.center(), vec2(17., 17.)),
                        name,
                        TEXT,
                    );
                    if b.clicked() {
                        match name {
                            "close" => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                            "minimize" => {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true))
                            }
                            _ => ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(
                                !ctx.input(|i| i.viewport().maximized.unwrap_or(false)),
                            )),
                        }
                    }
                }
            });
    }
    pub(super) fn reference_sidebar(&mut self, ctx: &egui::Context) {
        self.navigation_sidebar(ctx);
    }
    pub(super) fn reference_toolbar(&mut self, ui: &mut egui::Ui) {
        if matches!(self.page, Page::Playlist(_)) {
            self.playlist_toolbar(ui);
            return;
        }
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 76.), Sense::hover());
        let p = ui.painter().clone();
        let arrows = Rect::from_min_size(r.min + vec2(22., 16.), vec2(93., 48.));
        p.rect_filled(arrows, 25, Color32::from_rgb(29, 29, 31));
        for (name, x) in [
            ("back", arrows.left() + 27.),
            ("forward", arrows.right() - 27.),
        ] {
            let a = Rect::from_center_size(pos2(x, arrows.center().y), vec2(31., 31.));
            let b = ui
                .interact(a, Id::new(("history", name)), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            paint_icon(&p, a, name, GREY);
            if b.clicked() {
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
        let avatar = Rect::from_center_size(pos2(r.right() - 35., r.top() + 40.), vec2(42., 42.));
        self.profile_button(ui, avatar);
        let width = (r.width() * 0.34).clamp(200., 368.);
        let search = Rect::from_min_size(
            pos2(avatar.left() - 24. - width, r.top() + 16.),
            vec2(width, 48.),
        );
        p.rect_filled(search, 25, Color32::from_rgba_unmultiplied(37, 37, 35, 240));
        paint_icon(
            &p,
            Rect::from_center_size(search.left_center() + vec2(23., 0.), vec2(25., 25.)),
            "search",
            GREY,
        );
        let response = at(
            ui,
            Rect::from_min_max(search.min + vec2(54., 7.), search.max - vec2(42., 6.)),
            "search-input",
            |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .font(egui::TextStyle::Body)
                        .font(FontId::proportional(18.))
                        .hint_text("Search")
                        .frame(false)
                        .desired_width(f32::INFINITY),
                )
            },
        );
        self.search_input_response(ui, &response, search);
        if self.signing_in {
            at(
                ui,
                Rect::from_min_max(
                    pos2(arrows.right() + 18., r.top() + 13.),
                    pos2(search.left() - 16., r.bottom()),
                ),
                "login-state",
                |ui| {
                    label(ui, &self.login_progress, 13., false, GREY);
                    if small_pill(ui, "Cancel sign-in").clicked() {
                        crate::auth::cancel_login();
                    }
                },
            );
        } else if !self.connected && arrows.right() + 196. < search.left() {
            at(
                ui,
                Rect::from_min_size(pos2(arrows.right() + 24., r.top() + 22.), vec2(160., 38.)),
                "connect",
                |ui| {
                    if pill(ui, "Connect TIDAL", true).clicked() {
                        self.busy = true;
                        self.send(Command::Login(self.settings.clone()));
                    }
                },
            );
        }
        if self.download_active {
            at(
                ui,
                Rect::from_min_size(r.min + vec2(22., 73.), vec2(400., 23.)),
                "download-progress",
                |ui| {
                    ui.horizontal(|ui| {
                        label(ui, "Downloading", 13., false, GREY);
                        if small_pill(ui, "Stop after current track").clicked() {
                            self.worker.download_cancel.store(true, Ordering::Relaxed);
                        }
                    });
                },
            );
        }
    }
    pub(super) fn reference_home(&mut self, ui: &mut egui::Ui) {
        let margin = (ui.available_width() * 0.052).clamp(30., 60.) as i8;
        Frame::new()
            .inner_margin(Margin {
                left: margin,
                right: (margin - 4).max(26),
                top: 0,
                bottom: 120,
            })
            .show(ui, |ui| {
                let (tabs, _) =
                    ui.allocate_exact_size(vec2(ui.available_width(), 40.), Sense::hover());
                for (i, (text, x)) in [("For you", 0.)].into_iter().enumerate() {
                    let color = if i == 0 { TEXT } else { GREY };
                    ui.painter().text(
                        tabs.min + vec2(x, 7.),
                        Align2::LEFT_TOP,
                        text,
                        font(18., true),
                        color,
                    );
                }
                ui.painter().line_segment(
                    [tabs.left_bottom(), tabs.right_bottom()],
                    Stroke::new(1.0_f32, Color32::from_white_alpha(35)),
                );
                ui.painter().line_segment(
                    [tabs.left_bottom(), tabs.left_bottom() + vec2(63., 0.)],
                    Stroke::new(3.0_f32, TEXT),
                );
                if self.connected {
                    at(
                        ui,
                        Rect::from_min_size(tabs.right_top() - vec2(88., 0.), vec2(88., 32.)),
                        "home-refresh",
                        |ui| {
                            if small_pill(ui, "Refresh").clicked() {
                                self.refresh_discovery();
                            }
                        },
                    );
                }
                ui.add_space(42.);
                if !self.connected {
                    card().show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        heading(ui, "Sign in to TIDAL", 26.);
                        ui.add_space(4.);
                        muted(
                            ui,
                            if self.signing_in {
                                self.login_progress.clone()
                            } else {
                                "One click — TIDAL opens in your browser with your code filled in."
                                    .into()
                            },
                        );
                        ui.add_space(14.);
                        if self.signing_in {
                            ui.horizontal(|ui| {
                                ui.add(egui::Spinner::new().color(ACCENT));
                                if small_pill(ui, "Cancel").clicked() {
                                    crate::auth::cancel_login();
                                }
                            });
                        } else if pill(ui, "Sign in with TIDAL", true).clicked() {
                            self.busy = true;
                            self.send(Command::Login(self.settings.clone()));
                        }
                    });
                    ui.add_space(26.);
                }
                if self.connected {
                    self.discovery_home(ui);
                }
            });
    }
    pub(super) fn reference_player(&mut self, ctx: &egui::Context) {
        let menu_queue = self.queue.clone();
        let screen = ctx.content_rect();
        let visibility = self.lyrics_chrome_visibility(ctx);
        if visibility <= 0. {
            return;
        }
        let rect = layout::player_rect(screen)
            .translate(vec2(0., reserved_player(ctx) * (1. - visibility)));
        egui::Area::new(Id::new("reference-player"))
            .fixed_pos(rect.min)
            .constrain(false)
            .order(egui::Order::Foreground)
            .movable(false)
            .show(ctx, |ui| {
                ui.set_clip_rect(screen);
                ui.set_opacity(visibility);
                if self.page == Page::Lyrics && self.lyrics_chrome_hidden {
                    ui.disable();
                }
                ui.set_min_size(rect.size());
                let (r, _) = ui.allocate_exact_size(rect.size(), Sense::hover());
                let p = ui.painter().clone();
                self.player_glass(&p, r);
                let duration = self.duration();
                let position = self.position();
                let progress = Rect::from_min_size(r.min, vec2(r.width(), 4.));
                p.rect_filled(
                    progress,
                    2,
                    Color32::from_rgba_unmultiplied(123, 124, 109, 95),
                );
                if duration > 0. {
                    p.rect_filled(
                        Rect::from_min_size(
                            progress.min,
                            vec2(
                                progress.width() * (position / duration).clamp(0., 1.) as f32,
                                4.,
                            ),
                        ),
                        2,
                        ACCENT,
                    );
                }
                let seek = ui.interact(
                    progress.expand2(vec2(0., 8.)),
                    Id::new("seek"),
                    Sense::click_and_drag(),
                );
                if duration > 0. && (seek.hovered() || seek.dragged()) {
                    let handle_x = progress.left()
                        + progress.width() * (position / duration).clamp(0., 1.) as f32;
                    p.rect_filled(
                        Rect::from_center_size(pos2(handle_x, progress.center().y), vec2(8., 18.)),
                        4,
                        TEXT,
                    );
                    if let Some(pointer) = ui.ctx().pointer_hover_pos() {
                        let time = crate::model::seek_time(
                            pointer.x,
                            progress.left(),
                            progress.width(),
                            duration,
                        );
                        let text = clock(time);
                        let bubble = Rect::from_center_size(
                            pos2(
                                pointer.x.clamp(r.left() + 30., r.right() - 30.),
                                progress.top() - 25.,
                            ),
                            vec2(54., 30.),
                        );
                        let preview = ui.ctx().layer_painter(egui::LayerId::new(
                            egui::Order::Tooltip,
                            Id::new("seek-preview"),
                        ));
                        preview.rect_filled(bubble, 6, Color32::WHITE);
                        preview.text(
                            bubble.center(),
                            Align2::CENTER_CENTER,
                            text,
                            font(15., true),
                            Color32::BLACK,
                        );
                    }
                }
                if (seek.clicked() || seek.dragged())
                    && duration > 0.
                    && let (Some(pos), Some(a)) = (seek.interact_pointer_pos(), &self.audio)
                {
                    a.seek(crate::model::seek_time(
                        pos.x,
                        progress.left(),
                        progress.width(),
                        duration,
                    ));
                }
                let track = self.current.clone();
                let title_width = ui
                    .painter()
                    .layout_no_wrap(
                        track
                            .as_ref()
                            .map(|t| t.title.as_str())
                            .unwrap_or("Choose a track")
                            .into(),
                        font(18., true),
                        TEXT,
                    )
                    .size()
                    .x;
                let artist_width = ui
                    .painter()
                    .layout_no_wrap(
                        track
                            .as_ref()
                            .map(|t| t.artist.as_str())
                            .unwrap_or("")
                            .into(),
                        font(16., false),
                        GREY,
                    )
                    .size()
                    .x;
                let metadata_width = (title_width
                    + if track.as_ref().is_some_and(|t| t.explicit) {
                        27.
                    } else {
                        0.
                    })
                .max(artist_width);
                let layout = PlayerLayout::new(r, self.page == Page::Lyrics, metadata_width);
                if let Some(cover) = layout.cover {
                    at(ui, cover, "player-cover", |ui| {
                        if self
                            .cover(ui, track.as_ref().unwrap_or(&Track::default()), 68.)
                            .clicked()
                        {
                            self.page = Page::Lyrics;
                        }
                    });
                }
                let [title_row, artist_row, context_row] = layout.metadata_rows();
                at(ui, title_row, "player-title", |ui| {
                    ui.spacing_mut().item_spacing = vec2(7., 0.);
                    let title = track
                        .as_ref()
                        .map(|t| t.title.as_str())
                        .unwrap_or("Choose a track");
                    let explicit = track.as_ref().is_some_and(|t| t.explicit);
                    let natural = title_width;
                    let width = natural
                        .min((ui.available_width() - if explicit { 27. } else { 0. }).max(24.));
                    ui.horizontal(|ui| {
                        ui.add_sized(
                            vec2(width, 22.),
                            egui::Label::new(
                                RichText::new(title).font(font(18., true)).color(TEXT),
                            )
                            .truncate(),
                        )
                        .on_hover_text(title);
                        if explicit {
                            ui.add(egui::Label::new(
                                RichText::new(" E ")
                                    .font(font(13., true))
                                    .color(Color32::BLACK)
                                    .background_color(TEXT),
                            ));
                        }
                    });
                });
                let artist_response = at(ui, artist_row, "player-artist", |ui| {
                    label(
                        ui,
                        track.as_ref().map(|t| t.artist.as_str()).unwrap_or(""),
                        16.,
                        false,
                        GREY,
                    )
                });
                if artist_response
                    .interact(Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text("Open artist")
                    .clicked()
                    && let Some(t) = &track
                {
                    self.open_track_artist(t);
                }
                let context = self
                    .playing_playlist
                    .as_ref()
                    .and_then(|id| self.playlists.iter().find(|p| p.id == *id))
                    .map(|p| p.title.as_str())
                    .unwrap_or("Tracks");
                at(ui, context_row, "player-source", |ui| {
                    label(ui, context, 15., false, GREY)
                });
                if let Some(t) = track.as_ref() {
                    let saved = self.favorite_ids.contains(&t.id);
                    let heart = layout.favorite;
                    let b = ui
                        .interact(heart, Id::new("favorite-current"), Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    paint_icon(
                        &p,
                        heart.shrink(7.),
                        if saved { "heart" } else { "heart-outline" },
                        if saved || b.hovered() { TEXT } else { GREY },
                    );
                    if b.clicked() && t.id > 0 && self.connected {
                        self.send(Command::Favorite(t.id, !saved));
                        self.send(Command::Favorites);
                    }
                    let dots = layout.menu;
                    paint_icon(&p, dots.shrink(5.), "dots", GREY);
                    let more = ui
                        .interact(dots, Id::new("current-menu"), Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    more.clone().context_menu(|ui| {
                        self.track_menu(ui, t, self.index, &menu_queue, None, false)
                    });
                    egui::Popup::menu(&more)
                        .show(|ui| self.track_menu(ui, t, self.index, &menu_queue, None, false));
                }
                self.transport_controls(ui, layout.transport, layout.transport_center);
                p.text(
                    layout.time,
                    Align2::CENTER_CENTER,
                    format!("{} | {}", clock(position), clock(duration)),
                    font(18., true),
                    Color32::from_gray(142),
                );
                let quality = layout.quality;
                if self.settings.show_quality {
                    p.rect_filled(quality, 14, Color32::from_rgb(0, 50, 46));
                    let format = self
                        .audio
                        .as_ref()
                        .map(|a| {
                            format!(
                                "{}-bit {:.1}kHz\n{}",
                                a.format.bit_depth,
                                a.format.sample_rate as f64 / 1000.,
                                if a.format.encoded_bitrate > 0 {
                                    format!("{}kb/s", a.format.encoded_bitrate / 1000)
                                } else {
                                    a.format.codec.clone()
                                }
                            )
                        })
                        .unwrap_or_else(|| "Lossless\nReady".into());
                    p.text(
                        quality.center(),
                        Align2::CENTER_CENTER,
                        format,
                        font(16., true),
                        ACCENT,
                    );
                    let quality_button = ui
                        .interact(quality, Id::new("quality"), Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text("Music information");
                    self.signal_popover(ui, &quality_button);
                }
                for (i, (name, tip)) in [
                    ("queue", "Queue"),
                    ("lyrics", "Lyrics"),
                    ("volume", "Volume · right-click to mute"),
                    ("float", "Floating lyrics"),
                ]
                .into_iter()
                .enumerate()
                {
                    if name == "lyrics" && !self.settings.one_click_lyrics {
                        continue;
                    }
                    let area = layout.extras[i];
                    if (name == "lyrics" && self.page == Page::Lyrics)
                        || (name == "queue" && self.queue_open)
                    {
                        p.circle_filled(area.center(), 20., Color32::from_white_alpha(20));
                    }
                    paint_icon(&p, area.shrink(6.), name, GREY);
                    let b = ui
                        .interact(area, Id::new(("player-control", name)), Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text(tip);
                    if name == "queue" {
                        self.queue_button_rect = Some(area);
                    }
                    if b.clicked() {
                        match name {
                            "queue" => self.queue_open = !self.queue_open,
                            "lyrics" => {
                                if self.page == Page::Lyrics && self.now_tab == 2 {
                                    self.page = Page::Home;
                                } else {
                                    self.page = Page::Lyrics;
                                    self.now_tab = 2;
                                    self.tab_started = Instant::now();
                                }
                            }
                            "float" => {
                                self.settings.floating_lyrics = !self.settings.floating_lyrics
                            }
                            "volume" => {}
                            _ => {}
                        }
                    }
                    if name == "volume" {
                        self.volume_popover(&b);
                    }
                }
            });
    }
    fn player_glass(&mut self, painter: &egui::Painter, rect: Rect) {
        painter.rect_filled(rect, 0, Color32::from_rgb(7, 8, 9));
        let art = self.current.as_ref().and_then(|track| {
            [640, 320, 160, 80]
                .into_iter()
                .filter_map(|side| artwork_key(track, side))
                .find(|key| self.artwork.contains_key(key))
        });
        if self.settings.frosted_glass
            && let Some(art) = art.and_then(|key| self.artwork.get_mut(&key))
        {
            art.touched = self.frame;
            // Reuse the worker's tiny preblurred texture. No per-frame screen readback,
            // Gaussian pass, extra image allocation or full-resolution framebuffer.
            painter.image(
                art.blur.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
                Color32::from_white_alpha((120. + self.settings.effect_strength * 135.) as u8),
            );
            painter.rect_filled(rect, 0, Color32::from_black_alpha(170));
            let mut shade = egui::Mesh::default();
            for (position, alpha) in [
                (rect.left_top(), 0),
                (rect.right_top(), 0),
                (rect.left_bottom(), 65),
                (rect.right_bottom(), 65),
            ] {
                shade.colored_vertex(position, Color32::from_black_alpha(alpha));
            }
            shade.add_triangle(0, 1, 2);
            shade.add_triangle(1, 3, 2);
            painter.add(egui::Shape::mesh(shade));
        }
    }

    pub(in crate::ui) fn lyrics_chrome_visibility(&self, ctx: &egui::Context) -> f32 {
        if self.page != Page::Lyrics {
            return 1.;
        }
        layout::ease(ctx.animate_bool_with_time(
            Id::new("lyrics-chrome-visible"),
            !self.lyrics_chrome_hidden,
            if self.settings.animated_lyrics {
                0.24 / self.settings.motion_speed
            } else {
                0.
            },
        ))
    }
    // Cached per playback generation; stays None until the cover has loaded.
    fn lyric_accent(&mut self) -> Option<Color32> {
        if !self.settings.cover_theme {
            return None;
        }
        if let Some((generation, color)) = self.accent
            && generation == self.generation
        {
            return Some(color);
        }
        let track = self.current.as_ref()?;
        let palette = [640, 320, 160, 80]
            .into_iter()
            .filter_map(|side| artwork_key(track, side))
            .find_map(|key| self.artwork.get(&key).map(|a| a.palette))?;
        let color = vibrant(palette);
        self.accent = Some((self.generation, color));
        Some(color)
    }
    pub(super) fn reference_lyrics(&mut self, ui: &mut egui::Ui) {
        let r = ui.max_rect();
        self.lyrics_controls(ui, r);
        let layout = layout::LyricsLayout::new(
            r,
            reserved_player(ui.ctx()) * self.lyrics_chrome_visibility(ui.ctx()),
        );
        let cover = layout.cover;
        let size = cover.width();
        let current = self.current.clone().unwrap_or_default();
        at(ui, cover, "lyrics-cover", |ui| {
            self.cover_with_radius(ui, &current, size, 12);
        });
        let lyrics_rect = layout.lines;
        let text_size = (self.settings.lyric_size * layout.font_scale * 2.).round() / 2.;
        if self.now_tab != 2 {
            self.now_playing_panel(ui, lyrics_rect);
            return;
        }
        let data = self.lyrics.clone();
        let position = self.lyric_position();
        let active = data.active(position);
        let text_width = (lyrics_rect.width() - 16.).max(80.);
        // Keep the sung text luminous white like the reference; artwork colors belong
        // in the backdrop and optional halo, never as a dark tint over the words.
        let custom = self.settings.lyric_color;
        let text_color = if custom.iter().all(|c| *c >= 235) {
            Color32::WHITE
        } else {
            Color32::from_rgb(custom[0], custom[1], custom[2])
        };
        let glow_color = if self.settings.lyric_cover_color {
            self.lyric_accent().unwrap_or(text_color)
        } else {
            text_color
        };
        let glow_color = Color32::from_rgba_unmultiplied(
            glow_color.r(),
            glow_color.g(),
            glow_color.b(),
            if self.settings.lyric_cover_color {
                180
            } else {
                110
            },
        );
        let idle_color = Color32::from_white_alpha(155);
        let unlit_color = Color32::from_white_alpha(125);
        at(ui, lyrics_rect, "lyrics-lines", |ui| {
            if data.lines.is_empty() {
                ui.add_space(lyrics_rect.height() * 0.25);
                label(
                    ui,
                    if self.current.is_some() {
                        &self.lyrics_status
                    } else {
                        "Play a track to see its lyrics"
                    },
                    24.,
                    true,
                    GREY,
                );
                return;
            }
            let key = (
                self.generation,
                text_width.round() as u32,
                (text_size * 10.) as u32,
                self.translated,
                data.lines.len(),
                ui.ctx().pixels_per_point().to_bits(),
            );
            if self.lyric_layout_key != Some(key) {
                self.lyric_offsets.clear();
                self.lyric_galleys.clear();
                self.lyric_offsets.push(0.);
                for line in &data.lines {
                    let size = crate::lyric_fonts::measure(
                        ui.ctx(),
                        &line.text,
                        font(text_size, true),
                        text_width,
                    );
                    let secondary = if self.translated { 30. } else { 0. };
                    let height = (size.y + 42. + secondary).max(100.);
                    self.lyric_offsets
                        .push(self.lyric_offsets.last().copied().unwrap_or(0.) + height);
                }
                self.lyric_layout_key = Some(key);
            }
            self.preview_lyric_navigation(ui.ctx());
            if lyric_navigation::scroll(
                ui,
                lyrics_rect,
                // Keep enough trailing room to center even the final sung line.
                *self.lyric_offsets.last().unwrap_or(&0.)
                    + if data.synced {
                        lyrics_rect.height() * 0.62
                    } else {
                        0.
                    },
                &mut self.lyric_scroll,
            ) {
                self.lyric_manual_scroll = true;
                self.ui_motion_until = Instant::now() + Duration::from_millis(220);
            }
            if data.synced && !self.lyric_manual_scroll {
                let target = active
                    .map(|i| (self.lyric_offsets[i] - lyrics_rect.height() * 0.38).max(0.))
                    .unwrap_or(0.);
                let dt = self.animation_clock.elapsed().as_secs_f32().min(0.1);
                self.animation_clock = Instant::now();
                let seeked = self
                    .lyric_last_position
                    .is_some_and(|last| position < last - 0.12 || position - last > 0.75);
                if seeked || self.lyric_entry_generation != self.generation {
                    self.lyric_scroll = target;
                    self.lyric_velocity = 0.;
                    self.lyric_entry_generation = self.generation;
                }
                self.lyric_last_position = Some(position);
                crate::lyrics::follow_scroll(
                    &mut self.lyric_scroll,
                    &mut self.lyric_velocity,
                    target,
                    dt * self.settings.motion_speed,
                );
                if !self.settings.animated_lyrics {
                    self.lyric_scroll = target;
                    self.lyric_velocity = 0.;
                }
                if self.settings.animated_lyrics
                    && (self.lyric_velocity.abs() > 0.05
                        || (self.lyric_scroll - target).abs() > 0.02)
                {
                    ui.ctx().request_repaint();
                }
            } else {
                if !data.synced && self.lyric_entry_generation != self.generation {
                    self.lyric_scroll = 0.;
                    self.lyric_entry_generation = self.generation;
                }
                self.lyric_velocity = 0.;
                self.lyric_last_position = Some(position);
            }
            let offsets = &self.lyric_offsets;
            {
                let viewport = Rect::from_min_size(pos2(0., self.lyric_scroll), lyrics_rect.size());
                let origin = ui.max_rect().min - vec2(0., self.lyric_scroll);
                let first = offsets
                    .partition_point(|&v| v < viewport.top())
                    .saturating_sub(1);
                let last = offsets
                    .partition_point(|&v| v < viewport.bottom())
                    .min(data.lines.len());
                self.lyric_galleys
                    .retain(|i, _| *i >= first.saturating_sub(2) && *i < last + 2);
                for i in first..last {
                    let line = &data.lines[i];
                    let galley = self
                        .lyric_galleys
                        .entry(i)
                        .or_insert_with(|| {
                            crate::lyric_fonts::layout(
                                ui.ctx(),
                                &line.text,
                                font(text_size, true),
                                text_width,
                            )
                        })
                        .clone();
                    let active_line = active == Some(i);
                    let area = Rect::from_min_size(
                        origin + vec2(0., offsets[i]),
                        vec2(text_width, offsets[i + 1] - offsets[i]),
                    );
                    at(ui, area, ("line", i), |ui| {
                        let size = text_size;
                        let emphasis = if !data.synced {
                            1.
                        } else if self.settings.animated_lyrics {
                            layout::ease(ui.ctx().animate_bool_with_time(
                                Id::new(("lyric-emphasis", self.generation, i)),
                                active_line,
                                0.14 / self.settings.motion_speed,
                            ))
                        } else if active_line {
                            1.
                        } else {
                            0.
                        };
                        let was_sung = active.is_some_and(|current| i < current);
                        let resting = if was_sung {
                            Color32::from_white_alpha(210)
                        } else {
                            idle_color
                        };
                        let color = resting.lerp_to_gamma(text_color, emphasis);
                        let mut word_seeked = false;
                        let response = if active_line && !line.words.is_empty() && galley.native() {
                            // Shape the complete line once. Highlight and seek visual
                            // word fragments without breaking Arabic joins/Indic clusters.
                            let response =
                                lyric_galley(ui, galley.clone(), unlit_color, None, None);
                            let mut cursor = 0;
                            for (word_index, word) in line.words.iter().enumerate() {
                                let Some(range) = crate::lyric_fonts::word_range(
                                    &line.text,
                                    &word.text,
                                    &mut cursor,
                                ) else {
                                    continue;
                                };
                                let fraction = if self.settings.animated_lyrics {
                                    ((position - word.start) / (word.end - word.start).max(0.06))
                                        .clamp(0., 1.) as f32
                                } else if position >= word.start {
                                    1.
                                } else {
                                    0.
                                };
                                for (fragment_index, (local, rtl)) in
                                    galley.fragments(range.start, range.end).iter().enumerate()
                                {
                                    let hit = local.translate(response.rect.min.to_vec2());
                                    if fraction > 0. {
                                        let mut clip = hit.expand2(vec2(0., 7.));
                                        if *rtl {
                                            clip.min.x = hit.right() - hit.width() * fraction;
                                        } else {
                                            clip.max.x = hit.left() + hit.width() * fraction;
                                        }
                                        let painter = ui
                                            .painter()
                                            .with_clip_rect(ui.clip_rect().intersect(clip));
                                        paint_lyric_galley(
                                            &painter,
                                            response.rect.min,
                                            galley.clone(),
                                            color,
                                            self.settings.lyric_glow.then_some(glow_color),
                                        );
                                    }
                                    if ui
                                        .interact(
                                            hit,
                                            Id::new((
                                                "native-lyric-word",
                                                i,
                                                word_index,
                                                fragment_index,
                                            )),
                                            Sense::click(),
                                        )
                                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                                        .clicked()
                                        && let Some(a) = &self.audio
                                    {
                                        word_seeked = true;
                                        a.seek(
                                            (word.start
                                                - self.settings.lyric_offset_ms as f64 / 1000.)
                                                .max(0.),
                                        );
                                    }
                                }
                            }
                            response
                        } else if active_line && !line.words.is_empty() {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.;
                                for word in &line.words {
                                    let fraction = if self.settings.animated_lyrics {
                                        ((position - word.start)
                                            / (word.end - word.start).max(0.06))
                                        .clamp(0., 1.)
                                            as f32
                                    } else if position >= word.start {
                                        1.
                                    } else {
                                        0.
                                    };
                                    let b = lyric_text(
                                        ui,
                                        &word.text,
                                        size,
                                        unlit_color,
                                        None,
                                        false,
                                        None,
                                    );
                                    if fraction > 0. {
                                        let clip = Rect::from_min_max(
                                            b.rect.min - vec2(6., 7.),
                                            pos2(
                                                b.rect.left() + b.rect.width() * fraction,
                                                b.rect.bottom() + 7.,
                                            ),
                                        );
                                        let painter = ui
                                            .painter()
                                            .with_clip_rect(ui.clip_rect().intersect(clip));
                                        let galley = crate::lyric_fonts::layout(
                                            ui.ctx(),
                                            &word.text,
                                            font(size, true),
                                            f32::INFINITY,
                                        );
                                        paint_lyric_galley(
                                            &painter,
                                            b.rect.min,
                                            galley,
                                            color,
                                            self.settings.lyric_glow.then_some(glow_color),
                                        );
                                    }
                                    if b.clicked()
                                        && let Some(a) = &self.audio
                                    {
                                        a.seek(
                                            (word.start
                                                - self.settings.lyric_offset_ms as f64 / 1000.)
                                                .max(0.),
                                        );
                                    }
                                }
                            })
                            .response
                        } else {
                            lyric_galley(
                                ui,
                                galley,
                                color,
                                (active_line
                                    && self.settings.lyric_glow
                                    && self.settings.effect_strength > 0.1)
                                    .then_some(glow_color),
                                if data.synced
                                    && !active_line
                                    && self.settings.lyric_motion_blur
                                    && self.settings.lyric_blur_strength > 0.
                                {
                                    Some(LyricBlur {
                                        radius: 3.5
                                            * (1. - emphasis)
                                            * self.settings.lyric_blur_strength.clamp(0., 1.),
                                        trail: (self.lyric_velocity * 0.006).clamp(-8., 8.)
                                            * self.settings.lyric_blur_strength.clamp(0., 1.),
                                    })
                                } else {
                                    None
                                },
                            )
                        };
                        response.context_menu(|ui| {
                            if self.settings.copy_lyrics && ui.button("Copy this line").clicked() {
                                ui.ctx().copy_text(line.text.clone());
                                ui.close();
                            }
                        });
                        if response.clicked()
                            && !word_seeked
                            && data.synced
                            && let Some(a) = &self.audio
                        {
                            a.seek(
                                (line.start - self.settings.lyric_offset_ms as f64 / 1000.).max(0.),
                            );
                        }
                        if self.translated {
                            let second = if self.settings.romanize {
                                &line.romanized
                            } else {
                                &line.translated
                            };
                            if !second.is_empty() {
                                label(ui, second, 20., false, GREY);
                            }
                        }
                    });
                }
            }
        });
    }
}

#[cfg(test)]
mod link_tests {
    use super::*;
    #[test]
    fn metadata_links_click_with_a_hand_cursor_and_editors_keep_the_text_cursor() {
        for editor in [false, true] {
            let ctx = egui::Context::default();
            assert!(crate::ui::load_font(&ctx, ""));
            crate::ui::style(&ctx);
            let pointer = pos2(40., 34.);
            let mut clicked = false;
            let mut value = "Search".to_owned();
            let mut cursor = egui::CursorIcon::Default;
            for frame in 0..3 {
                let mut events = vec![egui::Event::PointerMoved(pointer)];
                if frame > 0 {
                    events.push(egui::Event::PointerButton {
                        pos: pointer,
                        button: egui::PointerButton::Primary,
                        pressed: frame == 1,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(640., 480.))),
                        events,
                        time: Some(frame as f64 / 60.),
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            at(
                                ui,
                                Rect::from_min_size(pos2(20., 20.), vec2(300., 36.)),
                                "link-test",
                                |ui| {
                                    if editor {
                                        ui.add(egui::TextEdit::singleline(&mut value));
                                    } else {
                                        clicked |= text_link(ui, "THE PEAK", 18., true, TEXT, true)
                                            .clicked();
                                    }
                                },
                            );
                        });
                    },
                );
                cursor = output.platform_output.cursor_icon;
            }
            assert_eq!(
                cursor,
                if editor {
                    egui::CursorIcon::Text
                } else {
                    egui::CursorIcon::PointingHand
                }
            );
            if !editor {
                assert!(clicked);
            }
        }
    }
}
