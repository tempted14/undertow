use super::*;

/// Shared collection filter: centered glyphs, a visible focus ring, and a clear
/// target large enough to use without aiming at a tiny text character.
pub(super) fn track_filter(
    ui: &mut egui::Ui,
    rect: Rect,
    value: &mut String,
    hint: &str,
    id: &str,
) -> bool {
    let editor_id = Id::new((id, "editor"));
    // Egui clears focus at the start of an Escape frame; remember which editor
    // owned it, clear that filter and keep it ready for the next query.
    let escaped = !value.is_empty()
        && ui.memory(|m| m.has_focus(editor_id) || m.had_focus_last_frame(editor_id))
        && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    if escaped {
        value.clear();
        ui.memory_mut(|m| m.request_focus(editor_id));
    }
    let background = ui.painter().add(egui::Shape::Noop);
    paint_icon(
        ui.painter(),
        Rect::from_center_size(rect.left_center() + vec2(25., 0.), vec2(23., 23.)),
        "search",
        GREY,
    );
    let can_clear = !value.is_empty();
    let editor = Rect::from_center_size(
        pos2(
            (rect.left() + 53. + rect.right() - if can_clear { 48. } else { 18. }) * 0.5,
            rect.center().y,
        ),
        vec2(
            (rect.width() - 53. - if can_clear { 48. } else { 18. }).max(20.),
            26.,
        ),
    );
    let response = at(ui, editor, id, |ui| {
        ui.add(
            egui::TextEdit::singleline(value)
                .id(editor_id)
                .font(font(18., false))
                .hint_text(hint)
                .frame(false)
                .desired_width(f32::INFINITY)
                .margin(Vec2::ZERO),
        )
    });
    let mut changed = response.changed() || escaped;
    if can_clear {
        let target = Rect::from_center_size(rect.right_center() - vec2(24., 0.), vec2(34., 34.));
        let clear = ui
            .interact(target, Id::new((id, "clear")), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Clear filter");
        let hover = ui
            .ctx()
            .animate_bool_with_time(clear.id.with("hover"), clear.hovered(), 0.12);
        ui.painter()
            .rect_filled(target, 17, Color32::from_white_alpha((hover * 20.) as u8));
        paint_icon(
            ui.painter(),
            target.shrink(8.),
            "close",
            if clear.hovered() { TEXT } else { GREY },
        );
        if clear.clicked() {
            value.clear();
            response.request_focus();
            changed = true;
        }
    }
    ui.painter().set(
        background,
        egui::Shape::rect_stroke(
            rect,
            14,
            Stroke::new(
                1_f32,
                Color32::from_white_alpha(if response.has_focus() { 150 } else { 35 }),
            ),
            egui::StrokeKind::Inside,
        ),
    );
    changed
}

pub(super) fn filter_summary(ui: &egui::Ui, rect: Rect, matched: usize, total: usize) {
    ui.painter().text(
        rect.right_bottom() + vec2(0., 10.),
        Align2::RIGHT_TOP,
        format!("{matched} of {total} tracks"),
        font(14., false),
        GREY,
    );
}

pub(in crate::ui) fn input(
    ui: &mut egui::Ui,
    value: &mut String,
    hint: &str,
    height: f32,
    multiline: bool,
    limit: usize,
) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let background = ui.painter().add(egui::Shape::Noop);
    let response = at(ui, rect.shrink2(vec2(20., 16.)), ("input", hint), |ui| {
        let edit = if multiline {
            egui::TextEdit::multiline(value)
        } else {
            egui::TextEdit::singleline(value)
        };
        ui.add_sized(
            ui.available_size(),
            edit.frame(false)
                .font(font(18., false))
                .vertical_align(if multiline { Align::Min } else { Align::Center })
                .text_color(TEXT)
                .hint_text(egui::RichText::new(hint).color(Color32::from_rgb(132, 132, 142)))
                .char_limit(limit)
                .desired_width(f32::INFINITY),
        )
    });
    ui.painter().set(
        background,
        egui::Shape::rect_filled(rect, 12, Color32::from_rgb(24, 24, 27)),
    );
    ui.painter().rect_stroke(
        rect,
        12,
        Stroke::new(
            1_f32,
            if response.has_focus() {
                Color32::from_gray(185)
            } else {
                Color32::from_rgb(36, 36, 40)
            },
        ),
        egui::StrokeKind::Inside,
    );
    response
}

