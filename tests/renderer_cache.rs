use eframe::egui::{self, Color32, Mesh, Rect, TextureId, TextureOptions, pos2, vec2};
use egui_software_backend::{BufferMutRef, ColorFieldOrder, EguiSoftwareRender};

fn texture(id: TextureId, color: Color32) -> egui::TexturesDelta {
    egui::TexturesDelta {
        set: vec![(
            id,
            egui::epaint::ImageDelta::full(
                egui::ColorImage::new([1, 1], vec![color]),
                TextureOptions::LINEAR,
            ),
        )],
        ..Default::default()
    }
}
fn rect(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: Color32,
    id: TextureId,
) -> egui::ClippedPrimitive {
    let mut mesh = Mesh::with_texture(id);
    mesh.add_colored_rect(Rect::from_min_size(pos2(x, y), vec2(width, height)), color);
    egui::ClippedPrimitive {
        clip_rect: Rect::from_min_max(pos2(0., 0.), pos2(160., 160.)),
        primitive: egui::epaint::Primitive::Mesh(mesh),
    }
}
fn replacement(
    renderer: &mut EguiSoftwareRender,
    size: usize,
    jobs: &[egui::ClippedPrimitive],
    delta: &egui::TexturesDelta,
) -> Vec<[u8; 4]> {
    let mut pixels = vec![[88; 4]; size * size];
    renderer.render_to_canvas(size, size, jobs, delta, 1.);
    renderer.copy_canvas_to_buffer(&mut BufferMutRef::new(&mut pixels, size, size));
    pixels
}
fn fresh(
    size: usize,
    jobs: &[egui::ClippedPrimitive],
    delta: &egui::TexturesDelta,
) -> Vec<[u8; 4]> {
    let mut renderer = EguiSoftwareRender::new(ColorFieldOrder::Rgba);
    let mut pixels = vec![[0; 4]; size * size];
    renderer.render(
        &mut BufferMutRef::new(&mut pixels, size, size),
        jobs,
        delta,
        1.,
    );
    pixels
}
#[test]
fn replacement_matches_cleared_surface_blending_including_transparency() {
    let delta = texture(TextureId::Managed(0), Color32::WHITE);
    let jobs = vec![
        rect(
            5.,
            6.,
            120.,
            115.,
            Color32::from_rgba_premultiplied(15, 40, 60, 100),
            TextureId::Managed(0),
        ),
        rect(
            50.,
            45.,
            100.,
            100.,
            Color32::from_rgba_premultiplied(25, 5, 10, 0),
            TextureId::Managed(0),
        ),
    ];
    let mut renderer = EguiSoftwareRender::new(ColorFieldOrder::Rgba);
    assert_eq!(
        replacement(&mut renderer, 160, &jobs, &delta),
        fresh(160, &jobs, &delta)
    );
    assert_eq!(
        replacement(&mut renderer, 160, &jobs, &Default::default()),
        fresh(160, &jobs, &delta)
    );
}
#[test]
fn cached_frames_handle_motion_clip_reordering_removal_and_resize() {
    let delta = texture(TextureId::Managed(0), Color32::WHITE);
    let red = rect(3., 3., 100., 100., Color32::RED, TextureId::Managed(0));
    let blue = rect(
        32.,
        40.,
        100.,
        100.,
        Color32::from_rgba_unmultiplied(0, 0, 255, 160),
        TextureId::Managed(0),
    );
    let mut renderer = EguiSoftwareRender::new(ColorFieldOrder::Rgba);
    let mut clipped = blue.clone();
    clipped.clip_rect = Rect::from_min_max(pos2(50., 50.), pos2(95., 110.));
    let frames = [
        vec![red.clone(), blue.clone()],
        vec![blue, red.clone()],
        vec![red.clone(), clipped],
        vec![red],
        vec![],
        vec![rect(
            15.25,
            30.5,
            110.,
            80.,
            Color32::GREEN,
            TextureId::Managed(0),
        )],
    ];
    for (i, jobs) in frames.iter().enumerate() {
        let updates = if i == 0 {
            delta.clone()
        } else {
            Default::default()
        };
        assert_eq!(
            replacement(&mut renderer, 160, jobs, &updates),
            fresh(160, jobs, &delta),
            "frame {i}"
        );
    }
    assert_eq!(
        replacement(&mut renderer, 128, &frames[5], &Default::default()),
        fresh(128, &frames[5], &delta)
    );
}
#[test]
fn retained_surface_copies_only_changes_and_matches_a_full_copy() {
    let id = TextureId::Managed(0);
    let delta = texture(id, Color32::WHITE);
    let mut renderer = EguiSoftwareRender::new(ColorFieldOrder::Bgra);
    let mut size = 157;
    let mut pixels = vec![[88; 4]; size * size];
    let background = rect(0., 0., 157., 157., Color32::DARK_GRAY, id);
    let first = vec![
        background.clone(),
        rect(120., 142., 30., 15., Color32::RED, id),
    ];
    let moved = vec![
        background.clone(),
        rect(138., 146., 19., 11., Color32::GREEN, id),
    ];
    let empty = egui::TexturesDelta::default();
    for (jobs, updates, full) in [
        (&first, &delta, true),
        (&first, &empty, false),
        (&moved, &empty, false),
        (&vec![background], &empty, false),
    ] {
        renderer.render_to_canvas(size, size, jobs, updates, 1.);
        let copied =
            renderer.copy_dirty_canvas_to_buffer(&mut BufferMutRef::new(&mut pixels, size, size));
        let mut expected = vec![[0; 4]; size * size];
        renderer.copy_canvas_to_buffer(&mut BufferMutRef::new(&mut expected, size, size));
        assert_eq!(pixels, expected);
        if full {
            assert_eq!(copied, size * size);
        } else if std::ptr::eq(jobs, &first) {
            assert_eq!(copied, 0);
        } else {
            assert!(copied > 0 && copied < size * size);
        }
    }
    size = 135;
    pixels.resize(size * size, [88; 4]);
    renderer.render_to_canvas(size, size, &moved, &empty, 1.);
    let copied =
        renderer.copy_dirty_canvas_to_buffer(&mut BufferMutRef::new(&mut pixels, size, size));
    assert_eq!(copied, size * size);
}
#[test]
fn unchanged_geometry_invalidates_when_texture_pixels_change() {
    let id = TextureId::Managed(0);
    let jobs = vec![rect(0., 0., 140., 140., Color32::WHITE, id)];
    let mut renderer = EguiSoftwareRender::new(ColorFieldOrder::Bgra);
    let before = replacement(&mut renderer, 160, &jobs, &texture(id, Color32::RED));
    let partial = egui::TexturesDelta {
        set: vec![(
            id,
            egui::epaint::ImageDelta::partial(
                [0, 0],
                egui::ColorImage::new([1, 1], vec![Color32::BLUE]),
                TextureOptions::LINEAR,
            ),
        )],
        ..Default::default()
    };
    let after = replacement(&mut renderer, 160, &jobs, &partial);
    assert!(before != after, "Texture update left stale pixels");
    assert_eq!(after[20 * 160 + 20], [255, 0, 0, 255]);
}

