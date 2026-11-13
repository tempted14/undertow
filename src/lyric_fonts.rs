//! Script-aware lyric fallback. Keep the existing atlas for ordinary text; let
//! Windows shape complex scripts and choose installed fallback fonts when needed.
//! Only visible lines get masks. A small LRU retains them across glow/scroll frames.
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Vec2};
use std::sync::Arc;

#[cfg(windows)]
pub(crate) mod native;

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
    /// Each hit is a visual fragment, with direction, of one logical UTF-8 range.
    /// Keeping the whole line shaped preserves joining and ligatures at word edges.
    pub fn fragments(&self, byte_start: usize, byte_end: usize) -> Arc<[(Rect, bool)]> {
        match self {
            #[cfg(windows)]
            Self::Native(line) => line.fragments(byte_start, byte_end),
            _ => Arc::default(),
        }
    }
    pub fn native(&self) -> bool {
        #[cfg(windows)]
        if matches!(self, Self::Native(_)) {
            return true;
        }
        false
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

/// Match sequential word-provider text without using byte counts as UTF-16 indices.
pub fn word_range(text: &str, word: &str, cursor: &mut usize) -> Option<std::ops::Range<usize>> {
    let word = word.trim();
    if word.is_empty() || !text.is_char_boundary(*cursor) {
        return None;
    }
    let start = *cursor + text.get(*cursor..)?.find(word)?;
    let end = start + word.len();
    *cursor = end;
    Some(start..end)
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
    #[test]
    fn repeated_unicode_word_ranges_keep_original_byte_boundaries() {
        let text = "ਪਿਆਰ ਪਿਆਰ 🌊";
        let mut cursor = 0;
        let first = super::word_range(text, "ਪਿਆਰ ", &mut cursor).unwrap();
        let second = super::word_range(text, "ਪਿਆਰ", &mut cursor).unwrap();
        assert_eq!(&text[first.clone()], "ਪਿਆਰ");
        assert_eq!(&text[second.clone()], "ਪਿਆਰ");
        assert!(second.start > first.end);
        assert_eq!(
            &text[super::word_range(text, "🌊", &mut cursor).unwrap()],
            "🌊"
        );
        assert!(super::word_range(text, "missing", &mut cursor).is_none());
    }
}
