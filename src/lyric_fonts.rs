//! Script-aware lyric fallback. Keep the existing atlas for ordinary text; let
//! Windows shape complex scripts and choose installed fallback fonts when needed.
//! Only visible lines get masks. A small LRU retains them across glow/scroll frames.
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Vec2};
use std::sync::Arc;

mod halo;
#[cfg(windows)]
pub(crate) mod native;

pub fn release_fullscreen_cache(ctx: &egui::Context) {
    halo::clear(ctx);
    #[cfg(windows)]
    native::clear_lines(ctx);
}

#[derive(Clone)]
pub enum Layout {
    Atlas(Arc<egui::Galley>),
    #[cfg(windows)]
    Native(Arc<native::Line>),
}

impl From<Arc<egui::Galley>> for Layout {
    fn from(value: Arc<egui::Galley>) -> Self {
        Self::Atlas(value)
    }
}

impl Layout {
    pub fn paint_halo(&self, painter: &egui::Painter, origin: Pos2, color: Color32) {
        halo::paint(self, painter, origin, color);
    }

    pub fn size(&self) -> Vec2 {
        match self {
            Self::Atlas(line) => line.size(),
            #[cfg(windows)]
            Self::Native(line) => line.size,
        }
    }
    pub fn paint(&self, painter: &egui::Painter, origin: Pos2, color: Color32) {
        match self {
            Self::Atlas(line) => {
                painter.galley_with_override_text_color(origin, line.clone(), color);
            }
            #[cfg(windows)]
            Self::Native(line) => line.paint(painter, origin, color),
        }
    }
}

pub fn layout(ctx: &egui::Context, text: &str, font: FontId, width: f32) -> Layout {
    #[cfg(windows)]
    if (requires_shaping(text)
        || (!text.is_ascii() && !ctx.fonts_mut(|f| f.has_glyphs(&font, text))))
        && let Some(line) = native::cached(ctx, text, font.size, width)
    {
        return Layout::Native(line);
    }
    Layout::Atlas(ctx.fonts_mut(|f| f.layout(text.to_owned(), font, Color32::WHITE, width)))
}

/// Measure long lyric sets without allocating a bitmap for every offscreen line.
pub fn measure(ctx: &egui::Context, text: &str, font: FontId, width: f32) -> Vec2 {
    #[cfg(windows)]
    if (requires_shaping(text)
        || (!text.is_ascii() && !ctx.fonts_mut(|f| f.has_glyphs(&font, text))))
        && let Some(size) = native::measure(ctx, text, font.size, width)
    {
        return size;
    }
    ctx.fonts_mut(|f| {
        f.layout(text.to_owned(), font, Color32::WHITE, width)
            .size()
    })
}

fn requires_shaping(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(c as u32,
        0x0300..=0x036f | // combining Latin marks
        0x0590..=0x109f | // Hebrew, Arabic, Indic, Thai, Lao, Tibetan, Myanmar
        0x1100..=0x11ff | // decomposed Hangul jamo
        0x1780..=0x18af | // Khmer, Mongolian
        0x1900..=0x1cff | // Tai and South/Southeast Asian extensions
        0x1dc0..=0x1dff | // supplementary combining marks
        0x200c..=0x200f | 0x202a..=0x202e | 0x2066..=0x2069 | // joining/bidi controls
        0xa800..=0xabff | 0xd7b0..=0xd7ff | 0xfb1d..=0xfdff | 0xfe20..=0xfe2f | 0xfe70..=0xfeff |
        0x10a00..=0x10fff | 0x11000..=0x11fff | 0x1e800..=0x1eeff)
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn complex_scripts_and_combining_marks_use_contextual_layout() {
        for text in [
            "ਪੰਜਾਬੀ",
            "हिन्दी",
            "বাংলা",
            "தமிழ்",
            "العربية",
            "עברית",
            "ภาษาไทย",
            "မြန်မာ",
            "ខ្មែរ",
            "e\u{301}",
            "한글",
        ] {
            assert!(super::requires_shaping(text), "{text}");
        }
        for text in [
            "English",
            "Español",
            "Français",
            "Ελληνικά",
            "Русский",
            "日本語",
            "한국어",
        ] {
            assert!(!super::requires_shaping(text), "{text}");
        }
    }
}