pub(super) fn button(
    ui: &mut egui::Ui,
    title: &str,
    size: Vec2,
    primary: bool,
    enabled: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        size,
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let hover = layout::ease(ui.ctx().animate_bool_with_time(
        response.id.with("hover"),
        enabled && response.hovered(),
        0.12,
    ));
    let fill = if !enabled {
        Color32::from_rgb(112, 112, 124)
    } else if primary {
        Color32::WHITE
    } else {
        Color32::from_rgb(36, 36, 40).lerp_to_gamma(Color32::from_rgb(49, 49, 54), hover)
    };
    ui.painter().rect_filled(rect, 15, fill);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        title,
        font(16., true),
        if primary {
            Color32::from_rgb(8, 13, 18)
        } else {
            TEXT
        },
    );
    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

impl App {
    pub(in crate::ui) fn playlist_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.dialog.take() else {
            return;
        };
        if draft.revision == 0 {
            draft.revision = rand::random::<u64>().max(1);
        }
        let mut close = false;
        let compact = ctx.content_rect().height() < 760.;
        let padding = if compact { 24 } else { 32 };
        let form_height = if compact { 244. } else { 326. };
        let width = (ctx.content_rect().width() - 72.).clamp(340., 800.);
        let modal=egui::Modal::new(Id::new("playlist-dialog")).frame(
            Frame::new().fill(Color32::from_rgb(24,24,27)).corner_radius(18)
                .stroke(Stroke::new(1_f32,Color32::from_gray(36)))
                .inner_margin(padding).shadow(egui::epaint::Shadow{offset:[0,16],blur:40,spread:0,color:Color32::from_black_alpha(130)})
        ).show(ctx,|ui| {
            ui.set_width(width-padding as f32*2.);
            ui.spacing_mut().item_spacing=vec2(16.,if compact {12.} else {16.});
            let (header,_)=ui.allocate_exact_size(vec2(ui.available_width(),36.),Sense::hover());
            ui.painter().text(header.left_center(),Align2::LEFT_CENTER,if draft.kind=="create" {"Create playlist"} else if draft.kind=="delete" {"Delete playlist"} else {"Edit playlist"},font(23.,true),TEXT);
            let x=Rect::from_center_size(header.right_center()-vec2(8.,0.),vec2(30.,30.));
            paint_icon(ui.painter(),x.shrink(6.),"close",TEXT);
            close=ui.interact(x,Id::new("close-playlist-dialog"),Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Close").clicked();
            egui::ScrollArea::vertical().id_salt("playlist-dialog-body").max_height((ctx.content_rect().height()-220.).max(220.)).show(ui,|ui| {
                if draft.kind=="delete" {
                    ui.add(egui::Label::new(RichText::new(format!("Delete ‘{}’ from your TIDAL account?",draft.title)).font(font(18.,false))).wrap());
                    ui.add_space(10.);
                    ui.label("This deletes the playlist on TIDAL for all synced devices.");
                } else {
                    let w=ui.available_width();
                    let side=(w*0.27).clamp(130.,197.)*if compact {0.84} else {1.};
                    let fields=Rect::from_min_size(ui.cursor().min+vec2(side+26.,0.),vec2(w-side-26.,form_height));
                    let art=Rect::from_min_size(ui.cursor().min,vec2(side,side));
                    if let Some(image)=&draft.preview {
                        ui.painter().image(image.id(),art,Rect::from_min_max(Pos2::ZERO,pos2(1.,1.)),Color32::WHITE);
                    } else if !draft.cover.is_empty() {
                        at(ui,art,"playlist-draft-cover",|ui|self.cover(ui,&Track{cover:draft.cover.clone(),..Default::default()},side));
                    } else { ui.painter().rect_filled(art,2,Color32::from_gray(104)); }
                    at(ui,Rect::from_min_size(art.left_bottom()+vec2(0.,16.),vec2(side,44.)),"pick-playlist-image",|ui| {
                        if button(ui,if draft.picking_image {"Choosing…"} else {"Change image"},vec2(side,44.),false,!draft.saving && !draft.picking_image).on_hover_text("Image saved on this device").clicked() {
                            draft.picking_image=true;
                            self.send(Command::PickPlaylistImage(draft.revision));
                        }
                    });
                    if !compact { at(ui,Rect::from_min_size(art.left_bottom()+vec2(0.,68.),vec2(side,52.)),"playlist-image-scope",|ui| {
                        ui.add(egui::Label::new(RichText::new("Image saved on this device").font(font(12.,false)).color(GREY)).wrap());
                    }); }
                    at(ui,fields,"playlist-draft-fields",|ui| {
                        ui.spacing_mut().item_spacing.y=0.;
                        ui.add_enabled_ui(!draft.saving,|ui| {
                            input(ui,&mut draft.title,"Title",if compact {72.} else {94.},false,250);
                            ui.add_space(16.);
                            input(ui,&mut draft.description,"Write a description",if compact {124.} else {184.},true,500);
                            ui.add_space(7.);
                            label(ui,format!("{}/500 characters",draft.description.chars().count()),14.,false,Color32::from_rgb(132,132,142));
                        });
                    });
                    ui.allocate_space(vec2(w,form_height));
                    ui.add_space(8.);
                    Frame::new().fill(Color32::from_rgb(30,30,34)).corner_radius(18)
                        .stroke(Stroke::new(1_f32,Color32::from_gray(36))).inner_margin(Margin::symmetric(22,20)).show(ui,|ui| {
                        ui.set_width(ui.available_width());
                        let (r,response)=ui.allocate_exact_size(vec2(ui.available_width(),58.),Sense::click());
                        let public=draft.public.unwrap_or(false);
                        if response.clicked() && !draft.saving {draft.public=Some(!public);}
                        ui.painter().text(r.left_top(),Align2::LEFT_TOP,"Make it public",font(18.,true),TEXT);
                        let text=ui.painter().layout("Your playlist will be visible on your Profile and accessible by anyone.".into(),font(15.,false),Color32::from_gray(175),(r.width()-86.).max(100.));
                        ui.painter().galley(r.left_top()+vec2(0.,29.),text,TEXT);
                        let rail=Rect::from_center_size(r.right_center()-vec2(28.,0.),vec2(55.,34.));
                        let t=layout::ease(ui.ctx().animate_bool_with_time(Id::new(("playlist-public",draft.revision)),draft.public.unwrap_or(false),0.16));
                        ui.painter().rect_filled(rail,18,Color32::from_gray(48).lerp_to_gamma(Color32::from_gray(231),t));
                        ui.painter().circle_filled(pos2(rail.left()+17.+21.*t,rail.center().y),12.,Color32::BLACK);
                    });
                }
                if !draft.problem.is_empty() {
                    ui.add(egui::Label::new(RichText::new(&draft.problem).font(font(14.,false)).color(Color32::from_rgb(255,147,147))).wrap());
                }
            });
            ui.add_space(8.);
            ui.with_layout(Layout::right_to_left(Align::Center),|ui| {
                let deleting=draft.kind=="delete";
                let enabled=!draft.saving && !draft.picking_image && (deleting || !draft.title.trim().is_empty());
                if button(ui,if draft.saving {"Saving…"} else if deleting {"Delete"} else {"Save"},vec2(if deleting {100.} else {76.},46.),true,enabled).clicked() {
                    if deleting {self.send(Command::Delete(draft.id.clone()));self.page=Page::Home;close=true;}
                    else {
                        draft.saving=true;draft.problem.clear();
                        self.send(Command::SavePlaylist(draft.revision,crate::model::PlaylistEdit {
                            id:draft.id.clone(),title:draft.title.trim().into(),description:draft.description.clone(),
                            public:if draft.kind=="create" {Some(draft.public.unwrap_or(false))} else {draft.public},image:draft.image.clone(),
                        }));
                    }
                }
                if draft.kind=="edit" && button(ui,"Delete playlist…",vec2(160.,46.),false,!draft.saving).clicked() {draft.kind="delete".into();}
            });
        });
        if !close && !(modal.should_close() && !draft.saving) {
            self.dialog = Some(draft);
        }
    }

