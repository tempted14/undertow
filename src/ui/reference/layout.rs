//! Shared geometry keeps hit targets and painted content aligned during resizing.
use super::*;

pub(super) fn player_height(width: f32) -> f32 {
    if width < 1200. { 180. } else { 116. }
}

pub(in crate::ui) fn reserved_player(ctx: &egui::Context) -> f32 {
    player_height(ctx.content_rect().width())
}

pub(super) fn player_rect(screen: Rect) -> Rect {
    Rect::from_min_max(
        pos2(
            screen.left(),
            screen.bottom() - player_height(screen.width()),
        ),
        screen.right_bottom(),
    )
}

pub(super) fn lyrics_control_rects(page: Rect) -> [Rect; 6] {
    let widths = [152., 96., 114., 40., 40., 38.];
    let gap = 10.;
    let total = widths.iter().sum::<f32>() + gap * 5.;
    let mut x = page.right() - 36. - total;
    std::array::from_fn(|i| {
        let r = Rect::from_center_size(
            pos2(x + widths[i] * 0.5, page.top() + 46.),
            vec2(widths[i], 44.),
        );
        x += widths[i] + gap;
        r
    })
}

pub(super) fn columns(width: f32, minimum: f32, gap: f32, maximum: usize) -> usize {
    (((width + gap) / (minimum + gap)).floor() as usize).clamp(1, maximum)
}

pub(in crate::ui) fn queue_rect(screen: Rect, footer: f32) -> Rect {
    Rect::from_min_max(
        pos2(
            (screen.right() - 454.).max(screen.left() + 16.),
            screen.top() + 44.,
        ),
        pos2(screen.right() - 12., screen.bottom() - footer - 12.),
    )
}
pub(in crate::ui) fn caption_rect(
    screen: Rect,
    footer: f32,
    sidebar: f32,
    queue: bool,
    natural_width: f32,
) -> Option<Rect> {
    if screen.height() - footer < 360. {
        return None;
    }
    let left = screen.left() + if queue { sidebar } else { 0. };
    let right = if queue {
        queue_rect(screen, footer).left() - 16.
    } else {
        screen.right()
    };
    let width = natural_width.min((right - left - 80.).max(0.));
    (width >= 100.).then(|| {
        Rect::from_center_size(
            pos2((left + right) * 0.5, screen.bottom() - footer - 43.),
            vec2(width, 54.),
        )
    })
}

/// Zero velocity and acceleration at both ends; timing is independent of FPS.
pub(in crate::ui) fn ease(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    t * t * t * (t * (t * 6. - 15.) + 10.)
}

pub(super) fn seek_preview_x(player: Rect, pointer: f32) -> f32 {
    if !player.is_finite() || !pointer.is_finite() {
        return 0.;
    }
    if player.width() <= 0. {
        return player.left();
    }
    let inset = 30f32.min(player.width() * 0.5);
    pointer.clamp(player.left() + inset, player.right() - inset)
}

pub(super) struct LyricsLayout {
    pub cover: Rect,
    pub lines: Rect,
    pub font_scale: f32,
}
impl LyricsLayout {
    pub fn new(page: Rect, footer: f32) -> Self {
        let body = Rect::from_min_max(
            page.min + vec2(0., 100.),
            pos2(page.right(), page.bottom() - footer - 24.),
        );
        let side = (page.width() * 0.36).min(body.height() * 0.96).max(80.);
        let cover = Rect::from_center_size(
            pos2(
                page.left() + page.width() * 0.05 + side * 0.5,
                body.center().y,
            ),
            vec2(side, side),
        );
        let lines = Rect::from_min_max(
            pos2(page.left() + page.width() * 0.47, body.top()),
            pos2(
                page.right() - (page.width() * 0.035).clamp(24., 72.),
                body.bottom(),
            ),
        );
        Self {
            cover,
            lines,
            font_scale: (lines.width() / 650.).clamp(0.72, 1.),
        }
    }
}

pub(super) struct PlayerLayout {
    pub metadata: Rect,
    pub cover: Option<Rect>,
    pub transport: Rect,
    pub transport_center: f32,
    pub time: Pos2,
    pub quality: Rect,
    pub favorite: Rect,
    pub menu: Rect,
    pub extras: [Rect; 4],
}

