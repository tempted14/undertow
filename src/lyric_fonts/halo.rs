use super::*;
use egui::{ColorImage, Id, TextureHandle, TextureOptions, pos2, vec2};
use image::GrayImage;
use std::collections::VecDeque;

const SIGMA: f32 = 3.4;

fn cache_id() -> Id {
    Id::new("lyric-halo-cache")
}

pub(super) fn clear(ctx: &egui::Context) {
    drop(ctx.data_mut(|data| data.remove_temp::<VecDeque<(Layout, Halo)>>(cache_id())));
}

#[derive(Clone)]
struct Halo {
    texture: TextureHandle,
    offset: Vec2,
    scale: f32,
}

struct Mask {
    alpha: GrayImage,
    offset: Vec2,
    scale: f32,
}

pub(super) fn paint(layout: &Layout, painter: &egui::Painter, origin: Pos2, color: Color32) {
    if color.a() == 0 {
        return;
    }
    let ctx = painter.ctx();
    let key = cache_id();
    let mut cache = ctx.data_mut(|data| {
        data.remove_temp::<VecDeque<(Layout, Halo)>>(key)
            .unwrap_or_default()
    });
    let same_layout = |other: &Layout| match (layout, other) {
        (Layout::Atlas(a), Layout::Atlas(b)) => Arc::ptr_eq(a, b),
        #[cfg(windows)]
        (Layout::Native(a), Layout::Native(b)) => Arc::ptr_eq(a, b),
        #[cfg(windows)]
        _ => false,
    };
    let mut entry = cache
        .iter()
        .position(|(other, _)| same_layout(other))
        .and_then(|i| cache.remove(i));
    if entry.is_none() {
        entry = build(layout, ctx).map(|halo| (layout.clone(), halo));
    }
    if let Some(entry) = entry {
        let halo = &entry.1;
        let rect = Rect::from_min_size(origin + halo.offset, halo.texture.size_vec2() / halo.scale);
        painter.image(
            halo.texture.id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
            color,
        );
        cache.push_front(entry);
        cache.truncate(3);
    }
    ctx.data_mut(|data| data.insert_temp(key, cache));
}

fn build(layout: &Layout, ctx: &egui::Context) -> Option<Halo> {
    let mask = match layout {
        Layout::Atlas(galley) => atlas_mask(galley, ctx)?,
        #[cfg(windows)]
        Layout::Native(line) => {
            let (alpha, scale) = line.halo_mask();
            let pad = (SIGMA * scale * 3.).ceil() as u32;
            if !fits(alpha.width() + 2 * pad, alpha.height() + 2 * pad) {
                return None;
            }
            let mut padded = GrayImage::new(alpha.width() + 2 * pad, alpha.height() + 2 * pad);
            image::imageops::replace(&mut padded, alpha, i64::from(pad), i64::from(pad));
            Mask {
                alpha: padded,
                offset: line.texture_offset() - Vec2::splat(pad as f32 / scale),
                scale,
            }
        }
    };
    let blurred = image::imageops::fast_blur(&mask.alpha, SIGMA * mask.scale);
    let size = [blurred.width() as usize, blurred.height() as usize];
    let image = ColorImage::new(
        size,
        blurred
            .into_raw()
            .into_iter()
            .map(Color32::from_white_alpha)
            .collect(),
    );
    Some(Halo {
        texture: ctx.load_texture("lyric-halo", image, TextureOptions::LINEAR),
        offset: mask.offset,
        scale: mask.scale,
    })
}

fn atlas_mask(galley: &egui::Galley, ctx: &egui::Context) -> Option<Mask> {
    let scale = galley.pixels_per_point;
    let bounds = galley.mesh_bounds;
    if !bounds.is_finite() || !bounds.is_positive() || scale != ctx.pixels_per_point() {
        return None;
    }
    let pad = (SIGMA * scale * 3.).ceil() as i32;
    let left = (bounds.left() * scale).floor() as i32 - pad;
    let top = (bounds.top() * scale).floor() as i32 - pad;
    let width = ((bounds.right() * scale).ceil() as i32 + pad - left) as u32;
    let height = ((bounds.bottom() * scale).ceil() as i32 + pad - top) as u32;
    if !fits(width, height) {
        return None;
    }
    let mut alpha = GrayImage::new(width, height);
    let atlas = ctx.fonts(|fonts| fonts.image());
    for row in &galley.rows {
        for glyph in &row.glyphs {
            let uv = glyph.uv_rect;
            if uv.is_nothing()
                || usize::from(uv.max[0]) > atlas.size[0]
                || usize::from(uv.max[1]) > atlas.size[1]
            {
                continue;
            }
            let pos = row.pos + glyph.pos.to_vec2() + uv.offset;
            let x = ((pos.x * scale).round() as i32 - left).max(0) as u32;
            let y = ((pos.y * scale).round() as i32 - top).max(0) as u32;
            let glyph_width = u32::from(uv.max[0] - uv.min[0]).min(width.saturating_sub(x));
            let glyph_height = u32::from(uv.max[1] - uv.min[1]).min(height.saturating_sub(y));
            for dy in 0..glyph_height {
                let start =
                    (usize::from(uv.min[1]) + dy as usize) * atlas.size[0] + usize::from(uv.min[0]);
                for dx in 0..glyph_width {
                    let pixel = alpha.get_pixel_mut(x + dx, y + dy);
                    pixel.0[0] = pixel.0[0].max(atlas.pixels[start + dx as usize].a());
                }
            }
        }
    }
    Some(Mask {
        alpha,
        offset: vec2(left as f32, top as f32) / scale,
        scale,
    })
}

fn fits(width: u32, height: u32) -> bool {
    u64::from(width) * u64::from(height) <= 1_000_000
}