#[test]
fn transposed_geometry_cannot_alias_a_cached_mesh() {
    let delta = texture(TextureId::Managed(0), Color32::WHITE);
    let original = rect(5., 20., 25., 70., Color32::RED, TextureId::Managed(0));
    let mut transposed = original.clone();
    if let egui::epaint::Primitive::Mesh(mesh) = &mut transposed.primitive {
        for vertex in &mut mesh.vertices {
            vertex.pos = pos2(vertex.pos.y, vertex.pos.x);
        }
    }
    let mut renderer = EguiSoftwareRender::new(ColorFieldOrder::Rgba);
    let before = replacement(&mut renderer, 160, &[original], &delta);
    let after = replacement(
        &mut renderer,
        160,
        &[transposed.clone()],
        &Default::default(),
    );
    assert!(before != after, "Transposition reused stale geometry");
    assert!(
        after == fresh(160, &[transposed], &delta),
        "Transposed cache differs from a fresh render"
    );
}

#[test]
fn solid_background_cache_uses_no_raster_bitmap_and_matches_direct_pixels() {
    let delta = texture(TextureId::Managed(0), Color32::WHITE);
    let jobs = vec![
        rect(0., 0., 160., 160., Color32::BLACK, TextureId::Managed(0)),
        rect(
            12.,
            17.,
            110.,
            95.,
            Color32::from_rgba_premultiplied(20, 50, 80, 120),
            TextureId::Managed(0),
        ),
    ];
    let mut cached = EguiSoftwareRender::new(ColorFieldOrder::Rgba);
    let pixels = replacement(&mut cached, 160, &jobs, &delta);
    let mut direct = EguiSoftwareRender::new(ColorFieldOrder::Rgba).with_caching(false);
    let mut expected = vec![[0; 4]; 160 * 160];
    direct.render(
        &mut BufferMutRef::new(&mut expected, 160, 160),
        &jobs,
        &delta,
        1.,
    );
    assert!(
        pixels == expected,
        "Compact flat backgrounds changed pixel output"
    );
    assert_eq!(
        cached.retained_bytes()[1],
        0,
        "Flat fills retained a raster bitmap"
    );
}
