use super::*;
use crate::{lyric_fonts::Layout, lyrics::Line};

struct Fragment {
    word: usize,
    rect: Rect,
    rtl: bool,
}

pub(in crate::ui) struct WordLayout {
    pub index: usize,
    fragments: Vec<Fragment>,
}

pub(super) struct Colors {
    pub resting: Color32,
    pub fill: Color32,
    pub glow: Option<Color32>,
}

impl WordLayout {
    pub fn new(index: usize, line: &Line, layout: &Layout) -> Self {
        let mut cursor = 0;
        let mut fragments = Vec::new();
        for (word_index, word) in line.words.iter().enumerate() {
            if let Some(range) = crate::lyric_fonts::word_range(&line.text, &word.text, &mut cursor)
            {
                fragments.extend(layout.fragments(range.start, range.end).iter().map(
                    |&(rect, rtl)| Fragment {
                        word: word_index,
                        rect,
                        rtl,
                    },
                ));
            }
        }
        Self { index, fragments }
    }

    pub fn len(&self) -> usize {
        self.fragments.len()
    }

    pub(super) fn paint(
        &self,
        ui: &mut egui::Ui,
        layout: &Layout,
        line: &Line,
        position: f64,
        animated: bool,
        colors: Colors,
    ) -> (egui::Response, Option<f64>) {
        if self.fragments.is_empty() {
            return (
                lyric_galley(ui, layout.clone(), colors.fill, colors.glow, None),
                None,
            );
        }
        let response = lyric_galley(ui, layout.clone(), colors.resting, None, None);
        let origin = response.rect.min;
        let painter = ui.painter().clone();
        let paint = |rect: Rect| {
            if let Some(glow) = colors.glow {
                paint_lyric_halo(
                    &painter.with_clip_rect(rect.expand2(vec2(5., 0.))),
                    origin,
                    layout,
                    glow,
                );
            }
            layout.paint(&painter.with_clip_rect(rect), origin, colors.fill);
        };
        let mut pending: Option<Rect> = None;
        let mut seek = None;
        for (i, fragment) in self.fragments.iter().enumerate() {
            let word = &line.words[fragment.word];
            let hit = fragment.rect.translate(origin.to_vec2());
            if ui
                .interact(
                    hit,
                    ui.id().with(("lyric-word", self.index, i)),
                    Sense::click(),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                seek = Some(word.start);
            }
            let fraction = if animated {
                ((position - word.start) / (word.end_time() - word.start)).clamp(0., 1.) as f32
            } else {
                f32::from(position >= word.start)
            };
            if fraction <= 0. {
                if let Some(clip) = pending.take() {
                    paint(clip);
                }
                continue;
            }
            let mut clip = hit.expand2(vec2(0., 7.));
            // A partially sung word lights only the portion reached by the audio.
            if fragment.rtl {
                clip.min.x = hit.right() - hit.width() * fraction;
            } else {
                clip.max.x = hit.left() + hit.width() * fraction;
            }
            if let Some(previous) = pending
                && previous.top() == clip.top()
                && previous.bottom() == clip.bottom()
                && previous.left() <= clip.right() + 16.
                && clip.left() <= previous.right() + 16.
            {
                pending = Some(previous.union(clip));
            } else if let Some(previous) = pending.replace(clip) {
                paint(previous);
            }
            if fraction < 1.
                && let Some(clip) = pending.take()
            {
                paint(clip);
            }
        }
        if let Some(clip) = pending {
            paint(clip);
        }
        (response, seek)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lyrics::Word;

    #[test]
    fn two_syllables_share_one_word_layout_and_highlight_separately() {
        let ctx = egui::Context::default();
        let lyrics =
            crate::lyrics::parse_lrc("[00:01.00]<00:01.00>Em<00:01.50>bers\n[00:02.00]drift");
        let line = &lyrics.lines[0];
        let mut widths = Vec::new();
        for position in [1.25, 1.5, 1.75] {
            let result = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let layout = crate::lyric_fonts::layout(
                        ctx,
                        &line.text,
                        FontId::proportional(32.),
                        450.,
                    );
                    let syllables = WordLayout::new(0, line, &layout);
                    assert_eq!(syllables.fragments.len(), 2);
                    assert_eq!(
                        syllables.fragments[0].rect.right(),
                        syllables.fragments[1].rect.left()
                    );
                    syllables.paint(
                        ui,
                        &layout,
                        line,
                        position,
                        true,
                        Colors {
                            resting: GREY,
                            fill: TEXT,
                            glow: None,
                        },
                    );
                });
            });
            widths.push(result.shapes.iter().rev().find(|shape| {
                matches!(&shape.shape, egui::Shape::Text(t) if t.galley.text() == "Embers")
            }).unwrap().clip_rect.width());
        }
        assert!(widths.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn consecutive_sung_words_share_one_halo_without_moving_their_hit_targets() {
        let ctx = egui::Context::default();
        let line = Line {
            text: "One two three".into(),
            words: ["One", "two", "three"]
                .into_iter()
                .enumerate()
                .map(|(i, text)| Word {
                    text: text.into(),
                    start: i as f64,
                    end: i as f64 + 0.9,
                })
                .collect(),
            ..Default::default()
        };
        let mut pointer = Pos2::ZERO;
        for pressed in [None, Some(true), Some(false)] {
            let mut events = vec![egui::Event::PointerMoved(pointer)];
            if let Some(pressed) = pressed {
                events.push(egui::Event::PointerButton {
                    pos: pointer,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                });
            }
            let out = ctx.run(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let layout = crate::lyric_fonts::layout(
                            ctx,
                            &line.text,
                            FontId::proportional(32.),
                            450.,
                        );
                        let hits = WordLayout::new(0, &line, &layout);
                        let (response, seek) = hits.paint(
                            ui,
                            &layout,
                            &line,
                            2.4,
                            true,
                            Colors {
                                resting: GREY,
                                fill: TEXT,
                                glow: Some(Color32::LIGHT_BLUE),
                            },
                        );
                        assert_eq!(response.rect.size(), layout.size());
                        pointer = response.rect.min + hits.fragments[1].rect.center().to_vec2();
                        if pressed == Some(false) {
                            assert_eq!(seek, Some(1.));
                        }
                    });
                },
            );
            let copies = out.shapes.iter().filter(|shape| {
                matches!(&shape.shape, egui::Shape::Text(t) if t.galley.text() == line.text)
            }).count();
            // One resting line plus one merged bright fill and its eight halo taps.
            assert_eq!(copies, 10);
        }
    }
}