impl PlayerLayout {
    pub fn new(r: Rect, lyrics: bool, text_width: f32) -> Self {
        let stacked = r.height() > 140.;
        let transport = r.translate(vec2(0., if stacked { 76. } else { 0. }));
        let transport_left = r.center().x - 150.;
        let quality = Rect::from_min_size(pos2(r.right() - 168., r.top() + 27.), vec2(146., 45.));
        let text_x = r.left() + if lyrics { 24. } else { 113. };
        // Keep track actions attached to the song, rather than to the middle of
        // the window. Long metadata truncates before the action group.
        let group_limit = if stacked {
            quality.left() - 16.
        } else {
            transport_left - 20.
        };
        let text_width = text_width
            .clamp(120., 300.)
            .min((group_limit - text_x - 96.).max(0.));
        let favorite_x = text_x + text_width + 34.;
        let metadata = Rect::from_min_size(pos2(text_x, r.top() + 24.), vec2(text_width, 68.));
        let extras_start = if stacked {
            r.right() - 222.
        } else {
            quality.left() - 220.
        };
        let transport_center = if stacked {
            r.center().x.min(extras_start - 185.)
        } else {
            r.center().x
        };
        Self {
            metadata,
            cover: (!lyrics).then(|| Rect::from_min_size(r.min + vec2(24., 24.), vec2(68., 68.))),
            transport,
            transport_center,
            time: pos2(transport_center, r.top() + if stacked { 158. } else { 88. }),
            quality,
            favorite: Rect::from_center_size(pos2(favorite_x, r.top() + 59.), vec2(44., 44.)),
            menu: Rect::from_center_size(pos2(favorite_x + 44., r.top() + 59.), vec2(36., 36.)),
            extras: std::array::from_fn(|i| {
                Rect::from_center_size(
                    pos2(
                        extras_start + i as f32 * 52.,
                        r.top() + if stacked { 122. } else { 59. },
                    ),
                    vec2(38., 38.),
                )
            }),
        }
    }
    pub fn metadata_rows(&self) -> [Rect; 3] {
        [(0., 22.), (26., 20.), (50., 18.)].map(|(y, height)| {
            Rect::from_min_size(
                self.metadata.min + vec2(0., y),
                vec2(self.metadata.width(), height),
            )
        })
    }
}

