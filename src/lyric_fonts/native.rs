use eframe::egui::{self, Color32, Pos2, Rect, TextureHandle, Vec2, pos2, vec2};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
};
use windows::{
    Win32::{
        Foundation::{COLORREF, E_FAIL},
        Graphics::{DirectWrite::*, Gdi::*},
    },
    core::{BOOL, IUnknown, Interface, Ref, Result, implement},
};

const BUDGET: usize = 8 * 1024 * 1024;
const CAPACITY: usize = 24;
const PAD: f32 = 5.;

pub struct Engine {
    factory: IDWriteFactory,
    params: IDWriteRenderingParams,
}

impl Engine {
    pub fn new() -> Result<Self> {
        unsafe {
            let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let params = factory.CreateRenderingParams()?;
            Ok(Self { factory, params })
        }
    }

    /// The small floating window uses the same fallback/shaping engine without
    /// creating another egui context, atlas or GPU cache.
    pub fn paint_gdi(
        &self,
        dc: HDC,
        text: &str,
        bounds: windows::Win32::Foundation::RECT,
        scale: f32,
        color: [u8; 3],
    ) -> Result<()> {
        let width = (bounds.right - bounds.left).max(1) as f32 / scale;
        let (_, _, image, _) = self.raster(text, 24., width, scale)?;
        let w = image.size[0];
        let h = image.size[1];
        let mut bgra = Vec::with_capacity(w * h * 4);
        for pixel in image.pixels {
            let alpha = pixel.a() as u32;
            let mut rgb = [0; 3];
            for (i, background) in [10, 17, 22].into_iter().enumerate() {
                rgb[i] = ((color[i] as u32 * alpha + background * (255 - alpha) + 127) / 255) as u8;
            }
            bgra.extend_from_slice(&[rgb[2], rgb[1], rgb[0], 255]);
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                biHeight: -(h as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        unsafe {
            let saved = SaveDC(dc);
            if saved == 0 {
                return Err(E_FAIL.into());
            }
            IntersectClipRect(dc, bounds.left, bounds.top, bounds.right, bounds.bottom);
            let rows = SetDIBitsToDevice(
                dc,
                bounds.left - (PAD * scale) as i32,
                bounds.top - (PAD * scale) as i32,
                w as u32,
                h as u32,
                0,
                0,
                0,
                h as u32,
                bgra.as_ptr().cast(),
                &info,
                DIB_RGB_COLORS,
            );
            let _ = RestoreDC(dc, saved);
            if rows == 0 {
                return Err(E_FAIL.into());
            }
        }
        Ok(())
    }

    fn layout(
        &self,
        text: &str,
        size: f32,
        width: f32,
    ) -> Result<(IDWriteTextLayout, DWRITE_TEXT_METRICS)> {
        let text16: Vec<u16> = text.encode_utf16().collect();
        if text16.len() > 16_384 || !size.is_finite() || size <= 0. {
            return Err(E_FAIL.into());
        }
        unsafe {
            let format = self.factory.CreateTextFormat(
                windows::core::w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                windows::core::w!(""),
            )?;
            if first_strong_rtl(text) {
                format.SetReadingDirection(DWRITE_READING_DIRECTION_RIGHT_TO_LEFT)?;
            }
            format.SetWordWrapping(if width.is_finite() {
                DWRITE_WORD_WRAPPING_WRAP
            } else {
                DWRITE_WORD_WRAPPING_NO_WRAP
            })?;
            let line = self.factory.CreateTextLayout(
                &text16,
                &format,
                width.clamp(1., 16_384.),
                16_384.,
            )?;
            let mut metrics = DWRITE_TEXT_METRICS::default();
            line.GetMetrics(&mut metrics)?;
            Ok((line, metrics))
        }
    }

    fn target(
        &self,
        layout: &IDWriteTextLayout,
        width: u32,
        height: u32,
        scale: f32,
        x: f32,
    ) -> Result<(IDWriteBitmapRenderTarget, u32)> {
        unsafe {
            let target = self
                .factory
                .GetGdiInterop()?
                .CreateBitmapRenderTarget(None, width, height)?;
            target.SetPixelsPerDip(scale)?;
            // A grayscale mask avoids ClearType color fringes on translucent artwork.
            target
                .cast::<IDWriteBitmapRenderTarget1>()?
                .SetTextAntialiasMode(DWRITE_TEXT_ANTIALIAS_MODE_GRAYSCALE)?;
            let dc = target.GetMemoryDC();
            PatBlt(dc, 0, 0, width as i32, height as i32, BLACKNESS).ok()?;
            let missing = Arc::new(AtomicU32::new(0));
            let renderer: IDWriteTextRenderer = Renderer {
                target: target.clone(),
                params: self.params.clone(),
                missing: missing.clone(),
            }
            .into();
            layout.Draw(None, &renderer, PAD - x, PAD)?;
            Ok((target, missing.load(Ordering::Relaxed)))
        }
    }

    fn raster(
        &self,
        text: &str,
        size: f32,
        width: f32,
        scale: f32,
    ) -> Result<(
        IDWriteTextLayout,
        DWRITE_TEXT_METRICS,
        egui::ColorImage,
        u32,
    )> {
        let (layout, metrics) = self.layout(text, size, width)?;
        let scale = scale.clamp(0.5, 4.);
        let w =
            ((metrics.widthIncludingTrailingWhitespace.max(1.) + PAD * 2.) * scale).ceil() as usize;
        let h = ((metrics.height.max(1.) + PAD * 2.) * scale).ceil() as usize;
        if w > 16_384 || h > 4096 || w.saturating_mul(h) > BUDGET / 4 {
            return Err(E_FAIL.into());
        }
        let (target, missing) = self.target(&layout, w as u32, h as u32, scale, metrics.left)?;
        let mut dib = DIBSECTION::default();
        unsafe {
            let dc = target.GetMemoryDC();
            if GetObjectW(
                GetCurrentObject(dc, OBJ_BITMAP),
                std::mem::size_of::<DIBSECTION>() as i32,
                Some((&mut dib as *mut DIBSECTION).cast()),
            ) != std::mem::size_of::<DIBSECTION>() as i32
                || dib.dsBm.bmBits.is_null()
                || dib.dsBm.bmBitsPixel != 32
                || dib.dsBm.bmWidth != w as i32
                || dib.dsBm.bmHeight.unsigned_abs() != h as u32
                || dib.dsBm.bmWidthBytes < (w * 4) as i32
            {
                return Err(E_FAIL.into());
            }
            let stride = dib.dsBm.bmWidthBytes as usize;
            let bytes = std::slice::from_raw_parts(dib.dsBm.bmBits.cast::<u8>(), stride * h);
            let mut pixels = Vec::with_capacity(w * h);
            for y in 0..h {
                // GetMemoryDC guarantees a top-down DIB. GetObject can return a
                // positive header height here; using that sign flips every glyph.
                for px in bytes[y * stride..y * stride + w * 4].as_chunks::<4>().0 {
                    let alpha = px[0].max(px[1]).max(px[2]);
                    pixels.push(Color32::from_white_alpha(alpha));
                }
            }
            Ok((
                layout,
                metrics,
                egui::ColorImage::new([w, h], pixels),
                missing,
            ))
        }
    }
}

/// DirectWrite supplies positioned, fallback-resolved glyph runs. Its bitmap
/// target owns and releases the memory DC/DIB, so no selected GDI objects escape.
#[implement(IDWriteTextRenderer)]
struct Renderer {
    target: IDWriteBitmapRenderTarget,
    params: IDWriteRenderingParams,
    missing: Arc<AtomicU32>,
}

#[allow(non_snake_case)]
impl IDWritePixelSnapping_Impl for Renderer_Impl {
    fn IsPixelSnappingDisabled(&self, _: *const std::ffi::c_void) -> Result<BOOL> {
        Ok(false.into())
    }
    fn GetCurrentTransform(
        &self,
        _: *const std::ffi::c_void,
        transform: *mut DWRITE_MATRIX,
    ) -> Result<()> {
        unsafe { self.target.GetCurrentTransform(transform) }
    }
    fn GetPixelsPerDip(&self, _: *const std::ffi::c_void) -> Result<f32> {
        Ok(unsafe { self.target.GetPixelsPerDip() })
    }
}

#[allow(non_snake_case)]
impl IDWriteTextRenderer_Impl for Renderer_Impl {
    fn DrawGlyphRun(
        &self,
        _: *const std::ffi::c_void,
        x: f32,
        y: f32,
        mode: DWRITE_MEASURING_MODE,
        run: *const DWRITE_GLYPH_RUN,
        _: *const DWRITE_GLYPH_RUN_DESCRIPTION,
        _: Ref<IUnknown>,
    ) -> Result<()> {
        unsafe {
            if !run.is_null() && (*run).glyphCount > 0 && !(*run).glyphIndices.is_null() {
                let glyphs =
                    std::slice::from_raw_parts((*run).glyphIndices, (*run).glyphCount as usize);
                self.missing.fetch_add(
                    glyphs.iter().filter(|&&id| id == 0).count() as u32,
                    Ordering::Relaxed,
                );
            }
            self.target
                .DrawGlyphRun(x, y, mode, run, &self.params, COLORREF(0xffffff), None)
        }
    }
    fn DrawUnderline(
        &self,
        _: *const std::ffi::c_void,
        _: f32,
        _: f32,
        _: *const DWRITE_UNDERLINE,
        _: Ref<IUnknown>,
    ) -> Result<()> {
        Ok(())
    }
    fn DrawStrikethrough(
        &self,
        _: *const std::ffi::c_void,
        _: f32,
        _: f32,
        _: *const DWRITE_STRIKETHROUGH,
        _: Ref<IUnknown>,
    ) -> Result<()> {
        Ok(())
    }
    fn DrawInlineObject(
        &self,
        _: *const std::ffi::c_void,
        _: f32,
        _: f32,
        _: Ref<IDWriteInlineObject>,
        _: BOOL,
        _: BOOL,
        _: Ref<IUnknown>,
    ) -> Result<()> {
        Ok(())
    }
}

type FragmentCache = Mutex<HashMap<(usize, usize), Arc<[(Rect, bool)]>>>;

pub struct Line {
    pub size: Vec2,
    text: String,
    layout: IDWriteTextLayout,
    left: f32,
    texture: TextureHandle,
    scale: f32,
    hits: FragmentCache,
}

impl Line {
    pub fn paint(&self, painter: &egui::Painter, origin: Pos2, color: Color32) {
        painter.image(
            self.texture.id(),
            Rect::from_min_size(
                origin - vec2(PAD, PAD),
                self.texture.size_vec2() / self.scale,
            ),
            Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
            color,
        );
    }
    pub fn fragments(&self, start: usize, end: usize) -> Arc<[(Rect, bool)]> {
        if let Ok(hits) = self.hits.lock()
            && let Some(cached) = hits.get(&(start, end))
        {
            return cached.clone();
        }
        let fragments: Arc<[(Rect, bool)]> = self.resolve_fragments(start, end).into();
        if let Ok(mut hits) = self.hits.lock()
            && hits.len() < 512
        {
            hits.insert((start, end), fragments.clone());
        }
        fragments
    }
    fn resolve_fragments(&self, start: usize, end: usize) -> Vec<(Rect, bool)> {
        let Some((prefix, range)) = self.text.get(..start).zip(self.text.get(start..end)) else {
            return Vec::new();
        };
        let offset = prefix.encode_utf16().count() as u32;
        let length = range.encode_utf16().count() as u32;
        if length == 0 {
            return Vec::new();
        }
        unsafe {
            let mut count = 0;
            // The initial call reports required capacity with E_NOT_SUFFICIENT_BUFFER.
            let _ = self
                .layout
                .HitTestTextRange(offset, length, -self.left, 0., None, &mut count);
            if count == 0 || count > 4096 {
                return Vec::new();
            }
            let mut hits = vec![DWRITE_HIT_TEST_METRICS::default(); count as usize];
            if self
                .layout
                .HitTestTextRange(offset, length, -self.left, 0., Some(&mut hits), &mut count)
                .is_err()
            {
                return Vec::new();
            }
            hits.into_iter()
                .take(count as usize)
                .filter(|h| h.isText.as_bool())
                .map(|h| {
                    (
                        Rect::from_min_size(pos2(h.left, h.top), vec2(h.width, h.height)),
                        h.bidiLevel % 2 == 1,
                    )
                })
                .collect()
        }
    }
}

#[derive(Hash, PartialEq, Eq, Clone)]
struct Key {
    text: String,
    size: u32,
    width: u32,
    scale: u32,
}
#[derive(Default)]
struct Cache {
    engine: Option<Engine>,
    failed: bool,
    lines: HashMap<Key, (Arc<Line>, u64)>,
    clock: u64,
    bytes: usize,
}
impl Cache {
    fn engine(&mut self) -> Option<&Engine> {
        if self.engine.is_none() && !self.failed {
            self.engine = Engine::new().ok();
            self.failed = self.engine.is_none();
        }
        self.engine.as_ref()
    }
    fn evict(&mut self, incoming: usize) {
        while !self.lines.is_empty()
            && (self.bytes + incoming > BUDGET || self.lines.len() >= CAPACITY)
        {
            let key = self
                .lines
                .iter()
                .min_by_key(|(_, (_, age))| age)
                .map(|(k, _)| k.clone())
                .unwrap();
            if let Some((line, _)) = self.lines.remove(&key) {
                self.bytes = self
                    .bytes
                    .saturating_sub(line.texture.size()[0] * line.texture.size()[1] * 4);
            }
        }
    }
}
fn cache(ctx: &egui::Context) -> Arc<Mutex<Cache>> {
    ctx.data_mut(|data| {
        data.get_temp_mut_or_default::<Arc<Mutex<Cache>>>(egui::Id::new("lyric-script-fonts"))
            .clone()
    })
}
pub fn measure(ctx: &egui::Context, text: &str, size: f32, width: f32) -> Option<Vec2> {
    let cache = cache(ctx);
    let mut cache = cache.lock().ok()?;
    let (_, metrics) = cache.engine()?.layout(text, size, width).ok()?;
    Some(vec2(
        metrics.widthIncludingTrailingWhitespace.max(1.),
        metrics.height,
    ))
}
pub fn cached(ctx: &egui::Context, text: &str, size: f32, width: f32) -> Option<Arc<Line>> {
    let scale = ctx.pixels_per_point().clamp(0.5, 4.);
    let key = Key {
        text: text.into(),
        size: size.to_bits(),
        width: width.to_bits(),
        scale: scale.to_bits(),
    };
    let shared = cache(ctx);
    let mut cache = shared.lock().ok()?;
    cache.clock += 1;
    let now = cache.clock;
    if let Some((line, touched)) = cache.lines.get_mut(&key) {
        *touched = now;
        return Some(line.clone());
    }
    let (layout, metrics, image, _) = cache.engine()?.raster(text, size, width, scale).ok()?;
    let bytes = image.pixels.len() * 4;
    cache.evict(bytes);
    let line = Arc::new(Line {
        size: vec2(
            metrics.widthIncludingTrailingWhitespace.max(1.),
            metrics.height,
        ),
        text: text.into(),
        layout,
        left: metrics.left,
        texture: ctx.load_texture("lyric-script", image, egui::TextureOptions::LINEAR),
        scale,
        hits: Mutex::default(),
    });
    cache.lines.insert(key, (line.clone(), now));
    cache.bytes += bytes;
    Some(line)
}

fn first_strong_rtl(text: &str) -> bool {
    for c in text.chars() {
        if c == '\u{200e}' {
            return false;
        }
        if c == '\u{200f}' || c == '\u{061c}' {
            return true;
        }
        if matches!(c as u32, 0x0590..=0x08ff | 0xfb1d..=0xfdff | 0xfe70..=0xfefc | 0x10800..=0x10fff | 0x1e800..=0x1eeff)
            && c.is_alphabetic()
        {
            return true;
        }
        if c.is_alphabetic() {
            return false;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn directwrite_mask_is_upright_and_keeps_transparent_edges() {
        let engine = Engine::new().unwrap();
        let (_, _, mask, missing) = engine.raster("T", 64., 200., 1.).unwrap();
        assert_eq!(missing, 0);
        let widths: Vec<usize> = mask
            .pixels
            .chunks(mask.size[0])
            .map(|row| row.iter().filter(|p| p.a() > 128).count())
            .filter(|&w| w > 0)
            .collect();
        assert!(widths.len() > 20);
        assert!(
            widths[..3].iter().sum::<usize>()
                > widths[widths.len() - 3..].iter().sum::<usize>() * 2
        );
        assert!(mask.pixels[0].a() == 0);
    }
    #[test]
    fn native_layout_renders_installed_scripts_and_wraps_at_dpi() {
        let engine = Engine::new().unwrap();
        for text in [
            "ਪੰਜਾਬੀ ਵਿੱਚ ਪਿਆਰ",
            "हिन्दी में प्यार",
            "বাংলা ভালোবাসা",
            "தமிழ் பாடல்",
            "తెలుగు పాట",
            "ગુજરાતી ગીત",
            "മലയാളം പാട്ട്",
            "ಕನ್ನಡ ಹಾಡು",
            "සිංහල ගීතය",
            "العربية لغة جميلة",
            "اردو میں محبت",
            "עברית שיר",
            "ภาษาไทย",
            "ພາສາລາວ",
            "မြန်မာစာ",
            "ភាសាខ្មែរ",
            "日本語の歌",
            "한국어 노래",
            "한글",
            "中文歌曲",
            "Հայերեն երգ",
            "ქართული სიმღერა",
            "አማርኛ",
            "e\u{301}",
        ] {
            let (_, metrics, image, missing) = engine.raster(text, 38., 500., 1.5).unwrap();
            assert_eq!(missing, 0, "Missing glyphs in {text}");
            assert!(metrics.height > 20. && metrics.width > 0., "{text}");
            assert!(image.pixels.iter().any(|p| p.a() > 0), "{text}");
        }
        let (_, wide) = engine
            .layout("ਪੰਜਾਬੀ ਵਿੱਚ ਪਿਆਰ ਪੰਜਾਬੀ ਵਿੱਚ ਪਿਆਰ", 38., 900.)
            .unwrap();
        let (_, narrow) = engine
            .layout("ਪੰਜਾਬੀ ਵਿੱਚ ਪਿਆਰ ਪੰਜਾਬੀ ਵਿੱਚ ਪਿਆਰ", 38., 160.)
            .unwrap();
        assert!(narrow.lineCount > wide.lineCount);
        assert!(first_strong_rtl("123 العربية English"));
        assert!(!first_strong_rtl("English العربية"));
        assert!(!first_strong_rtl("\u{200e}العربية"));
        assert!(first_strong_rtl("\u{200f}English"));
    }
    #[test]
    fn native_cache_reuses_masks_and_keeps_a_fixed_budget() {
        let ctx = egui::Context::default();
        let mut first = None;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let a = cached(ctx, "ਪੰਜਾਬੀ", 38., 500.).unwrap();
            let b = cached(ctx, "ਪੰਜਾਬੀ", 38., 500.).unwrap();
            assert!(Arc::ptr_eq(&a, &b));
            assert!(!a.fragments(0, "ਪੰਜਾਬੀ".len()).is_empty());
            assert!(Arc::ptr_eq(
                &a.fragments(0, "ਪੰਜਾਬੀ".len()),
                &a.fragments(0, "ਪੰਜਾਬੀ".len())
            ));
            first = Some(a);
            for n in 0..80 {
                cached(ctx, &format!("ਪੰਜਾਬੀ {n}"), 38., 500.).unwrap();
            }
            let shared = cache(ctx);
            let cache = shared.lock().unwrap();
            assert!(cache.bytes <= BUDGET && cache.lines.len() <= CAPACITY);
        });
        // External visible-line handles survive an LRU eviction without dangling textures.
        assert!(first.unwrap().texture.size()[0] > 0);
    }
    #[test]
    fn bidi_word_fragments_use_utf16_positions_and_visual_direction() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let text = "مرحبا بالعالم";
            let line = cached(ctx, text, 38., 500.).unwrap();
            let first = line.fragments(0, "مرحبا".len());
            let second = line.fragments("مرحبا ".len(), text.len());
            assert!(!first.is_empty() && !second.is_empty());
            assert!(first.iter().all(|(_, rtl)| *rtl));
            assert!(first[0].0.left() > second[0].0.left());
            let mixed = "🌊 ਪੰਜਾਬੀ";
            let line = cached(ctx, mixed, 38., 500.).unwrap();
            let hits = line.fragments("🌊 ".len(), mixed.len());
            assert!(!hits.is_empty() && hits[0].0.width() > 20.);
            assert!(!hits[0].1);
            assert!(line.fragments(1, 2).is_empty());
            assert!(line.fragments(0, 1000).is_empty());
        });
    }
    #[test]
    fn optional_script_contact_sheet() {
        let Ok(path) = std::env::var("UNDERTOW_FONT_PROOF") else {
            return;
        };
        let engine = Engine::new().unwrap();
        let mut sheet = image::RgbaImage::from_pixel(1600, 1440, image::Rgba([12, 14, 19, 255]));
        for (index, text) in [
            "Punjabi / ਪੰਜਾਬੀ ਵਿੱਚ ਪਿਆਰ",
            "Hindi / हिन्दी में प्यार",
            "Bengali / বাংলা ভালোবাসা",
            "Tamil / தமிழ் பாடல்",
            "Telugu / తెలుగు పాట",
            "Gujarati / ગુજરાતી ગીત",
            "Malayalam / മലയാളം പാട്ട്",
            "Kannada / ಕನ್ನಡ ಹಾಡು",
            "Sinhala / සිංහල ගීතය",
            "Arabic / العربية لغة جميلة",
            "Urdu / اردو میں محبت",
            "Hebrew / עברית שיר",
            "Thai / ภาษาไทย",
            "Lao / ພາສາລາວ",
            "Myanmar / မြန်မာစာ",
            "Khmer / ភាសាខ្មែរ",
            "Japanese / 日本語の歌",
            "Korean / 한국어 노래",
            "Chinese / 中文歌曲",
            "Armenian / Հայերեն երգ",
            "Georgian / ქართული სიმღერა",
            "Amharic / አማርኛ",
            "Combining marks / cafe\u{301}",
            "Mixed RTL / 123 مرحبا بالعالم 🌊",
        ]
        .iter()
        .enumerate()
        {
            let (_, _, mask, missing) = engine.raster(text, 34., 730., 1.).unwrap();
            assert_eq!(missing, 0, "{text}");
            let rgba: Vec<u8> = mask
                .pixels
                .iter()
                .flat_map(|p| p.to_srgba_unmultiplied())
                .collect();
            let image =
                image::RgbaImage::from_raw(mask.size[0] as u32, mask.size[1] as u32, rgba).unwrap();
            image::imageops::overlay(
                &mut sheet,
                &image,
                24 + (index as i64 % 2) * 800,
                20 + (index as i64 / 2) * 118,
            );
        }
        sheet.save(path).unwrap();
    }
}