    pub(in crate::ui) fn reference_toast(&mut self, ctx: &egui::Context) -> Option<Rect> {
        let visible = !self.notice.is_empty()
            && (self.notice_at.elapsed() < Duration::from_secs(if self.error { 30 } else { 9 })
                || self.busy);
        let progress =
            layout::ease(ctx.animate_bool_with_time(Id::new("toast-visible"), visible, 0.18));
        if progress <= 0. || self.notice.is_empty() {
            return None;
        }
        let message = self.notice.clone();
        let area = egui::Area::new(Id::new("toast"))
            .anchor(
                Align2::CENTER_BOTTOM,
                vec2(0., -reserved_player(ctx) - 18. + 10. * (1. - progress)),
            )
            .order(egui::Order::Tooltip)
            .show(ctx, |ui| {
                ui.set_opacity(progress);
                let limit = (ctx.content_rect().width() - 100.).clamp(220., 680.);
                Frame::new()
                    .fill(Color32::from_rgb(27, 43, 47))
                    .corner_radius(20)
                    .inner_margin(Margin::symmetric(16, 13))
                    .stroke(Stroke::new(1_f32, Color32::from_rgb(31, 48, 52)))
                    .show(ui, |ui| {
                        let galley = ui.painter().layout(
                            message,
                            font(13., true),
                            Color32::WHITE,
                            limit - 96.,
                        );
                        let (r, _) = ui.allocate_exact_size(
                            vec2(
                                (galley.size().x + 64.).max(210.).min(limit - 32.),
                                galley.size().y.max(24.),
                            ),
                            Sense::hover(),
                        );
                        ui.painter().galley(
                            r.left_center() - vec2(0., galley.size().y * 0.5),
                            galley,
                            Color32::WHITE,
                        );
                        let x = Rect::from_center_size(
                            r.right_center() - vec2(20., 0.),
                            vec2(40., 24.),
                        );
                        let dismiss = ui
                            .interact(
                                x.expand2(vec2(0., 4.)),
                                Id::new("dismiss-toast"),
                                Sense::click(),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text("Dismiss");
                        let hover = layout::ease(ctx.animate_bool_with_time(
                            dismiss.id.with("hover"),
                            dismiss.hovered(),
                            0.12,
                        ));
                        ui.painter().rect_filled(
                            x,
                            12,
                            Color32::from_white_alpha((hover * 12.) as u8),
                        );
                        ui.painter().rect_stroke(
                            x,
                            12,
                            Stroke::new(1_f32, Color32::from_rgb(47, 65, 69)),
                            egui::StrokeKind::Inside,
                        );
                        paint_icon(
                            ui.painter(),
                            Rect::from_center_size(x.center(), vec2(12., 12.)),
                            "close",
                            TEXT,
                        );
                        if dismiss.clicked() {
                            self.notice.clear();
                        }
                    });
            });
        Some(area.response.rect)
    }
}

#[cfg(test)]
mod filter_tests {
    use super::*;
    #[test]
    fn filter_clear_click_and_escape_clear_text_and_keep_editor_focus() {
        for clear_button in [true, false] {
            let ctx = egui::Context::default();
            assert!(crate::ui::load_font(&ctx, ""));
            let mut value = "Night".to_owned();
            let mut changed = false;
            let pointer = if clear_button {
                pos2(416., 46.)
            } else {
                pos2(110., 46.)
            };
            for frame in 0..4 {
                let mut events = vec![egui::Event::PointerMoved(pointer)];
                if frame == 1 || frame == 2 {
                    events.push(egui::Event::PointerButton {
                        pos: pointer,
                        button: egui::PointerButton::Primary,
                        pressed: frame == 1,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
                if frame == 3 && !clear_button {
                    events.push(egui::Event::Key {
                        key: egui::Key::Escape,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(640., 480.))),
                        events,
                        time: Some(frame as f64 / 60.),
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            changed |= track_filter(
                                ui,
                                Rect::from_min_size(pos2(20., 20.), vec2(420., 52.)),
                                &mut value,
                                "Filter tracks",
                                "filter-test",
                            );
                        });
                    },
                );
            }
            assert!(changed && value.is_empty(), "clear_button={clear_button}");
            assert!(
                ctx.wants_keyboard_input(),
                "The filter remains ready for another query"
            );
        }
    }
}