pub(in crate::ui) fn resize_hit(
    screen: Rect,
    pointer: Pos2,
) -> Option<(egui::ResizeDirection, egui::CursorIcon)> {
    if !screen.contains(pointer) {
        return None;
    }
    let p = pointer - screen.min;
    let left = p.x < 12.;
    let right = p.x >= screen.width() - 12.;
    let top = p.y < 12.;
    let bottom = p.y >= screen.height() - 12.;
    let near_left = p.x < 26.;
    let near_right = p.x >= screen.width() - 26.;
    let near_top = p.y < 26.;
    let near_bottom = p.y >= screen.height() - 26.;
    use egui::CursorIcon as C;
    use egui::ResizeDirection as D;
    if near_left && near_top {
        Some((D::NorthWest, C::ResizeNwSe))
    } else if near_right && near_top {
        Some((D::NorthEast, C::ResizeNeSw))
    } else if near_left && near_bottom {
        Some((D::SouthWest, C::ResizeNeSw))
    } else if near_right && near_bottom {
        Some((D::SouthEast, C::ResizeNwSe))
    } else if left {
        Some((D::West, C::ResizeHorizontal))
    } else if right {
        Some((D::East, C::ResizeHorizontal))
    } else if top {
        Some((D::North, C::ResizeVertical))
    } else if bottom {
        Some((D::South, C::ResizeVertical))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn seek_preview_handles_collapsed_player_bounds() {
        for width in [-30., 0., 1., 24., 59., 60., 640.] {
            let rect = Rect::from_min_size(pos2(40., 100.), vec2(width, 116.));
            for pointer in [-100., 50., 2000.] {
                let x = seek_preview_x(rect, pointer);
                assert!(x.is_finite());
                assert!(x >= rect.left() && x <= rect.right().max(rect.left()));
                if width >= 60. {
                    assert_eq!(x, pointer.clamp(rect.left() + 30., rect.right() - 30.));
                }
            }
        }
    }
    use super::*;
    #[test]
    fn queue_and_live_caption_do_not_overlap_when_the_window_is_resized() {
        for width in (640..=3840).step_by(31) {
            let screen = Rect::from_min_size(Pos2::ZERO, vec2(width as f32, 600.));
            let footer = player_height(width as f32);
            let sidebar = if width < 1100 { 76. } else { 315. };
            let queue = queue_rect(screen, footer);
            if let Some(caption) = caption_rect(screen, footer, sidebar, true, 1000.) {
                assert!(!caption.intersects(queue));
                assert!(caption.left() >= sidebar);
                assert!(caption.bottom() <= screen.bottom() - footer);
            }
        }
    }
    #[test]
    fn lyrics_resize_keeps_art_text_and_footer_separate() {
        for width in (640..=3840).step_by(31) {
            for height in (480..=2160).step_by(47) {
                let page =
                    Rect::from_min_size(pos2(0., 32.), vec2(width as f32, height as f32 - 32.));
                let p = LyricsLayout::new(page, player_height(width as f32));
                assert!(page.contains_rect(p.cover) && page.contains_rect(p.lines));
                assert!(!p.cover.intersects(p.lines));
                assert!(p.cover.bottom() < page.bottom() - player_height(width as f32));
                assert!(p.lines.bottom() < page.bottom() - player_height(width as f32));
                assert!((0.72..=1.).contains(&p.font_scale));
            }
        }
    }

    #[test]
    fn player_hit_targets_do_not_overlap_at_supported_sizes() {
        for width in (640..=3840).step_by(7) {
            let screen = Rect::from_min_size(pos2(7., 13.), vec2(width as f32, 900.));
            let r = player_rect(screen);
            assert_eq!(r.left(), screen.left());
            assert_eq!(r.right_bottom(), screen.right_bottom());
            for lyrics in [false, true] {
                let p = PlayerLayout::new(r, lyrics, 300.);
                let transport = Rect::from_center_size(
                    pos2(p.transport_center, p.transport.top() + 46.),
                    vec2(300., 52.),
                );
                let mut regions = vec![p.metadata, transport, p.quality, p.favorite, p.menu];
                regions.extend(p.extras);
                if let Some(cover) = p.cover {
                    regions.push(cover);
                }
                for (i, region) in regions.iter().enumerate() {
                    assert!(r.contains_rect(*region), "{width}: {region:?}");
                    for other in &regions[i + 1..] {
                        assert!(
                            !region.intersects(*other),
                            "{width}: {region:?} overlaps {other:?}"
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn track_actions_stay_with_the_metadata_for_short_and_long_titles() {
        for width in (640..=3840).step_by(19) {
            let r = player_rect(Rect::from_min_size(pos2(7., 13.), vec2(width as f32, 900.)));
            for lyrics in [false, true] {
                for text in [20., 180., 1200.] {
                    let p = PlayerLayout::new(r, lyrics, text);
                    assert!((p.favorite.left() - p.metadata.right() - 12.).abs() < 0.01);
                    assert_eq!(p.favorite.center().y, p.menu.center().y);
                    assert!(p.menu.right() - p.metadata.left() <= 400.);
                    assert!(!p.metadata.intersects(p.favorite));
                    assert!(!p.favorite.intersects(p.menu));
                    assert!(
                        p.menu.right()
                            < if r.height() > 140. {
                                p.quality.left()
                            } else {
                                p.transport_center - 150.
                            }
                    );
                }
            }
        }
    }
    #[test]
    fn metadata_rows_fit_and_align_with_the_cover() {
        for width in (640..=3840).step_by(31) {
            let r = player_rect(Rect::from_min_size(Pos2::ZERO, vec2(width as f32, 480.)));
            let p = PlayerLayout::new(r, false, 300.);
            let cover = p.cover.unwrap();
            assert_eq!(p.metadata.top(), cover.top());
            assert_eq!(p.metadata.bottom(), cover.bottom());
            let rows = p.metadata_rows();
            for (i, row) in rows.iter().enumerate() {
                assert!(p.metadata.contains_rect(*row));
                assert!(rows[i + 1..].iter().all(|other| !row.intersects(*other)));
            }
        }
    }
    #[test]
    fn generous_resize_edges_leave_the_content_and_header_buttons_clear() {
        let r = Rect::from_min_size(pos2(7., 13.), vec2(640., 480.));
        use egui::ResizeDirection as D;
        assert!(matches!(
            resize_hit(r, r.left_center() + vec2(11., 0.)),
            Some((D::West, _))
        ));
        assert!(matches!(
            resize_hit(r, r.right_bottom() - vec2(25., 25.)),
            Some((D::SouthEast, _))
        ));
        assert!(resize_hit(r, r.center()).is_none());
        assert!(resize_hit(r, r.right_top() + vec2(-30., 15.)).is_none());
        assert!(resize_hit(r, r.left_top() - vec2(1., 0.)).is_none());
    }

    #[test]
    fn lyrics_controls_remain_separate_and_hidden_view_reclaims_footer() {
        for width in (640..=3840).step_by(31) {
            let page = Rect::from_min_size(pos2(0., 32.), vec2(width as f32, 568.));
            let controls = lyrics_control_rects(page);
            for (i, r) in controls.iter().enumerate() {
                assert!(page.contains_rect(*r));
                assert!(controls[i + 1..].iter().all(|other| !r.intersects(*other)));
            }
            let visible = LyricsLayout::new(page, player_height(width as f32));
            let hidden = LyricsLayout::new(page, 0.);
            assert!(hidden.lines.bottom() > visible.lines.bottom());
            assert!(!hidden.lines.intersects(hidden.cover));
        }
    }

    #[test]
    fn reflow_preserves_card_minimums_and_easing_is_monotonic() {
        for width in (270..=2200).step_by(3) {
            let count = columns(width as f32, 250., 10., 3);
            assert!((width as f32 - (count - 1) as f32 * 10.) / count as f32 >= 250.);
        }
        let mut previous = 0.;
        for n in 0..=1000 {
            let current = ease(n as f32 / 1000.);
            assert!(current >= previous - 0.000002);
            previous = current;
        }
        assert_eq!(ease(0.), 0.);
        assert_eq!(ease(1.), 1.);
        assert!(ease(0.001) < 0.000001);
        assert!(1. - ease(0.999) < 0.000002);
    }
}
