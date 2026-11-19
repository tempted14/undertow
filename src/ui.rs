use crate::{
    audio::Audio,
    config::{Settings, data_dir},
    lyrics::Lyrics,
    metrics::Metrics,
    model::{Album, Artist, CatalogItem, CreditGroup, Playlist, Track, clock},
    presence::Presence,
    worker::{Command, Event, Worker},
};
use eframe::egui::{
    self, Align, Align2, Color32, FontId, Frame, Id, Layout, Margin, Pos2, Rect, RichText, Sense,
    Stroke, TextureHandle, TextureOptions, Vec2, pos2, vec2,
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, atomic::Ordering, mpsc},
    time::{Duration, Instant},
};

mod design;
mod diagnostics;
mod lyric_jump;
mod reference;

const TEXT: Color32 = design::TEXT;
const MUTED: Color32 = design::MUTED;
const ACCENT: Color32 = Color32::from_rgb(35, 225, 203);
const BG: Color32 = design::CANVAS;
const CARD: Color32 = design::SURFACE;

#[derive(Clone, PartialEq)]
enum Page {
    Home,
    Discovery(String),
    Mix(String),
    Favorites,
    Playlist(String),
    Album(u64, String),
    Artist(u64),
    Search,
    Lyrics,
    Settings,
    Profile,
}
struct Artwork {
    texture: TextureHandle,
    blur: TextureHandle,
    palette: [u8; 3],
    touched: u64,
}
#[derive(Default)]
struct Dialog {
    kind: String,
    id: String,
    title: String,
    description: String,
    public: Option<bool>,
    cover: String,
    image: Option<Arc<Vec<u8>>>,
    preview: Option<TextureHandle>,
    revision: u64,
    picking_image: bool,
    saving: bool,
    problem: String,
}
#[derive(Default)]
struct ArtworkScrollCheck {
    started: Option<Instant>,
    step: usize,
    records: Vec<serde_json::Value>,
}
struct PresenceSnapshot {
    settings: Settings,
    track: Option<Track>,
    position: f64,
    duration: f64,
    paused: bool,
}

#[derive(Default)]
struct LyricRequest {
    revision: u64,
    pending: bool,
}
impl LyricRequest {
    fn begin(&mut self) -> u64 {
        self.revision = self.revision.wrapping_add(1);
        self.pending = true;
        self.revision
    }
    fn finish(&mut self, revision: u64) -> bool {
        if revision != self.revision {
            return false;
        }
        self.pending = false;
        true
    }
}

pub struct App {
    settings: Settings,
    worker: Worker,
    page: Page,
    playlists: Arc<Vec<Playlist>>,
    tracks: HashMap<String, Arc<Vec<Track>>>,
    favorite_ids: HashSet<u64>,
    queue: Arc<Vec<Track>>,
    index: usize,
    current: Option<Track>,
    audio: Option<Audio>,
    completed_audio_seconds: f64,
    completed_underruns: u64,
    generation: u64,
    lyrics: Arc<Lyrics>,
    lyrics_status: String,
    lyric_request: LyricRequest,
    lyrics_chrome_hidden: bool,
    translated: bool,
    query: String,
    local_path: String,
    account: String,
    connected: bool,
    busy: bool,
    notice: String,
    notice_at: Instant,
    error: bool,
    artwork: HashMap<String, Artwork>,
    requested: HashSet<String>,
    art_requests: HashMap<String, u8>,
    art_visible: HashSet<String>,
    art_scroll_check: Option<ArtworkScrollCheck>,
    frame: u64,
    dialog: Option<Dialog>,
    download_active: bool,
    metrics: Metrics,
    presence_tx: mpsc::SyncSender<PresenceSnapshot>,
    presence_rx: mpsc::Receiver<String>,
    presence_status: String,
    last_presence: Instant,
    last_sync: Instant,
    font_loaded: String,
    show_details: bool,
    benchmark: Option<f64>,
    report: PathBuf,
    screenshot: Option<PathBuf>,
    screenshot_requested: bool,
    screenshot_number: usize,
    record_live: bool,
    last_record: Instant,
    overlay_window: Option<crate::overlay::Handle>,
    overlay_tick: Instant,
    muted: bool,
    autoplay_favorites: bool,
    signing_in: bool,
    device_prompt: Option<crate::auth::DevicePrompt>,
    manual_prompt: Option<crate::auth::ManualPrompt>,
    manual_return: String,
    login_check: Instant,
    login_progress: String,
    back_history: Vec<Page>,
    forward_history: Vec<Page>,
    history_page: Page,
    discovery: reference::discovery::State,
    repeat_mode: u8,
    shuffle_on: bool,
    lyric_offsets: Vec<f32>,
    lyric_galleys: HashMap<usize, crate::lyric_fonts::Layout>,
    lyric_word_galleys: Option<(usize, Vec<crate::lyric_fonts::Layout>)>,
    lyric_layout_key: Option<(u64, u32, u32, bool, usize, u32)>,
    lyric_scroll: f32,
    lyric_manual_scroll: bool,
    lyric_velocity: f32,
    lyric_frame_motion: bool,
    lyric_follow_target: f32,
    lyric_jump: Option<lyric_jump::Jump>,
    lyric_jump_trace: Vec<(f64, f64, f32, bool, u64)>,
    animation_clock: Instant,
    playing_playlist: Option<String>,
    startup_started: Option<Instant>,
    startup_ms: Vec<f64>,
    autoplay_album: bool,
    playlist_filter: String,
    playlist_filter_id: String,
    playlist_index: crate::collection::TrackIndex,
    playlist_scroll_reset: bool,
    playlist_filtered: Arc<Vec<usize>>,
    playlist_sort: Option<(usize, bool)>,
    startup_playlist: Option<String>,
    albums: crate::cache::AlbumCache,
    artists: HashMap<u64, Result<Arc<Artist>, String>>,
    artist_access: HashMap<u64, u64>,
    artist_tab: usize,
    artist_release_view: usize,
    artist_album_offset: f32,
    artist_single_offset: f32,
    artist_diagnostic_scroll: Option<f32>,
    artists_pending: HashSet<u64>,
    catalog: Arc<Vec<CatalogItem>>,
    catalog_view: crate::collection::CatalogIndex,
    catalog_query: String,
    search_sent: String,
    search_changed: Instant,
    search_preview: bool,
    search_anchor: Option<Rect>,
    search_tab: String,
    search_sort: usize,
    startup_album: Option<u64>,
    startup_artist: Option<u64>,
    startup_search: Option<String>,
    pending_list: Option<(String, reference::interactions::ListAction)>,
    saved_later: Arc<Vec<Track>>,
    show_saved_later: bool,
    queue_open: bool,
    queue_button_rect: Option<Rect>,
    play_history: Arc<Vec<Track>>,
    collection: HashSet<String>,
    collection_pending: HashSet<String>,
    motion_page: Page,
    motion_started: Instant,
    now_tab: usize,
    tab_started: Instant,
    credits: HashMap<u64, Result<Vec<CreditGroup>, String>>,
    credits_requested: HashSet<u64>,
    related_tracks: HashMap<u64, Result<Arc<Vec<Track>>, String>>,
    related_requested: HashSet<u64>,
    ui_motion_until: Instant,
    display_refresh_hz: f32,
    retained_render_bytes: [usize; 3],
    home_scroll: f32,
    home_content_height: f32,
    sidebar_expanded_width: f32,
    // Vibrant tint of the current cover, keyed by playback generation.
    accent: Option<(u64, Color32)>,
    // Vertex colors of the artwork-less backdrop, rebuilt only when the palette changes.
    backdrop_grid: Option<([u8; 3], Arc<Vec<Color32>>)>,
    settings_tab: usize,
    avatar: Option<TextureHandle>,
    volume_hover_deadline: Instant,
    volume_pinned: bool,
    volume_popup_rect: Option<Rect>,
    lyric_last_position: Option<f64>,
    lyric_entry_generation: u64,
    settings_dirty: bool,
    settings_changed_at: Instant,
}

// Match source resolution to physical pixels rather than downloading 320px art
// for every small row. Retina/high-DPI displays retain appropriately sharp art.
fn artwork_side(points: f32, pixels_per_point: f32) -> u32 {
    let pixels = points * pixels_per_point.max(1.);
    [80, 160, 320, 640]
        .into_iter()
        .find(|&side| pixels <= side as f32)
        .unwrap_or(640)
}

// Keep the service URL intact for disk/download quality, and pass the physical
// display size to the decoder even when TIDAL returns a fixed large cover URL.
fn artwork_key(track: &Track, side: u32) -> Option<String> {
    use std::hash::{Hash, Hasher};
    thread_local! {
        static MEMO: std::cell::RefCell<HashMap<u64, Option<String>>> = Default::default();
    }
    // Called for every visible cover on every frame; the key only depends on these fields,
    // so skip the string building and URL parsing after the first time.
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (
        &track.cover,
        track.id == 0,
        track.local_path.is_empty(),
        side,
    )
        .hash(&mut hasher);
    let slot = hasher.finish();
    MEMO.with(|memo| {
        let mut memo = memo.borrow_mut();
        if let Some(key) = memo.get(&slot) {
            return key.clone();
        }
        let key = artwork_key_uncached(track, side);
        if memo.len() >= 4096 {
            memo.clear();
        }
        memo.insert(slot, key.clone());
        key
    })
}
fn artwork_key_uncached(track: &Track, side: u32) -> Option<String> {
    let url = track.cover_url(if track.id == 0 { side.max(320) } else { side })?;
    if url.starts_with("local:") {
        return Some(format!("{url}#undertow-size={side}"));
    }
    if url.starts_with("https://") && side < 640 {
        let mut parsed = url::Url::parse(&url).ok()?;
        parsed.set_fragment(Some(&format!("undertow-size={side}")));
        Some(parsed.into())
    } else {
        Some(url)
    }
}

fn trim_artwork_cache(
    artwork: &mut HashMap<String, Artwork>,
    requested: &mut HashSet<String>,
    frame: u64,
    incoming: usize,
) {
    // A byte budget prevents old 640px lyric covers accumulating over a long
    // listening session. Current/visible artwork is pinned, even if a very
    // large viewport temporarily needs more than the cache budget.
    loop {
        let bytes: usize = artwork
            .values()
            .map(|a| {
                (a.texture.size().iter().product::<usize>()
                    + a.blur.size().iter().product::<usize>())
                    * 4
            })
            .sum();
        if artwork.len() < 256 && bytes.saturating_add(incoming) <= 8 * 1024 * 1024 {
            break;
        }
        let oldest = artwork
            .iter()
            .filter(|(_, a)| a.touched.saturating_add(2) < frame)
            .min_by_key(|(_, a)| a.touched)
            .map(|(key, _)| key.clone());
        let Some(key) = oldest else {
            break;
        };
        artwork.remove(&key);
        requested.remove(&key);
    }
}

const BUTTON: Color32 = design::SURFACE;
const BUTTON_HOVER: Color32 = design::HOVER;
const PANEL: Color32 = design::POPOVER;

fn pill(ui: &mut egui::Ui, text: impl Into<String>, active: bool) -> egui::Response {
    pill_sized(ui, text, active, 36.0, 15.0)
}
fn small_pill(ui: &mut egui::Ui, text: impl Into<String>) -> egui::Response {
    pill_sized(ui, text, false, 28.0, 13.0)
}
// Painted by hand so hover and press are eased by elapsed time (frame-rate
// independent). `active` selects the white primary variant.
fn pill_sized(
    ui: &mut egui::Ui,
    text: impl Into<String>,
    active: bool,
    height: f32,
    size: f32,
) -> egui::Response {
    let text = text.into();
    let galley = ui.painter().layout_no_wrap(
        text.clone(),
        FontId::new(size, egui::FontFamily::Name("Medium".into())),
        TEXT,
    );
    let (rect, response) =
        ui.allocate_exact_size(vec2(galley.size().x + height * 1.0, height), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), text.clone())
    });
    let hover = reference::layout::ease(ui.ctx().animate_bool_with_time(
        response.id.with("pill-hover"),
        response.hovered(),
        0.12,
    ));
    let press = response.is_pointer_button_down_on();
    let (fill, border, ink) = if active {
        (
            design::BRAND.lerp_to_gamma(Color32::from_gray(224), hover),
            Color32::TRANSPARENT,
            design::INK,
        )
    } else {
        (
            BUTTON.lerp_to_gamma(BUTTON_HOVER, hover),
            Color32::TRANSPARENT,
            TEXT,
        )
    };
    let body = rect.expand(if press { -1.5 } else { hover * 0.8 });
    ui.painter().rect_filled(body, body.height() * 0.5, fill);
    ui.painter().rect_stroke(
        body,
        body.height() * 0.5,
        Stroke::new(
            1.0_f32,
            if response.has_focus() {
                design::MUTED
            } else {
                border
            },
        ),
        egui::StrokeKind::Inside,
    );
    ui.painter().galley_with_override_text_color(
        body.center() - galley.size() * 0.5,
        galley,
        if ui.is_enabled() { ink } else { MUTED },
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
/// Turns the cover's average color into a bright, saturated accent for lyrics.
/// Near-gray covers stay neutral so black-and-white art doesn't get a random tint.
fn vibrant(rgb: [u8; 3]) -> Color32 {
    let [r, g, b] = rgb.map(|c| c as f32 / 255.);
    let max = r.max(g).max(b);
    let delta = max - r.min(g).min(b);
    if delta < 0.06 {
        return Color32::from_rgb(236, 242, 246);
    }
    let sextant = if max == r {
        ((g - b) / delta).rem_euclid(6.)
    } else if max == g {
        (b - r) / delta + 2.
    } else {
        (r - g) / delta + 4.
    };
    let value = 0.98;
    let saturation = (delta / max * 1.3).clamp(0.62, 0.95);
    let chroma = value * saturation;
    let x = chroma * (1. - (sextant % 2. - 1.).abs());
    let (r, g, b) = match sextant as u8 {
        0 => (chroma, x, 0.),
        1 => (x, chroma, 0.),
        2 => (0., chroma, x),
        3 => (0., x, chroma),
        4 => (x, 0., chroma),
        _ => (chroma, 0., x),
    };
    let m = value - chroma;
    Color32::from_rgb(
        ((r + m) * 255.) as u8,
        ((g + m) * 255.) as u8,
        ((b + m) * 255.) as u8,
    )
}
fn modal_frame() -> Frame {
    Frame::new()
        .fill(PANEL)
        .corner_radius(22)
        .stroke(Stroke::new(1.0_f32, Color32::from_white_alpha(22)))
        .inner_margin(28)
        .shadow(egui::epaint::Shadow {
            offset: [0, 18],
            blur: 48,
            spread: 0,
            color: Color32::from_black_alpha(170),
        })
}
fn muted(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).size(13.0).color(MUTED));
}
fn heading(ui: &mut egui::Ui, text: impl Into<String>, size: f32) {
    ui.label(RichText::new(text.into()).size(size).strong().color(TEXT));
}
fn card() -> Frame {
    Frame::new()
        .fill(CARD)
        .corner_radius(18)
        .stroke(Stroke::new(1.0_f32, Color32::from_white_alpha(12)))
        .inner_margin(22)
}
struct LyricBlur {
    radius: f32,
    trail: f32,
}
#[cfg(test)]
fn lyric_text(
    ui: &mut egui::Ui,
    text: &str,
    size: f32,
    color: Color32,
    glow: Option<Color32>,
    wrap: bool,
    blur: Option<LyricBlur>,
) -> egui::Response {
    let galley = crate::lyric_fonts::layout(
        ui.ctx(),
        text,
        FontId::new(size, egui::FontFamily::Name("Bold".into())),
        if wrap {
            ui.available_width()
        } else {
            f32::INFINITY
        },
    );
    lyric_galley(ui, galley, color, glow, blur)
}

// A line's shaped glyphs survive emphasis/color animation and scrolling. Only its
// paint color and position change; fonts/wrapping still invalidate the layout.
fn lyric_galley(
    ui: &mut egui::Ui,
    galley: impl Into<crate::lyric_fonts::Layout>,
    color: Color32,
    glow: Option<Color32>,
    blur: Option<LyricBlur>,
) -> egui::Response {
    let galley = galley.into();
    let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::click());
    if let Some(blur) = blur {
        let [red, green, blue, alpha] = color.to_srgba_unmultiplied();
        // Five taps instead of nine: same total weight, ~45% fewer galley draws for
        // every visible inactive line, on every frame.
        for (dx, dy, weight) in [
            (0., -1., 2.),
            (-1., 0., 2.),
            (0., 0., 8.),
            (1., 0., 2.),
            (0., 1., 2.),
        ] {
            let tint = Color32::from_rgba_unmultiplied(
                red,
                green,
                blue,
                (alpha as f32 * weight / 16.).round() as u8,
            );
            galley.paint(
                ui.painter(),
                rect.min
                    + vec2(
                        dx * blur.radius,
                        dy * (blur.radius + blur.trail.abs() * 0.5),
                    ),
                tint,
            );
        }
        return response;
    }
    paint_lyric_galley(ui.painter(), rect.min, galley, color, glow);
    response
}

fn paint_lyric_galley(
    painter: &egui::Painter,
    origin: Pos2,
    galley: impl Into<crate::lyric_fonts::Layout>,
    color: Color32,
    glow: Option<Color32>,
) {
    let galley = galley.into();
    if let Some(glow) = glow {
        let glow = glow.gamma_multiply(0.125);
        for offset in [vec2(-2., 0.), vec2(2., 0.), vec2(0., -2.), vec2(0., 2.)] {
            galley.paint(painter, origin + offset, glow);
        }
    }
    galley.paint(painter, origin, color);
}

impl App {
    pub fn new(ctx: &egui::Context, args: &[String]) -> Self {
        let mut settings = Settings::load();
        if args.iter().any(|arg| arg == "--background-benchmark")
            && let Some(scale) = args
                .windows(2)
                .find(|pair| pair[0] == "--preview-scale")
                .and_then(|pair| pair[1].parse::<f32>().ok())
                .filter(|scale| (1. ..=2.).contains(scale))
        {
            ctx.set_zoom_factor(scale);
        }
        if let Some(i) = args.iter().position(|a| a == "--fps")
            && let Some(fps) = args.get(i + 1).and_then(|s| s.parse::<u32>().ok())
        {
            settings.fps = fps.clamp(30, 240);
        }
        if args.iter().any(|a| a == "--show-sidebar") {
            settings.show_sidebar = true;
        }
        if args.iter().any(|a| a == "--collapsed") {
            settings.sidebar_collapsed = true;
        }
        if args.iter().any(|a| a == "--lossless-login") {
            settings.compatibility = true;
            settings.device_login = false;
            settings.paste_login = true;
        }
        if args.iter().any(|a| a == "--floating") {
            settings.floating_lyrics = true;
        }
        if settings.lyric_color == [35, 225, 203] {
            settings.lyric_color = [255, 255, 255];
        }
        if (settings.lyric_size - 38.0).abs() < 0.01 {
            settings.lyric_size = 48.0;
        }
        style(ctx);
        if !load_font(ctx, settings.effective_font()) {
            settings.font_enabled = false;
            load_font(ctx, "");
        }
        let worker = Worker::start(settings.clone(), ctx.clone());
        let (ptx, prx) = mpsc::sync_channel::<PresenceSnapshot>(1);
        let (stx, srx) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("discord-presence".into())
            .spawn(move || {
                let mut p = Presence::default();
                while let Ok(s) = prx.recv() {
                    let _ = p.update(
                        &s.settings.discord_client_id,
                        s.track.as_ref(),
                        s.position,
                        s.duration,
                        s.paused,
                        s.settings.discord,
                    );
                    let _ = stx.try_send(p.status.clone());
                }
            })
            .expect("Discord worker");
        let value = |flag: &str| {
            args.iter()
                .position(|a| a == flag)
                .and_then(|i| args.get(i + 1))
                .cloned()
        };
        let benchmark = value("--benchmark")
            .and_then(|v| v.parse::<f64>().ok())
            .map(|n| n.clamp(5.0, 7200.0));
        let mut app = Self {
            font_loaded: settings.effective_font().to_owned(),
            settings,
            worker,
            page: if args.iter().any(|a| a == "--settings") {
                Page::Settings
            } else if args.iter().any(|a| a == "--profile") {
                Page::Profile
            } else if args.iter().any(|a| a == "--favorites") {
                Page::Favorites
            } else if args.iter().any(|a| a == "--lyrics") {
                Page::Lyrics
            } else {
                Page::Home
            },
            playlists: Arc::new(
                crate::config::read_json(&data_dir().join("playlists.json")).unwrap_or_default(),
            ),
            tracks: HashMap::new(),
            favorite_ids: HashSet::new(),
            queue: Arc::new(Vec::new()),
            index: 0,
            current: None,
            audio: None,
            completed_audio_seconds: 0.,
            completed_underruns: 0,
            generation: 0,
            lyrics: Arc::new(Lyrics::default()),
            lyrics_status: "Choose a track to see its lyrics".into(),
            lyric_request: LyricRequest::default(),
            lyrics_chrome_hidden: args.iter().any(|a| a == "--hide-lyrics-ui"),
            translated: false,
            query: String::new(),
            local_path: String::new(),
            account: "Not connected".into(),
            connected: false,
            busy: false,
            notice: String::new(),
            notice_at: Instant::now(),
            error: false,
            artwork: HashMap::new(),
            requested: HashSet::new(),
            art_requests: HashMap::new(),
            art_visible: HashSet::new(),
            art_scroll_check: (args.iter().any(|a| a == "--artwork-scroll-benchmark")
                && args.iter().any(|a| a == "--background-benchmark")
                && benchmark.is_some())
            .then(ArtworkScrollCheck::default),
            frame: 0,
            dialog: (benchmark.is_some()
                && args.iter().any(|a| a == "--background-benchmark")
                && args.iter().any(|a| a == "--preview-create"))
            .then(|| Dialog {
                kind: "create".into(),
                ..Default::default()
            }),
            download_active: false,
            metrics: Metrics::default(),
            presence_tx: ptx,
            presence_rx: srx,
            presence_status: "Discord application ID needed".into(),
            last_presence: Instant::now() - Duration::from_secs(30),
            last_sync: Instant::now(),
            show_details: false,
            benchmark,
            report: value("--report")
                .map(PathBuf::from)
                .unwrap_or_else(|| data_dir().join("benchmark.json")),
            screenshot: value("--screenshot").map(PathBuf::from),
            screenshot_requested: false,
            screenshot_number: 0,
            record_live: args.iter().any(|a| a == "--record-live"),
            last_record: Instant::now(),
            overlay_window: None,
            overlay_tick: Instant::now() - Duration::from_secs(3),
            muted: args.iter().any(|a| a == "--mute"),
            autoplay_favorites: args.iter().any(|a| a == "--autoplay-favorites"),
            signing_in: false,
            device_prompt: None,
            manual_prompt: None,
            manual_return: String::new(),
            login_check: Instant::now(),
            login_progress: String::new(),
            back_history: Vec::new(),
            forward_history: Vec::new(),
            history_page: Page::Home,
            discovery: Default::default(),
            repeat_mode: 0,
            shuffle_on: false,
            lyric_offsets: Vec::new(),
            lyric_galleys: HashMap::new(),
            lyric_word_galleys: None,
            lyric_layout_key: None,
            lyric_scroll: 0.0,
            lyric_manual_scroll: false,
            lyric_velocity: 0.,
            lyric_frame_motion: false,
            lyric_follow_target: 0.,
            lyric_jump: None,
            lyric_jump_trace: Vec::new(),
            animation_clock: Instant::now(),
            playing_playlist: None,
            startup_started: None,
            startup_ms: Vec::new(),
            autoplay_album: args.iter().any(|a| a == "--autoplay-album"),
            playlist_filter: String::new(),
            playlist_filter_id: String::new(),
            playlist_index: crate::collection::TrackIndex::default(),
            playlist_scroll_reset: true,
            playlist_filtered: Arc::new(Vec::new()),
            playlist_sort: None,
            startup_playlist: value("--playlist"),
            albums: Default::default(),
            artists: HashMap::new(),
            artist_access: HashMap::new(),
            artist_tab: 0,
            artist_release_view: 0,
            artist_album_offset: 0.,
            artist_single_offset: 0.,
            artist_diagnostic_scroll: (benchmark.is_some()
                && args.iter().any(|a| a == "--background-benchmark"))
            .then(|| value("--artist-scroll").and_then(|v| v.parse::<f32>().ok()))
            .flatten(),
            artists_pending: HashSet::new(),
            catalog: Arc::new(Vec::new()),
            catalog_view: crate::collection::CatalogIndex::default(),
            catalog_query: String::new(),
            search_sent: String::new(),
            search_changed: Instant::now(),
            search_preview: false,
            search_anchor: None,
            search_tab: "Top results".into(),
            search_sort: 0,
            startup_artist: value("--artist").and_then(|s| s.parse().ok()),
            startup_album: value("--album").and_then(|s| s.parse().ok()),
            startup_search: value("--search").or_else(|| value("--search-preview")),
            pending_list: None,
            saved_later: Arc::new(
                crate::config::read_json(&data_dir().join("saved-later.json")).unwrap_or_default(),
            ),
            show_saved_later: false,
            queue_open: args.iter().any(|a| a == "--queue"),
            queue_button_rect: None,
            play_history: Arc::new(Vec::new()),
            collection: HashSet::new(),
            collection_pending: HashSet::new(),
            motion_page: Page::Home,
            motion_started: Instant::now(),
            now_tab: 2,
            tab_started: Instant::now(),
            credits: HashMap::new(),
            credits_requested: HashSet::new(),
            related_tracks: HashMap::new(),
            related_requested: HashSet::new(),
            ui_motion_until: Instant::now(),
            display_refresh_hz: 0.,
            retained_render_bytes: [0; 3],
            home_scroll: 0.,
            home_content_height: 0.,
            sidebar_expanded_width: 315.,
            accent: None,
            backdrop_grid: None,
            settings_tab: if args.iter().any(|a| a == "--plugins") {
                2
            } else {
                0
            },
            avatar: None,
            volume_hover_deadline: Instant::now(),
            volume_pinned: false,
            volume_popup_rect: None,
            lyric_last_position: None,
            lyric_entry_generation: u64::MAX,
            settings_dirty: false,
            settings_changed_at: Instant::now(),
        };
        if app.settings.profile_picture {
            app.send(Command::LoadAvatar);
        }
        if let Some(p) = value("--open") {
            app.open_local(p);
        }
        if benchmark.is_some() && args.iter().any(|a| a == "--background-benchmark") {
            app.preview_library(args);
            app.preview_discovery(args);
            if args.iter().any(|a| a == "--preview-filled")
                && let Some(d) = &mut app.dialog
            {
                d.title = "Late-night rotation".into();
                d.description = "Saved for a slower evening.".into();
            }
            if args
                .iter()
                .any(|a| a == "--preview-toast" || a == "--preview-toast-added")
            {
                app.toast(
                    if args.iter().any(|a| a == "--preview-toast-added") {
                        "Added to My Collection"
                    } else {
                        "Removed from My Collection"
                    },
                    false,
                );
            }
            if args.iter().any(|a| a == "--preview-queue")
                && let Some(track) = app.current.clone()
            {
                app.queue_open = true;
                let names = [
                    "Night Swim",
                    "Drift",
                    "City Lights",
                    "First Light",
                    "Quiet Hours",
                    "Afterglow",
                    "Blue Hour",
                    "Home Again",
                ];
                let tracks = names
                    .iter()
                    .enumerate()
                    .map(|(i, name)| Track {
                        title: (*name).into(),
                        artist: "Undertow preview".into(),
                        album: "Preview playlist".into(),
                        duration: 180 + i as u64,
                        ..track.clone()
                    })
                    .collect::<Vec<_>>();
                app.play_history = Arc::new(tracks[..5].to_vec());
                app.queue = Arc::new(tracks);
                app.index = 0;
            }
            app.preview_audit(args);
        }
        if args
            .iter()
            .any(|a| a == "--connect" || a == "--lossless-login")
        {
            app.send(Command::Login(app.settings.clone()));
        }
        app
    }
    fn send(&mut self, c: Command) {
        if matches!(c, Command::Login(_)) {
            if self.signing_in {
                self.toast("A TIDAL sign-in is already open in your browser", false);
                return;
            }
            self.signing_in = true;
            self.login_progress = if self.settings.paste_login {
                "Opening TIDAL — you'll paste the final address here."
            } else if self.settings.device_login {
                "Opening TIDAL in your browser — just approve the code."
            } else {
                "Complete sign-in in your browser, then allow it to open Undertow."
            }
            .into();
        }
        let lyric_request = matches!(c, Command::Lyrics(..) | Command::Translate(..));
        if let Err(error) = self.worker.tx.try_send(c) {
            let (mpsc::TrySendError::Full(command) | mpsc::TrySendError::Disconnected(command)) =
                error;
            let message = "Please wait for the current requests to finish".to_owned();
            match command {
                Command::Home => self.discovery.home_result(Err(message)),
                Command::DiscoverySection(section) => {
                    self.discovery.section_result(section.id, Err(message))
                }
                _ => {}
            }
            if lyric_request {
                self.lyric_request.pending = false;
            }
            self.signing_in = false;
            self.toast("Please wait for the current requests to finish", false);
        }
    }
    fn toast(&mut self, s: impl Into<String>, error: bool) {
        self.notice = s.into();
        self.notice_at = Instant::now();
        self.error = error;
    }
    fn lyric_position(&self) -> f64 {
        (self.position() + self.settings.lyric_offset_ms as f64 / 1000.).max(0.)
    }
    fn seek_lyric(&mut self, seconds: f64) {
        if let Some(audio) = &self.audio {
            audio.seek((seconds - self.settings.lyric_offset_ms as f64 / 1000.).max(0.));
            self.lyric_jump = self
                .settings
                .animated_lyrics
                .then(|| lyric_jump::Jump::new(self.lyric_scroll, seconds, true));
            self.lyric_frame_motion = self.lyric_jump.is_some();
            self.lyric_manual_scroll = false;
            self.lyric_velocity = 0.;
            self.lyric_entry_generation = self.generation;
        }
    }
    fn schedule_settings_save(&mut self) {
        self.settings_dirty = true;
        self.settings_changed_at = Instant::now();
    }
    fn position(&self) -> f64 {
        self.audio.as_ref().map(Audio::position).unwrap_or(0.0)
    }
    fn finish_audio(&mut self) {
        if let Some(audio) = self.audio.take() {
            self.completed_audio_seconds += audio.consumed_seconds();
            self.completed_underruns += audio.stats.underrun_samples.load(Ordering::Relaxed);
        }
    }
    fn duration(&self) -> f64 {
        self.audio
            .as_ref()
            .map(|a| a.format.duration)
            .filter(|d| *d > 0.0)
            .unwrap_or_else(|| {
                self.current
                    .as_ref()
                    .map(|t| t.duration as f64)
                    .unwrap_or(0.0)
            })
    }
    fn paused(&self) -> bool {
        self.audio.as_ref().is_none_or(|a| a.sink.is_paused())
    }
    fn toggle(&mut self) {
        if let Some(a) = &self.audio {
            if a.sink.is_paused() {
                a.sink.play();
            } else {
                a.sink.pause();
            }
        }
        self.last_presence = Instant::now() - Duration::from_secs(6);
    }
    fn play(&mut self, track: Track) {
        if self.audio.is_some()
            && let Some(previous) = &self.current
        {
            let history = Arc::make_mut(&mut self.play_history);
            if history
                .last()
                .is_none_or(|t| t.id != previous.id || t.local_path != previous.local_path)
            {
                history.push(previous.clone());
                if history.len() > 80 {
                    history.remove(0);
                }
            }
        }
        self.lyric_scroll = 0.0;
        self.lyric_word_galleys = None;
        self.lyric_jump = None;
        self.lyric_manual_scroll = false;
        self.lyric_velocity = 0.;
        self.startup_started = Some(Instant::now());
        self.lyric_layout_key = None;
        self.lyric_galleys.clear();
        self.finish_audio();
        self.generation += 1;
        self.lyric_request.pending = false;
        self.worker
            .generation
            .store(self.generation, Ordering::Release);
        self.lyrics = Arc::new(Lyrics::default());
        self.lyrics_status = "Loading lyrics…".into();
        self.translated = false;
        self.current = Some(track.clone());
        self.busy = true;
        self.send(Command::Prepare(self.generation, track));
    }
    fn request_lyrics(&mut self, refresh: bool) {
        if let Some(track) = self.current.clone() {
            let request = self.lyric_request.begin();
            if refresh {
                self.translated = false;
                self.lyric_layout_key = None;
                self.lyric_entry_generation = u64::MAX;
                self.lyric_last_position = None;
                self.lyric_velocity = 0.;
            }
            self.send(Command::Lyrics(self.generation, request, track, refresh));
        }
    }
    fn play_list(&mut self, tracks: Arc<Vec<Track>>, index: usize) {
        if let Some(t) = tracks.get(index).cloned() {
            self.playing_playlist = if let Page::Playlist(id) = &self.page {
                Some(id.clone())
            } else {
                None
            };
            self.queue = tracks;
            self.index = index;
            self.play(t);
        }
    }
    fn next(&mut self) {
        if self.index + 1 < self.queue.len() {
            self.index += 1;
            if let Some(t) = self.queue.get(self.index).cloned() {
                self.play(t);
            }
        } else if self.repeat_mode == 1 && !self.queue.is_empty() {
            self.play_list(self.queue.clone(), 0);
        } else {
            self.finish_audio();
        }
    }
    fn previous(&mut self) {
        if self.position() > 3.0 {
            if let Some(a) = &self.audio {
                a.seek(0.0);
            }
        } else if self.index > 0 {
            self.index -= 1;
            if let Some(t) = self.queue.get(self.index).cloned() {
                self.play(t);
            }
        }
    }
    fn open_local(&mut self, p: String) {
        let path = PathBuf::from(&p);
        if !path.is_file() {
            self.toast("The audio file does not exist", true);
            return;
        }
        let mut t = Track {
            title: path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Local audio")
                .into(),
            artist: "Local file".into(),
            album: "Your audio".into(),
            cover: [path.with_extension("jpg"), path.with_extension("png")]
                .into_iter()
                .find(|p| p.is_file())
                .map(|p| format!("local:{}", p.display()))
                .unwrap_or_default(),
            local_path: p,
            ..Default::default()
        };
        if self.benchmark.is_some()
            && std::env::args().any(|a| a == "--background-benchmark")
            && std::env::args().any(|a| a == "--preview-player")
        {
            t.title = "All Around Me".into();
            t.artist = "Flyleaf".into();
        }
        self.play_list(Arc::new(vec![t]), 0);
    }
    fn drain(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.worker.rx.try_recv() {
            let Some(event) =
                event.for_catalog_epoch(self.worker.catalog_epoch.load(Ordering::Acquire))
            else {
                continue;
            };
            let event = match event {
                Event::Mix(id, result) => {
                    let key = format!("mix:{id}");
                    match result {
                        Ok(tracks) => {
                            self.discovery.errors.remove(&key);
                            Event::Tracks(key, tracks)
                        }
                        Err(error) => {
                            if self.pending_list.as_ref().is_some_and(|(k, _)| k == &key) {
                                self.pending_list = None;
                            }
                            self.discovery.errors.insert(key, error.clone());
                            self.toast(error, true);
                            continue;
                        }
                    }
                }
                event => event,
            };
            match event {
                Event::Mix(..) => unreachable!("mix results converted above"),
                Event::CatalogRead(..) | Event::LibraryRead(..) => {
                    unreachable!("catalog events are unwrapped once")
                }
                Event::Avatar(image) => {
                    self.avatar =
                        Some(ctx.load_texture("profile-picture", *image, TextureOptions::LINEAR));
                    self.settings.profile_picture = true;
                    if self.benchmark.is_none() {
                        self.schedule_settings_save();
                    }
                }
                Event::PickedFont(path) => {
                    if load_font(ctx, &path) {
                        self.font_loaded = path.clone();
                        self.settings.font_path = path;
                        self.settings.font_enabled = true;
                        self.lyric_layout_key = None;
                        self.schedule_settings_save();
                    } else {
                        self.toast("Could not load this font (maximum 8 MiB)", true);
                    }
                }
                Event::PlaylistImage(request, result) => {
                    if let Some(d) = &mut self.dialog
                        && d.revision == request
                    {
                        d.picking_image = false;
                        match result {
                            Ok(image) => {
                                d.preview = Some(ctx.load_texture(
                                    format!("playlist-draft-{request}"),
                                    image.preview,
                                    TextureOptions::LINEAR,
                                ));
                                d.image = Some(Arc::new(image.png));
                                d.problem.clear();
                            }
                            Err(error) if !error.is_empty() => d.problem = error,
                            _ => {}
                        }
                    }
                }
                Event::PlaylistSaved(request, result) => match result {
                    Ok((message, warning)) => {
                        if self.dialog.as_ref().is_some_and(|d| d.revision == request) {
                            self.dialog = None;
                        }
                        self.artwork.retain(|key, _| !key.contains("playlist-art"));
                        self.requested.retain(|key| !key.contains("playlist-art"));
                        self.toast(message, warning);
                    }
                    Err(error) => {
                        if let Some(d) = &mut self.dialog
                            && d.revision == request
                        {
                            d.saving = false;
                            d.problem = error;
                        } else {
                            self.toast(error, true);
                        }
                    }
                },
                Event::DeviceCode(prompt) => {
                    self.signing_in = true;
                    // The browser opens with the code prefilled; the copy is a fallback
                    // for entering it by hand (for example on a phone).
                    ctx.copy_text(prompt.user_code.clone());
                    self.device_prompt = Some(prompt);
                }
                Event::ManualReturn(prompt) => {
                    self.signing_in = true;
                    self.manual_prompt = Some(prompt);
                }
                Event::Notice(s) => {
                    if s.starts_with("Lyrics:") {
                        self.lyrics_status = s.clone();
                    }
                    self.toast(s, false);
                }
                Event::Error(s) => {
                    self.pending_list = None;
                    self.device_prompt = None;
                    self.manual_prompt = None;
                    zeroize::Zeroize::zeroize(&mut self.manual_return);
                    self.signing_in = false;
                    self.busy = false;
                    self.download_active = false;
                    self.toast(s, true);
                }
                Event::Home(result) => self.discovery.home_result(result),
                Event::DiscoverySection(id, result) => self.discovery.section_result(id, result),
                Event::Connected(s) => {
                    self.albums.clear();
                    self.discovery = Default::default();
                    self.artists.clear();
                    self.artist_access.clear();
                    self.artists_pending.clear();
                    self.device_prompt = None;
                    self.manual_prompt = None;
                    zeroize::Zeroize::zeroize(&mut self.manual_return);
                    self.signing_in = false;
                    self.connected = true;
                    self.account = s;
                    self.busy = false;
                    self.refresh_discovery();
                    self.send(Command::Sync);
                    self.send(Command::CollectionState);
                }
                Event::SignedOut => {
                    self.albums.clear();
                    self.device_prompt = None;
                    self.manual_prompt = None;
                    zeroize::Zeroize::zeroize(&mut self.manual_return);
                    self.signing_in = false;
                    self.connected = false;
                    self.account = "Not connected".into();
                    self.artists.clear();
                    self.artist_access.clear();
                    self.artists_pending.clear();
                    self.playlists = Arc::default();
                    self.tracks.clear();
                    self.playlist_index.invalidate();
                    self.discovery = Default::default();
                    self.catalog_view = crate::collection::CatalogIndex::default();
                    self.playlist_filtered = Arc::new(Vec::new());
                    self.favorite_ids.clear();
                    self.play_history = Arc::new(Vec::new());
                    self.queue_open = false;
                    self.current = None;
                    self.finish_audio();
                    self.lyrics = Arc::new(Lyrics::default());
                    let _ = std::fs::remove_file(data_dir().join("playlists.json"));
                }
                Event::Playlists(p) => {
                    self.collection
                        .retain(|k| k.starts_with("album:") || k.starts_with("artist:"));
                    self.collection.extend(p.iter().map(|p| p.id.clone()));
                    self.playlists = Arc::new(p);
                    self.busy = false;
                    self.last_sync = Instant::now();
                    if let Some(id) = self.startup_artist.take() {
                        self.open_artist(id);
                    }
                    if let Some(id) = self.startup_album.take() {
                        let lyrics_view = self.page == Page::Lyrics;
                        self.open_album(id, String::new());
                        if lyrics_view {
                            self.page = Page::Lyrics;
                        }
                    }
                    if let Some(q) = self.startup_search.take() {
                        self.query = q;
                        self.search_preview = std::env::args().any(|a| a == "--search-preview");
                        if !self.search_preview {
                            self.page = Page::Search;
                        }
                        self.request_catalog();
                    }
                    if let Some(requested) = self.startup_playlist.take()
                        && let Some(playlist) = self
                            .playlists
                            .iter()
                            .find(|p| {
                                p.id == requested
                                    || p.title.to_lowercase().contains(&requested.to_lowercase())
                            })
                            .cloned()
                    {
                        self.page = Page::Playlist(playlist.id.clone());
                        self.send(Command::Playlist(playlist.id));
                    }
                    if !self.tracks.contains_key("favorites") {
                        self.send(Command::Favorites);
                    }
                }
                Event::Tracks(key, t) => {
                    if key == "favorites" {
                        self.favorite_ids = t.iter().filter(|t| t.id > 0).map(|t| t.id).collect();
                    }
                    let tracks = Arc::new(t);
                    if self.pending_list.as_ref().is_some_and(|(k, _)| k == &key)
                        && let Some((_, action)) = self.pending_list.take()
                    {
                        self.apply_list_action(action, tracks.clone());
                    }
                    if (self.autoplay_favorites && key == "favorites")
                        || (self.autoplay_album && key.starts_with("album:"))
                    {
                        self.autoplay_album = false;
                        self.autoplay_favorites = false;
                        if tracks.is_empty() {
                            self.toast("There are no saved tracks to play automatically", true);
                        } else {
                            self.play_list(tracks.clone(), 0);
                        }
                    }
                    self.tracks.insert(key.clone(), tracks);
                    // Keep complete active pages, evict inactive metadata before it can accumulate.
                    while self.tracks.values().map(|t| t.len()).sum::<usize>() > 20000
                        && self.tracks.len() > 1
                    {
                        let remove = self
                            .tracks
                            .keys()
                            .find(|k| **k != key && k.as_str() != "favorites")
                            .cloned()
                            .or_else(|| self.tracks.keys().find(|k| **k != key).cloned());
                        if let Some(k) = remove {
                            self.tracks.remove(&k);
                        } else {
                            break;
                        }
                    }
                    self.busy = false;
                }
                Event::Prepared(g, t, p) => {
                    let t = *t;
                    if g != self.generation {
                        p.stats.cancel.store(true, Ordering::Relaxed);
                        continue;
                    }
                    match Audio::start(
                        *p,
                        if self.muted {
                            0.0
                        } else {
                            self.settings.volume
                        },
                    ) {
                        Ok(a) => {
                            if let Some(track) = Arc::make_mut(&mut self.queue).get_mut(self.index)
                            {
                                track.duration = a.format.duration as u64;
                            }
                            self.current = Some(t.clone());
                            self.finish_audio();
                            self.audio = Some(a);
                            self.busy = false;
                            self.last_presence = Instant::now() - Duration::from_secs(6);
                            self.request_lyrics(false);
                        }
                        Err(e) => {
                            self.busy = false;
                            self.toast(e.to_string(), true);
                        }
                    }
                }
                Event::Lyrics(g, request, result) => {
                    if g == self.generation && self.lyric_request.finish(request) {
                        let l = match result {
                            Ok(lyrics) => lyrics,
                            Err(error) => {
                                if self.lyrics.lines.is_empty() {
                                    self.lyrics_status = "Lyrics unavailable · try resync".into();
                                }
                                self.toast(format!("Lyrics: {error}"), true);
                                continue;
                            }
                        };
                        self.lyric_layout_key = None;
                        self.lyric_entry_generation = u64::MAX;
                        self.lyric_last_position = None;
                        self.lyrics_status = format!(
                            "{} · {}",
                            l.source,
                            if l.word_synced() {
                                "Word synced"
                            } else if l.synced {
                                "Line synced"
                            } else {
                                "Unsynced"
                            }
                        );
                        self.lyrics = Arc::new(l);
                    }
                }
                Event::DownloadDone(s) => {
                    self.download_active = false;
                    self.toast(s, false);
                }
                Event::Catalog(q, items) => {
                    if q.trim() == self.query.trim() {
                        self.catalog_query = q;
                        self.catalog = Arc::new(items);
                        self.busy = false;
                    }
                }
                Event::Collection(key, saved) => {
                    self.collection_pending.remove(&key);
                    if saved {
                        self.collection.insert(key);
                    } else {
                        self.collection.remove(&key);
                    }
                    self.toast(
                        if saved {
                            "Added to My Collection"
                        } else {
                            "Removed from My Collection"
                        },
                        false,
                    );
                }
                Event::CollectionFailed(key, error) => {
                    self.collection_pending.remove(&key);
                    self.toast(error, true);
                }
                Event::CollectionState(keys) => {
                    self.collection.retain(|k| !k.starts_with("album:"));
                    self.collection.extend(keys);
                }
                Event::Credits(id, result) => {
                    if self.credits.len() >= 12 {
                        self.credits.clear();
                        self.credits_requested.clear();
                        self.credits_requested.insert(id);
                    }
                    self.credits.insert(id, result);
                }
                Event::Similar(id, result) => {
                    if self.related_tracks.len() >= 12 {
                        self.related_tracks.clear();
                        self.related_requested.clear();
                        self.related_requested.insert(id);
                    }
                    self.related_tracks.insert(id, result.map(Arc::new));
                }
                Event::Artist(id, result) => {
                    self.artists_pending.remove(&id);
                    if let Ok(artist) = &result
                        && let Some(followed) = artist.followed
                    {
                        let key = format!("artist:{id}");
                        if followed {
                            self.collection.insert(key);
                        } else {
                            self.collection.remove(&key);
                        }
                    }
                    self.cache_artist(id, result.map(|a| Arc::new(*a)));
                }
                Event::ArtistPreview(id, artist) => {
                    if self.artists.get(&id).is_none_or(|cached| {
                        cached.as_ref().ok().is_none_or(|a| a.tracks.is_empty())
                    }) {
                        self.cache_artist(id, Ok(Arc::new(*artist)));
                    }
                }
                Event::AlbumInfo(info) => {
                    self.albums.insert(info.id, *info);
                }
            }
        }
        for _ in 0..6 {
            let Ok(completion) = self.worker.art.rx.try_recv() else {
                break;
            };
            let url = completion.url;
            let a = completion.image;
            self.worker.art.acknowledge(&url);
            let incoming = (a.cover.pixels.len() + a.blur.pixels.len()) * 4;
            self.trim_artwork(incoming);
            let tex = ctx.load_texture(url.clone(), a.cover, TextureOptions::LINEAR);
            let blur = ctx.load_texture(format!("blur:{url}"), a.blur, TextureOptions::LINEAR);
            self.artwork.insert(
                url.clone(),
                Artwork {
                    texture: tex,
                    blur,
                    palette: a.palette,
                    touched: self.frame,
                },
            );
            self.requested.insert(url);
        }
        while let Ok(s) = self.presence_rx.try_recv() {
            self.presence_status = s;
        }
    }
    fn trim_artwork(&mut self, incoming: usize) {
        trim_artwork_cache(&mut self.artwork, &mut self.requested, self.frame, incoming);
    }
    fn cover(&mut self, ui: &mut egui::Ui, t: &Track, size: f32) -> egui::Response {
        self.cover_with_radius(ui, t, size, 5)
    }
    fn cover_with_radius(
        &mut self,
        ui: &mut egui::Ui,
        t: &Track,
        size: f32,
        radius: u8,
    ) -> egui::Response {
        self.cover_image(ui, t, size, radius, true)
    }
    fn cover_image(
        &mut self,
        ui: &mut egui::Ui,
        t: &Track,
        size: f32,
        radius: u8,
        placeholder: bool,
    ) -> egui::Response {
        // Keep layout/hit areas intact, but do not fetch or touch offscreen artwork.
        let bounds = Rect::from_min_size(ui.next_widget_position(), vec2(size, size));
        if !ui.is_rect_visible(bounds) {
            if bounds.intersects(ui.clip_rect().expand2(vec2(0., 160.))) {
                self.prefetch_cover(t, size, ui.ctx().pixels_per_point());
            }
            return ui.allocate_exact_size(vec2(size, size), Sense::click()).1;
        }
        let mut texture = None;
        if self.settings.show_covers {
            let side = artwork_side(size, ui.ctx().pixels_per_point());
            if let Some(url) = artwork_key(t, side) {
                self.art_visible.insert(url.clone());
                if !self.artwork.contains_key(&url) {
                    let priority = if self
                        .current
                        .as_ref()
                        .is_some_and(|c| c.id == t.id && c.cover == t.cover)
                    {
                        5
                    } else if size >= 200. {
                        4
                    } else if size < 50. {
                        2
                    } else {
                        3
                    };
                    self.art_requests
                        .entry(url.clone())
                        .and_modify(|p| *p = (*p).max(priority))
                        .or_insert(priority);
                }
                let available = std::iter::once(side)
                    .chain([640, 320, 160, 80].into_iter().filter(|s| *s != side))
                    .filter_map(|s| artwork_key(t, s))
                    .find(|key| self.artwork.contains_key(key));
                if let Some(key) = available
                    && let Some(art) = self.artwork.get_mut(&key)
                {
                    art.touched = self.frame;
                    texture = Some(art.texture.clone());
                }
            }
        }
        if let Some(tex) = texture {
            ui.add(
                egui::Image::new(&tex)
                    .fit_to_exact_size(vec2(size, size))
                    .corner_radius(radius)
                    .sense(Sense::click()),
            )
        } else {
            let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::click());
            if !placeholder {
                return response;
            }
            let p = ui.painter();
            p.rect_filled(rect, 10, Color32::from_rgb(25, 40, 46));
            p.circle_stroke(
                rect.center(),
                size * 0.28,
                Stroke::new((size * 0.015).max(1.0), Color32::from_rgb(53, 89, 91)),
            );
            p.circle_filled(rect.center(), size * 0.06, ACCENT);
            response
        }
    }
    fn diagnostic_scroll_offset(&mut self, rows: usize) -> Option<f32> {
        let check = self.art_scroll_check.as_mut()?;
        let started = check.started.get_or_insert_with(Instant::now);
        let sequence = [
            0, 10, 24, 48, 80, 120, 180, 240, 300, 240, 180, 120, 80, 48, 24, 10, 0,
        ];
        check.step =
            ((started.elapsed().as_secs_f32() / 2.).floor() as usize).min(sequence.len() - 1);
        Some(sequence[check.step].min(rows.saturating_sub(10)) as f32 * 64.)
    }
    fn record_artwork_scroll(&mut self, ctx: &egui::Context) {
        let Some(check) = self.art_scroll_check.as_mut() else {
            return;
        };
        let Some(started) = check.started else {
            return;
        };
        let wanted = self.art_visible.len();
        let loaded = self
            .art_visible
            .iter()
            .filter(|u| self.artwork.contains_key(*u))
            .count();
        if check
            .records
            .last()
            .is_none_or(|r| r["step"].as_u64() != Some(check.step as u64))
        {
            check.records.push(serde_json::json!({"step":check.step,"started_seconds":started.elapsed().as_secs_f64(),"visible":wanted,"loaded":loaded,"fully_loaded_after_ms":serde_json::Value::Null}));
        }
        use std::hash::{Hash, Hasher};
        let signature = self.art_visible.iter().fold(0u64, |sum, url| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            url.hash(&mut h);
            sum ^ h.finish()
        });
        let record = check.records.last_mut().unwrap();
        if record["visible_signature"].as_u64() != Some(signature) {
            record["visible_signature"] = serde_json::json!(signature);
            record["stable_frames"] = serde_json::json!(0);
            record["fully_loaded_after_ms"] = serde_json::Value::Null;
        }
        let stable = record["stable_frames"].as_u64().unwrap_or(0) + 1;
        record["stable_frames"] = serde_json::json!(stable);
        record["visible"] = serde_json::json!(wanted);
        record["loaded"] = serde_json::json!(loaded);
        if loaded < wanted {
            record["fully_loaded_after_ms"] = serde_json::Value::Null;
        }
        if wanted > 0
            && wanted == loaded
            && stable >= 3
            && record["fully_loaded_after_ms"].is_null()
        {
            record["fully_loaded_after_ms"] = serde_json::json!(
                (started.elapsed().as_secs_f64() - check.step as f64 * 2.) * 1000.
            );
        }
        ctx.request_repaint_after(Duration::from_millis(16));
    }
    fn prefetch_cover(&mut self, track: &Track, size: f32, pixels_per_point: f32) {
        if !self.settings.show_covers {
            return;
        }
        if let Some(url) = artwork_key(track, artwork_side(size, pixels_per_point))
            && !self.artwork.contains_key(&url)
        {
            self.art_requests.entry(url).or_insert(1);
        }
    }
    fn background(&mut self, ctx: &egui::Context) {
        self.reference_background(ctx);
    }
    fn title_bar(&mut self, ctx: &egui::Context) {
        self.reference_title(ctx);
    }
    fn sidebar(&mut self, ctx: &egui::Context) {
        self.reference_sidebar(ctx);
    }
    fn player(&mut self, ctx: &egui::Context) {
        self.reference_player(ctx);
    }
    fn toolbar(&mut self, ui: &mut egui::Ui) {
        self.reference_toolbar(ui);
    }
    fn home(&mut self, ui: &mut egui::Ui) {
        self.reference_home(ui);
    }
    fn track_menu(
        &mut self,
        ui: &mut egui::Ui,
        t: &Track,
        i: usize,
        tracks: &Arc<Vec<Track>>,
        playlist: Option<&str>,
        owned: bool,
    ) {
        self.reference_track_menu(ui, t, i, tracks, playlist, owned);
    }
    fn lyrics_page(&mut self, ui: &mut egui::Ui) {
        self.reference_lyrics(ui);
    }
    fn settings_page(&mut self, ui: &mut egui::Ui) {
        self.reference_settings(ui);
    }
    fn dialogs(&mut self, ctx: &egui::Context) {
        self.playlist_dialog(ctx);
    }
    fn overlays(&mut self, ctx: &egui::Context) {
        if let Some(prompt) = self.manual_prompt.clone() {
            let mut cancel = ctx.input(|i| i.key_pressed(egui::Key::Escape));
            egui::Window::new("Lossless TIDAL sign-in").title_bar(false).frame(modal_frame())
                .collapsible(false).resizable(false)
                .default_width(470.).anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new().color(ACCENT).size(22.));
                    heading(ui, "Connect your music", 24.);
                    });
                    ui.add_space(14.);
                    muted(ui, "1 · Sign in on TIDAL's website");
                    ui.add_space(6.);
                    if pill(ui, "Open TIDAL sign-in", true).clicked() { let _ = crate::auth::open_browser(&prompt.authorize_url); }
                    ui.add_space(16.);
                    muted(ui, "2 · Copy the final browser address. An Oops page is expected. Paste it below to connect automatically.");
                    ui.add_space(6.);
                    let before = self.manual_return.len();
                    let edit = ui.add(egui::TextEdit::singleline(&mut self.manual_return).password(true).desired_width(f32::INFINITY).margin(vec2(12., 9.)).hint_text("https://tidal.com/android/login/auth?…"));
                    let pasted = edit.changed() && self.manual_return.len() > before + 20 && self.manual_return.contains("code=");
                    let entered = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    ui.add_space(12.);
                    ui.horizontal(|ui| {
                        if pill(ui, "Paste sign-in link", true).clicked() {
                            zeroize::Zeroize::zeroize(&mut self.manual_return);
                            edit.request_focus();
                            ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                        }
                    });
                    let clicked = !self.manual_return.is_empty() && small_pill(ui, "Connect account").clicked();
                    if clicked || pasted || entered {
                        match crate::auth::submit_return(&self.manual_return) {
                            Ok(()) => { zeroize::Zeroize::zeroize(&mut self.manual_return); self.login_progress = "Verifying your account with TIDAL…".into(); }
                            Err(error) => self.toast(error.to_string(), true),
                        }
                    }
                    ui.add_space(12.);
                    muted(ui, "Paste the address only in this app — it holds a temporary sign-in code.");
                    muted(ui, format!("Request expires in {}", clock(prompt.expires_at.saturating_sub(crate::auth::now()) as f64)));
                    ui.add_space(10.);
                    if small_pill(ui, "Cancel").clicked() { cancel = true; }
                });
            if cancel {
                crate::auth::cancel_login();
                self.manual_prompt = None;
                zeroize::Zeroize::zeroize(&mut self.manual_return);
            }
        }
        if let Some(prompt) = self.device_prompt.clone() {
            let mut cancel = ctx.input(|i| i.key_pressed(egui::Key::Escape));
            egui::Window::new("Connect TIDAL").title_bar(false).frame(modal_frame())
                .collapsible(false).resizable(false).default_width(430.)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new().color(ACCENT).size(22.));
                        heading(ui, "Approve in your browser", 24.);
                    });
                    ui.add_space(8.);
                    muted(ui, "TIDAL opened with your code already filled in. Approve it there — Undertow connects the moment you do.");
                    ui.add_space(16.);
                    Frame::new().fill(BG).corner_radius(14)
                        .stroke(Stroke::new(1.0_f32, Color32::from_white_alpha(18)))
                        .inner_margin(Margin::symmetric(18, 12))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.vertical_centered(|ui| {
                                ui.label(RichText::new(&prompt.user_code).monospace().size(34.).color(ACCENT));
                            });
                        });
                    ui.add_space(6.);
                    muted(ui, "Code copied. On another device, open link.tidal.com and enter it.");
                    ui.add_space(14.);
                    ui.horizontal(|ui| {
                        if pill(ui, "Open TIDAL again", true).clicked() {
                            let _ = crate::auth::open_browser(&prompt.verification_url);
                        }
                        if pill(ui, "Copy code", false).clicked() { ctx.copy_text(prompt.user_code.clone()); }
                    });
                    ui.add_space(14.);
                    muted(ui, format!("Code expires in {}", clock(prompt.expires_at.saturating_sub(crate::auth::now()) as f64)));
                    ui.add_space(10.);
                    if small_pill(ui, "Cancel").clicked() { cancel = true; }
                });
            if cancel {
                crate::auth::cancel_login();
                self.device_prompt = None;
            }
        }
        let toast = self.reference_toast(ctx);
        if self.settings.everywhere
            && self.page != Page::Lyrics
            && self.dialog.is_none()
            && self.manual_prompt.is_none()
            && self.device_prompt.is_none()
            && let Some(i) = self.lyrics.active(self.lyric_position())
            && !self.lyrics.lines[i].text.trim().is_empty()
        {
            let line = self.lyrics.lines[i].text.clone();
            let galley = crate::lyric_fonts::layout(
                ctx,
                &line,
                FontId::new(18., egui::FontFamily::Name("Bold".into())),
                f32::INFINITY,
            );
            let sidebar = if self.settings.show_sidebar {
                if self.settings.sidebar_collapsed || ctx.content_rect().width() < 980. {
                    68.
                } else {
                    self.sidebar_expanded_width
                }
            } else {
                0.
            };
            if let Some(mut rect) = reference::layout::caption_rect(
                ctx.content_rect(),
                reference::layout::reserved_player(ctx),
                sidebar,
                self.queue_open,
                galley.size().x + 56.,
            ) {
                if let Some(toast) = toast
                    && toast.intersects(rect)
                {
                    rect = rect.translate(vec2(0., toast.top() - rect.bottom() - 12.));
                }
                let painter = ctx
                    .layer_painter(egui::LayerId::new(
                        egui::Order::Foreground,
                        Id::new("everywhere-lyrics"),
                    ))
                    .with_clip_rect(rect);
                painter.rect_filled(rect, 28, Color32::from_black_alpha(210));
                galley.paint(
                    &painter,
                    pos2(rect.left() + 28., rect.center().y - galley.size().y / 2.),
                    TEXT,
                );
            }
        }
        self.update_overlay();
        if self.show_details {
            let current = self.current.clone();
            egui::Window::new("Track information")
                .open(&mut self.show_details)
                .default_width(380.)
                .show(ctx, |ui| {
                    if let Some(track) = &current {
                        heading(ui, &track.title, 23.);
                        muted(ui, &track.artist);
                        muted(ui, &track.album);
                        if !track.isrc.is_empty() {
                            muted(ui, format!("ISRC: {}", track.isrc));
                        }
                        if ui.button("Full credits on TIDAL").clicked() {
                            let _ = crate::auth::open_browser(&format!(
                                "https://tidal.com/browse/track/{}",
                                track.id
                            ));
                        }
                    } else {
                        muted(ui, "Play a track to see its information.");
                    }
                });
        }
    }
    fn save_report(&self) {
        let a = self.audio.as_ref();
        let details = serde_json::json!({"authenticated":self.connected,"playback_active":a.is_some_and(|a|!a.sink.is_paused()&&!a.sink.empty()),"tidal_stream":self.current.as_ref().is_some_and(|t|t.id>0&&t.local_path.is_empty()),"muted":self.muted,"codec":a.map(|a|&a.format.codec),"sample_rate":a.map(|a|a.format.sample_rate),"bits":a.map(|a|a.format.bit_depth),"output_rate":a.map(|a|a.format.output_rate),"played_seconds":self.position(),"underrun_samples":a.map(|a|a.stats.underrun_samples.load(Ordering::Relaxed)),"lyrics_synchronized":self.lyrics.synced,"lyrics_source":self.lyrics.source,"lyric_lines":self.lyrics.lines.len(),"word_synced":self.lyrics.word_synced(),"discord_status":self.presence_status,"discord_acknowledged":self.presence_status=="Rich Presence active","artwork_textures":self.artwork.len(),"frosted_backdrop_active":self.settings.frosted_glass&&!self.artwork.is_empty(),"floating_lyrics_pid":serde_json::Value::Null,"floating_lyrics_in_process":self.overlay_window.as_ref().is_some_and(|w|w.is_alive()),"display_refresh_hz":self.display_refresh_hz,"renderer":if crate::software_mode(){ "software" }else{ "OpenGL" },"settings":self.settings.diagnostic(),"integrated_plugin_features":16,"all_16_live_validated":false,"limitations":"Account playlist round trips, protected/offline entitlement, remote lyric/translation providers and Discord acknowledgment must be validated with live accounts. Local playback measurements alone do not validate the requested full scenario."});
        let mut details = details;
        details["lyrics_ui_hidden"] =
            serde_json::json!(self.page == Page::Lyrics && self.lyrics_chrome_hidden);
        details["lyric_request_pending"] = serde_json::json!(self.lyric_request.pending);
        details["pcm_packet_allocations"] =
            serde_json::json!(a.map(|a| a.stats.pcm_allocations.load(Ordering::Relaxed)));
        details["decoder_waits"] =
            serde_json::json!(a.map(|a| a.stats.decoder_waits.load(Ordering::Relaxed)));
        details["lyric_jump_trace"] = serde_json::json!(self.lyric_jump_trace);
        details["lyric_jump_active"] = serde_json::json!(self.lyric_jump.is_some());
        details["ui_frames"] = serde_json::json!(self.frame);
        details["retained_lyric_layouts"] = serde_json::json!(self.lyric_galleys.len());
        details["retained_lyric_word_layouts"] = serde_json::json!(
            self.lyric_word_galleys
                .as_ref()
                .map_or(0, |(_, words)| words.len())
        );
        details["lyric_manual_scroll"] = serde_json::json!(self.lyric_manual_scroll);
        details["lyric_scroll_offset"] = serde_json::json!(self.lyric_scroll);
        if let Page::Artist(id) = self.page {
            details["artist_page"] = serde_json::json!(self.artists.get(&id).and_then(|a|a.as_ref().ok()).map(|a| {
                serde_json::json!({"id":id,"name":a.name,"tracks":a.tracks.len(),"albums":a.albums.len(),"singles":a.singles.len(),"fan_count":a.fans,"follow_status_loaded":a.followed.is_some(),"tab":self.artist_tab,"release_view":self.artist_release_view})
            }));
        }
        details["artwork_pipeline"] = self.worker.art.stats();
        details["artwork_scroll_steps"] =
            serde_json::json!(self.art_scroll_check.as_ref().map(|c| &c.records));
        details["retained_render_bytes"] = serde_json::json!({"canvas":self.retained_render_bytes[0],"raster_cache":self.retained_render_bytes[1],"textures":self.retained_render_bytes[2]});
        details["background_benchmark"] =
            serde_json::json!(std::env::args().any(|a| a == "--background-benchmark"));
        details["playback_startup_ms"] = serde_json::json!(self.startup_ms);
        details["consumed_audio_seconds"] = serde_json::json!(
            self.completed_audio_seconds + a.map(Audio::consumed_seconds).unwrap_or(0.)
        );
        details["total_underrun_samples"] = serde_json::json!(
            self.completed_underruns
                + a.map(|audio| audio.stats.underrun_samples.load(Ordering::Relaxed))
                    .unwrap_or(0)
        );
        let report = self.metrics.report(details);
        let _ = crate::config::write_json(&self.report, &report);
    }
}

impl App {
    fn draw(&mut self, ctx: &egui::Context, frame_cpu: f32) {
        self.frame += 1;
        self.art_requests.clear();
        self.art_visible.clear();
        if self.page != self.history_page {
            self.back_history.push(self.history_page.clone());
            if self.back_history.len() > 64 {
                self.back_history.remove(0);
            }
            self.forward_history.clear();
            self.history_page = self.page.clone();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::B)) {
            if !self.settings.show_sidebar {
                self.settings.show_sidebar = true;
                self.settings.sidebar_collapsed = false;
            } else {
                self.settings.sidebar_collapsed = !self.settings.sidebar_collapsed;
            }
        }
        self.drain(ctx);
        if self.startup_started.is_some()
            && self
                .audio
                .as_ref()
                .is_some_and(|a| a.consumed_seconds() > 0.)
            && let Some(started) = self.startup_started.take()
        {
            self.startup_ms
                .push(started.elapsed().as_secs_f64() * 1000.);
        }
        if self.signing_in && self.login_check.elapsed() > Duration::from_secs(1) {
            self.login_check = Instant::now();
            if let Ok(status) = crate::config::read_json::<serde_json::Value>(
                &data_dir().join("signin-status.json"),
            ) {
                self.login_progress = match status["stage"].as_str().unwrap_or("") {
                    "requesting_device_code" => "Getting a sign-in code from TIDAL…",
                    "waiting_for_device_approval" => {
                        "Waiting for you to approve the code on TIDAL's website…"
                    }
                    "waiting_for_pasted_return" => {
                        "Paste the final TIDAL page address into Undertow to finish sign-in…"
                    }
                    "authenticated" => "Checking your account and loading your library…",
                    "cancelled" => "Sign-in cancelled",
                    "callback_state_missing" => {
                        "TIDAL's return omitted the login state; Undertow rejected it safely."
                    }
                    "callback_rejected" | "callback_state_mismatch" | "return_rejected" => {
                        "This return belongs to a different sign-in. Use the latest browser tab."
                    }
                    "callback_delivery_failed" => {
                        "The browser return could not reach this Undertow window."
                    }
                    "callback_received"
                    | "callback_delivered"
                    | "exchanging_code"
                    | "callback_without_state_pkce_required" => {
                        "Browser return received. Verifying the account with TIDAL…"
                    }
                    _ => "Waiting for the browser to open Undertow after sign-in…",
                }
                .into();
            }
        }
        self.metrics.sample(frame_cpu, &[]);
        if self.record_live && self.last_record.elapsed() > Duration::from_secs(15) {
            self.last_record = Instant::now();
            self.save_report();
        }
        let playback_error = self
            .audio
            .as_ref()
            .and_then(|a| a.stats.error.lock().ok()?.take());
        if let Some(error) = playback_error {
            if let Some(a) = &self.audio {
                a.sink.pause();
            }
            self.toast(error, true);
        }
        if let Some(a) = &self.audio
            && a.sink.empty()
            && !a.sink.is_paused()
        {
            if self.repeat_mode == 2 {
                if let Some(t) = self.current.clone() {
                    self.play(t);
                }
            } else {
                self.next();
            }
        }
        let files = ctx.input(|i| i.raw.dropped_files.clone());
        for f in files {
            if let Some(p) = f.path {
                self.open_local(p.display().to_string());
                break;
            }
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::L)) {
            self.page = Page::Lyrics;
            self.now_tab = 2;
            self.tab_started = Instant::now();
        }
        if self.page == Page::Lyrics
            && self.lyrics_chrome_hidden
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.lyrics_chrome_hidden = false;
        }
        if self.page != Page::Lyrics {
            self.lyrics_chrome_hidden = false;
        }
        if !ctx.wants_keyboard_input() {
            ctx.input(|i| {
                if i.key_pressed(egui::Key::Space) {
                    self.toggle();
                }
                if i.modifiers.ctrl && i.key_pressed(egui::Key::Comma) {
                    self.page = Page::Settings;
                }
                if i.modifiers.ctrl && i.key_pressed(egui::Key::ArrowRight) {
                    self.next();
                }
                if i.modifiers.ctrl && i.key_pressed(egui::Key::ArrowLeft) {
                    self.previous();
                }
            });
        }
        if self.connected && self.last_sync.elapsed() > Duration::from_secs(60) {
            self.last_sync = Instant::now();
            self.send(Command::Sync);
            if let Page::Playlist(id) = &self.page {
                self.send(Command::Playlist(id.clone()));
            }
        }
        if self.last_presence.elapsed() > Duration::from_secs(5) {
            self.last_presence = Instant::now();
            let snapshot = PresenceSnapshot {
                settings: self.settings.clone(),
                track: self.current.clone(),
                position: self.position(),
                duration: self.duration(),
                paused: self.paused(),
            };
            let _ = self.presence_tx.try_send(snapshot);
        }
        if self.page != self.motion_page {
            self.motion_page = self.page.clone();
            self.motion_started = Instant::now();
        }
        // Keep audio, requests and floating lyrics serviced, but do not build and
        // tessellate the entire invisible interface while the window is minimized.
        if self.benchmark.is_none() && ctx.input(|i| i.viewport().minimized == Some(true)) {
            if self.settings_dirty
                && self.settings_changed_at.elapsed() >= Duration::from_millis(180)
            {
                self.settings_dirty = false;
                self.send(Command::Settings(self.settings.clone()));
            }
            self.update_overlay();
            ctx.request_repaint_after(Duration::from_millis(200));
            return;
        }
        let transition = if self.settings.animated_lyrics {
            (self.motion_started.elapsed().as_secs_f32() * self.settings.motion_speed / 0.24)
                .clamp(0., 1.)
        } else {
            1.
        };
        let eased = reference::layout::ease(transition);
        self.lyric_frame_motion = false;
        self.background(ctx);
        self.title_bar(ctx);
        self.player(ctx);
        self.sidebar(ctx);
        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(Color32::TRANSPARENT)
                    .inner_margin(Margin::symmetric(
                        if matches!(
                            self.page,
                            Page::Home
                                | Page::Discovery(_)
                                | Page::Mix(_)
                                | Page::Favorites
                                | Page::Lyrics
                                | Page::Playlist(_)
                                | Page::Album(..)
                                | Page::Artist(_)
                                | Page::Search
                                | Page::Settings
                        ) {
                            0
                        } else {
                            48
                        },
                        0,
                    )),
            )
            .show(ctx, |ui| {
                let mut screen = ui.max_rect();
                if self.page != Page::Lyrics {
                    screen.max.x -= 12.; // Keep scrollbars clear of the window resize border.
                }
                let offset = if self.page == Page::Lyrics {
                    vec2(0., 36. * (1. - eased))
                } else {
                    vec2(22. * (1. - eased), 0.)
                };
                let mut content = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("animated-page")
                        .max_rect(screen.translate(offset)),
                );
                content.set_clip_rect(screen.intersect(ui.clip_rect()));
                content.set_opacity(0.45 + 0.55 * eased);
                if !matches!(self.page, Page::Lyrics | Page::Artist(_) | Page::Album(..)) {
                    self.toolbar(&mut content);
                }
                let mut docked;
                let window = ctx.content_rect();
                let ui = if self.queue_open
                    && self.page != Page::Lyrics
                    && reference::layout::queue_docked(window)
                {
                    let mut work = content.available_rect_before_wrap();
                    let queue = reference::layout::queue_rect(
                        window,
                        reference::layout::reserved_player(ctx),
                    );
                    work.max.x = work.right().min(queue.left() - 12.);
                    docked = content.new_child(
                        egui::UiBuilder::new()
                            .id_salt("queue-docked-content")
                            .max_rect(work),
                    );
                    docked.set_clip_rect(work.intersect(content.clip_rect()));
                    &mut docked
                } else {
                    &mut content
                };
                match self.page.clone() {
                    Page::Home => self.home_viewport(ui),
                    Page::Discovery(id) => self.reference_discovery(ui, &id),
                    Page::Mix(id) => self.reference_mix(ui, &id),
                    Page::Favorites => self.reference_favorites(ui),
                    Page::Playlist(id) => {
                        let p = self
                            .playlists
                            .iter()
                            .find(|p| p.id == id)
                            .or_else(|| self.discovery.lists.get(&id))
                            .cloned()
                            .unwrap_or_else(|| Playlist {
                                id: id.clone(),
                                ..Default::default()
                            });
                        self.reference_playlist(ui, &p);
                    }
                    Page::Album(id, title) => self.reference_album(ui, id, &title),
                    Page::Artist(id) => self.reference_artist(ui, id),
                    Page::Search => self.reference_search(ui),
                    Page::Lyrics => self.lyrics_page(ui),
                    Page::Settings => self.settings_page(ui),
                    Page::Profile => self.reference_profile(ui),
                }
            });
        if self.settings_dirty
            && !ctx.input(|i| i.pointer.any_down())
            && self.settings_changed_at.elapsed() >= Duration::from_millis(180)
        {
            self.settings_dirty = false;
            self.send(Command::Settings(self.settings.clone()));
        }
        self.reference_queue(ctx);
        self.dialogs(ctx);
        self.overlays(ctx);
        self.search_dropdown(ctx);
        self.resize_edges(ctx);
        self.trim_artwork(0);
        if !self
            .worker
            .art
            .submit(&self.art_requests, self.settings.artwork_cache_mb)
        {
            ctx.request_repaint_after(Duration::from_millis(8));
        }
        self.record_artwork_scroll(ctx);
        if transition < 1. {
            ctx.request_repaint();
        }
        if self.screenshot.is_some()
            && ((!self.screenshot_requested && self.metrics.elapsed() > 5.)
                || (std::env::args().any(|a| a == "--screenshot-series")
                    && self.screenshot_number < 8
                    && self.metrics.elapsed() > (self.screenshot_number + 1) as f64 * 5.))
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            self.screenshot_requested = true;
            self.screenshot_number += 1;
        }
        let screenshots = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| {
                    if let egui::Event::Screenshot { image, .. } = e {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        });
        if let Some(path) = &self.screenshot {
            for image in screenshots {
                let rgba = image
                    .pixels
                    .iter()
                    .flat_map(|c| c.to_array())
                    .collect::<Vec<_>>();
                if std::env::args().any(|a| a == "--screenshot-series") {
                    let numbered = path.with_file_name(format!(
                        "{}-{}.png",
                        path.file_stem().unwrap_or_default().to_string_lossy(),
                        self.screenshot_number
                    ));
                    let _ = image::save_buffer(
                        numbered,
                        &rgba,
                        image.width() as u32,
                        image.height() as u32,
                        image::ColorType::Rgba8,
                    );
                }
                let _ = image::save_buffer(
                    path,
                    &rgba,
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                );
            }
        }
        if self.benchmark.is_some_and(|n| self.metrics.elapsed() >= n) {
            self.save_report();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if ctx.input(|i| {
            i.raw_scroll_delta.length_sq() > 0. || i.smooth_scroll_delta.length_sq() > 0.1
        }) {
            self.ui_motion_until = Instant::now() + Duration::from_millis(220);
        }
        let scrolling = Instant::now() < self.ui_motion_until;
        let animating = self.lyric_frame_motion
            || (self.page == Page::Lyrics
                && self.tab_started.elapsed() < Duration::from_millis(240))
            || scrolling
            || transition < 1.;
        self.metrics.frame(
            animating && self.settings.animated_lyrics,
            ctx.cumulative_frame_nr(),
        );
        if animating
            && self.settings.animated_lyrics
            && (self.settings.display_sync || crate::software_mode())
        {
            ctx.request_repaint();
            return;
        }
        ctx.request_repaint_after(if animating && self.settings.animated_lyrics {
            Duration::from_secs_f64(1.0 / self.settings.fps.clamp(30, 240) as f64)
        } else if self.audio.is_some() && !self.paused() {
            let next = self
                .lyrics
                .next_visual_change(self.lyric_position(), self.settings.animated_lyrics);
            Duration::from_secs_f64(next.filter(|n| *n > 0.).unwrap_or(0.2).clamp(0.001, 0.2))
        } else {
            Duration::from_secs(1)
        });
    }
    fn finish(&mut self) {
        if self.benchmark.is_none() {
            self.worker.settings_closing.store(true, Ordering::Release);
            crate::auth::cancel_login();
            #[cfg(windows)]
            {
                let _ = crate::auth::ProtocolBridge::recover();
            }
            let _ = self.settings.save();
        }
        if self.benchmark.is_some() {
            self.save_report();
        }
        self.overlay_window = None;
    }
    fn update_overlay(&mut self) {
        if self.overlay_window.as_ref().is_some_and(|w| !w.is_alive()) {
            self.overlay_window = None;
            self.settings.floating_lyrics = false;
        }
        if !self.settings.floating_lyrics {
            self.overlay_window = None;
            return;
        }
        if self.overlay_tick.elapsed() < Duration::from_millis(200) {
            return;
        }
        self.overlay_tick = Instant::now();
        let line = self
            .lyrics
            .active(self.lyric_position())
            .and_then(|i| self.lyrics.lines.get(i))
            .map(|l| l.text.clone())
            .unwrap_or_else(|| self.lyrics_status.clone());
        let title = self
            .current
            .as_ref()
            .map(|t| format!("{} · {}", t.title, t.artist))
            .unwrap_or_else(|| "Undertow".into());
        let snapshot = crate::overlay::Snapshot {
            title,
            line,
            color: self.settings.lyric_color,
        };
        if let Some(window) = &self.overlay_window {
            window.update(snapshot);
        } else {
            let visible = !std::env::args().any(|a| a == "--background-benchmark");
            match crate::overlay::Handle::start(snapshot, visible) {
                Ok(window) => self.overlay_window = Some(window),
                Err(error) => {
                    self.settings.floating_lyrics = false;
                    self.toast(format!("Lyrics window: {error}"), true);
                }
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.draw(ctx, frame.info().cpu_usage.unwrap_or(0.0));
    }
    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        self.finish();
    }
}
impl egui_software_backend::App for App {
    fn update(
        &mut self,
        ctx: &egui::Context,
        backend: &mut egui_software_backend::SoftwareBackend,
    ) {
        backend.set_frame_rate_limit(self.settings.fps);
        backend.set_display_sync(self.settings.display_sync);
        self.display_refresh_hz = backend.monitor_refresh_hz();
        self.retained_render_bytes = backend.retained_render_bytes();
        backend.set_capture_frame_time(true);
        self.draw(
            ctx,
            backend.last_frame_time().unwrap_or_default().as_secs_f32(),
        );
    }
    fn on_exit(&mut self, _: &egui::Context) {
        self.finish();
    }
}

fn field(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.add_space(8.);
    muted(ui, label);
    ui.add_space(6.);
    reference::forms::input(ui, value, label, 54., false, 32768);
}

fn style(ctx: &egui::Context) {
    let mut s = (*ctx.style()).clone();
    s.visuals = egui::Visuals::dark();
    s.visuals.override_text_color = Some(TEXT);
    s.visuals.panel_fill = BG;
    s.visuals.window_fill = PANEL;
    s.visuals.window_stroke = Stroke::new(1.0_f32, Color32::from_white_alpha(22));
    s.visuals.extreme_bg_color = design::SURFACE;
    s.visuals.faint_bg_color = design::SIDEBAR;
    s.visuals.hyperlink_color = design::BRAND;
    s.visuals.widgets.noninteractive.bg_stroke =
        Stroke::new(1.0_f32, Color32::from_white_alpha(14));
    // Every stock widget (ui.button, selectable labels, combo boxes, checkboxes,
    // sliders) picks these up, so buttons without a custom painter still match.
    let radius = egui::CornerRadius::same(design::CONTROL);
    let widgets = &mut s.visuals.widgets;
    for w in [
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
        &mut widgets.open,
    ] {
        w.corner_radius = radius;
    }
    widgets.inactive.weak_bg_fill = BUTTON;
    widgets.inactive.bg_fill = design::SURFACE;
    widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_white_alpha(16));
    widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    widgets.hovered.weak_bg_fill = BUTTON_HOVER;
    widgets.hovered.bg_fill = design::HOVER;
    widgets.hovered.bg_stroke = Stroke::new(1.0_f32, design::BRAND.gamma_multiply(0.6));
    widgets.hovered.fg_stroke = Stroke::new(1.5_f32, Color32::WHITE);
    widgets.hovered.expansion = 0.;
    widgets.active.weak_bg_fill = design::SELECTED;
    widgets.active.bg_fill = design::SELECTED;
    widgets.active.bg_stroke = Stroke::new(1.0_f32, design::BRAND);
    widgets.active.fg_stroke = Stroke::new(1.5_f32, Color32::WHITE);
    widgets.active.expansion = 0.0;
    widgets.open.weak_bg_fill = BUTTON_HOVER;
    widgets.open.bg_stroke = Stroke::new(1.0_f32, ACCENT.gamma_multiply(0.55));
    s.visuals.selection.bg_fill = design::SELECTED;
    s.visuals.selection.stroke = Stroke::new(1.0_f32, design::BRAND);
    s.visuals.text_cursor.stroke = Stroke::new(2.0_f32, Color32::WHITE);
    s.interaction.selectable_labels = false;
    s.visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
    s.spacing.item_spacing = vec2(10., 8.);
    s.spacing.button_padding = vec2(16., 8.);
    s.spacing.slider_width = 100.;
    s.spacing.scroll = egui::style::ScrollStyle::floating();
    s.visuals.window_corner_radius = design::PANEL.into();
    s.visuals.menu_corner_radius = design::CONTROL.into();
    s.visuals.popup_shadow = egui::epaint::Shadow {
        offset: [0, 10],
        blur: 32,
        spread: 0,
        color: Color32::from_black_alpha(150),
    };
    s.animation_time = 0.12;
    ctx.set_style(s);
}
// Installed fonts stay file-backed and shared across font styles. The same
// complete font data and fallback order remain available at every DPI.
fn system_font(file: &str) -> Option<&'static [u8]> {
    use std::sync::OnceLock;
    type FontBytes = OnceLock<Option<crate::font_data::FontBytes>>;
    static REGULAR: FontBytes = OnceLock::new();
    static CJK: FontBytes = OnceLock::new();
    let slot = match file {
        "segoeui.ttf" => &REGULAR,
        "cjk" => &CJK,
        _ => return None,
    };
    slot.get_or_init(|| {
        let files: &[&str] = if file == "cjk" {
            &["malgun.ttf", "YuGothM.ttc", "msyh.ttc"]
        } else {
            std::slice::from_ref(&file)
        };
        files.iter().find_map(|name| {
            let path = std::path::Path::new("C:\\Windows\\Fonts").join(name);
            crate::font_data::FontBytes::open(&path)
        })
    })
    .as_ref()
    .map(crate::font_data::FontBytes::as_slice)
}

pub(crate) fn load_font(ctx: &egui::Context, path: &str) -> bool {
    let use_default = path.is_empty();
    let font_data = if use_default {
        egui::FontData::from_static(include_bytes!("../assets/tidal-ui/nationale-regular.ttf"))
    } else {
        let Ok(meta) = std::fs::metadata(path) else {
            return false;
        };
        if meta.len() > 8 * 1024 * 1024 {
            return false;
        }
        let Ok(data) = std::fs::read(path) else {
            return false;
        };
        if ab_glyph::FontRef::try_from_slice(&data).is_err() {
            return false;
        }
        egui::FontData::from_owned(data)
    };
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert("user".into(), font_data.into());
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "user".into());
    if let Some(bytes) = system_font("segoeui.ttf") {
        fonts.font_data.insert(
            "system-ui".into(),
            egui::FontData::from_static(bytes).into(),
        );
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .push("system-ui".into());
    }
    // One installed fallback covers Japanese, Korean and common CJK characters.
    // Share its data across styles and avoid loading multiple large CJK files.
    if let Some(bytes) = system_font("cjk") {
        fonts
            .font_data
            .insert("cjk".into(), egui::FontData::from_static(bytes).into());
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .push("cjk".into());
    }
    let fallbacks = fonts.families[&egui::FontFamily::Proportional].clone();
    for (family, key, bytes) in [
        (
            "Medium",
            "medium",
            include_bytes!("../assets/tidal-ui/nationale-medium.ttf").as_slice(),
        ),
        (
            "Bold",
            "bold",
            include_bytes!("../assets/tidal-ui/nationale-demibold.ttf").as_slice(),
        ),
    ] {
        let font_key = if use_default {
            fonts
                .font_data
                .insert(key.into(), egui::FontData::from_static(bytes).into());
            key
        } else {
            "user"
        };
        let mut names = vec![font_key.into()];
        names.extend(
            fallbacks
                .iter()
                .filter(|name| name.as_str() != font_key)
                .cloned(),
        );
        fonts
            .families
            .insert(egui::FontFamily::Name(family.into()), names);
    }
    ctx.set_fonts(fonts);
    true
}

#[cfg(test)]
mod performance_tests {
    #[test]
    fn white_primary_button_paints_dark_text() {
        use eframe::egui;
        let ctx = egui::Context::default();
        assert!(super::load_font(&ctx, ""));
        let frame = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                super::pill(ui, "Top results", true);
            });
        });
        assert!(frame.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Text(text)
                if text.override_text_color == Some(super::design::INK))
        }));
    }

    #[test]
    fn retained_word_layouts_preserve_wrap_clip_glow_and_hit_rectangles() {
        use eframe::egui::*;
        use egui_software_backend::{BufferMutRef, ColorFieldOrder, EguiSoftwareRender};
        let render = |cached: bool, width: usize, fraction: f32| {
            let ctx = Context::default();
            assert!(super::load_font(&ctx, ""));
            let mut rects = Vec::new();
            let output = ctx.run(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(width as f32, 240.))),
                    ..Default::default()
                },
                |ctx| {
                    CentralPanel::default().show(ctx, |ui| {
                        let words = ["Flowing ", "lyrics ", "日本語 ", "stay ", "responsive"];
                        let layouts: Vec<_> = if cached {
                            words
                                .iter()
                                .map(|text| {
                                    crate::lyric_fonts::layout(
                                        ctx,
                                        text,
                                        FontId::new(38., FontFamily::Name("Bold".into())),
                                        f32::INFINITY,
                                    )
                                })
                                .collect()
                        } else {
                            vec![]
                        };
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.;
                            for (i, text) in words.iter().enumerate() {
                                let word = if cached {
                                    super::lyric_galley(
                                        ui,
                                        layouts[i].clone(),
                                        Color32::from_white_alpha(125),
                                        None,
                                        None,
                                    )
                                } else {
                                    super::lyric_text(
                                        ui,
                                        text,
                                        38.,
                                        Color32::from_white_alpha(125),
                                        None,
                                        false,
                                        None,
                                    )
                                };
                                rects.push(word.rect);
                                let clip = Rect::from_min_max(
                                    word.rect.min - vec2(6., 7.),
                                    pos2(
                                        word.rect.left() + word.rect.width() * fraction,
                                        word.rect.bottom() + 7.,
                                    ),
                                );
                                let painter =
                                    ui.painter().with_clip_rect(ui.clip_rect().intersect(clip));
                                let layout = if cached {
                                    layouts[i].clone()
                                } else {
                                    crate::lyric_fonts::layout(
                                        ctx,
                                        text,
                                        FontId::new(38., FontFamily::Name("Bold".into())),
                                        f32::INFINITY,
                                    )
                                };
                                if fraction > 0. {
                                    super::paint_lyric_galley(
                                        &painter,
                                        word.rect.min,
                                        layout,
                                        Color32::WHITE,
                                        Some(Color32::from_white_alpha(110)),
                                    );
                                }
                            }
                        });
                    });
                },
            );
            let jobs = ctx.tessellate(output.shapes, output.pixels_per_point);
            let mut renderer = EguiSoftwareRender::new(ColorFieldOrder::Rgba);
            let mut pixels = vec![[0; 4]; width * 240];
            renderer.render(
                &mut BufferMutRef::new(&mut pixels, width, 240),
                &jobs,
                &output.textures_delta,
                1.,
            );
            (rects, pixels)
        };
        for width in [300, 640] {
            for fraction in [0., 0.35, 1.] {
                assert_eq!(
                    render(true, width, fraction),
                    render(false, width, fraction)
                );
            }
        }
    }
    #[test]
    fn retained_white_glyphs_preserve_the_current_text_color_pixel_for_pixel() {
        use eframe::egui::*;
        use egui_software_backend::{BufferMutRef, ColorFieldOrder, EguiSoftwareRender};
        let render = |cached: bool, color: Color32| {
            let ctx = Context::default();
            assert!(super::load_font(&ctx, ""));
            let output = ctx.run(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(480., 160.))),
                    ..Default::default()
                },
                |ctx| {
                    CentralPanel::default().show(ctx, |ui| {
                        let galley = ui.painter().layout(
                            "Glow and color".into(),
                            FontId::new(38., FontFamily::Name("Bold".into())),
                            if cached { Color32::WHITE } else { color },
                            420.,
                        );
                        if cached {
                            super::lyric_galley(
                                ui,
                                galley,
                                color,
                                Some(Color32::from_white_alpha(110)),
                                None,
                            );
                        } else {
                            let (rect, _) = ui.allocate_exact_size(galley.size(), Sense::click());
                            super::paint_lyric_galley(
                                ui.painter(),
                                rect.min,
                                galley,
                                color,
                                Some(Color32::from_white_alpha(110)),
                            );
                        }
                    });
                },
            );
            let jobs = ctx.tessellate(output.shapes, output.pixels_per_point);
            let mut renderer = EguiSoftwareRender::new(ColorFieldOrder::Rgba);
            let mut pixels = vec![[0; 4]; 480 * 160];
            renderer.render(
                &mut BufferMutRef::new(&mut pixels, 480, 160),
                &jobs,
                &output.textures_delta,
                1.,
            );
            pixels
        };
        for color in [
            Color32::WHITE,
            Color32::from_white_alpha(155),
            Color32::from_rgb(200, 80, 160),
        ] {
            assert_eq!(render(true, color), render(false, color));
        }
    }

    #[test]
    fn old_lyric_refresh_cannot_overwrite_or_unlock_a_newer_request() {
        let mut requests = super::LyricRequest::default();
        let old = requests.begin();
        let refresh = requests.begin();
        assert!(!requests.finish(old));
        assert!(requests.pending);
        assert!(requests.finish(refresh));
        assert!(!requests.pending);
        let translation = requests.begin();
        assert!(!requests.finish(refresh));
        assert!(requests.pending);
        assert!(requests.finish(translation));
    }

    #[test]
    fn progressive_lyric_glow_keeps_the_original_word_click_target() {
        use eframe::egui::*;
        let ctx = Context::default();
        assert!(super::load_font(&ctx, ""));
        let position = pos2(50., 30.);
        let mut clicked = false;
        for (frame, pressed) in [None, Some(true), Some(false)].into_iter().enumerate() {
            let mut events = vec![Event::PointerMoved(position)];
            if let Some(pressed) = pressed {
                events.push(Event::PointerButton {
                    pos: position,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                });
            }
            let _ = ctx.run(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(500., 200.))),
                    time: Some(frame as f64 / 120.),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    CentralPanel::default().show(ctx, |ui| {
                        let word =
                            super::lyric_text(ui, "Timing", 48., Color32::GRAY, None, false, None);
                        let painter = ui.painter().with_clip_rect(Rect::from_min_max(
                            word.rect.min,
                            word.rect.center_bottom(),
                        ));
                        let galley = painter.layout_no_wrap(
                            "Timing".into(),
                            FontId::new(48., FontFamily::Name("Bold".into())),
                            Color32::WHITE,
                        );
                        super::paint_lyric_galley(
                            &painter,
                            word.rect.min,
                            galley,
                            Color32::WHITE,
                            Some(Color32::LIGHT_BLUE),
                        );
                        clicked |= word.clicked();
                    });
                },
            );
        }
        assert!(
            clicked,
            "A decorative highlight must not steal click-to-seek from the word"
        );
    }
    fn add_art(
        ctx: &eframe::egui::Context,
        cache: &mut std::collections::HashMap<String, super::Artwork>,
        key: &str,
        touched: u64,
    ) {
        let tex = ctx.load_texture(
            key,
            eframe::egui::ColorImage::filled([640, 640], super::TEXT),
            Default::default(),
        );
        cache.insert(
            key.into(),
            super::Artwork {
                texture: tex.clone(),
                blur: tex,
                palette: [0; 3],
                touched,
            },
        );
    }
    #[test]
    fn artwork_byte_budget_evicts_inactive_covers_but_keeps_visible_covers() {
        let ctx = eframe::egui::Context::default();
        let mut cache = std::collections::HashMap::new();
        add_art(&ctx, &mut cache, "visible", 100);
        add_art(&ctx, &mut cache, "older", 1);
        add_art(&ctx, &mut cache, "old", 2);
        let mut requested = cache.keys().cloned().collect();
        super::trim_artwork_cache(&mut cache, &mut requested, 100, 0);
        assert!(cache.contains_key("visible"));
        assert!(!cache.contains_key("older"));
        assert!(!requested.contains("older"));
    }
    #[test]
    fn cache_does_not_evict_visible_covers_to_force_the_memory_budget() {
        let ctx = eframe::egui::Context::default();
        let mut cache = std::collections::HashMap::new();
        for key in ["a", "b", "c"] {
            add_art(&ctx, &mut cache, key, 100);
        }
        let mut requested = cache.keys().cloned().collect();
        super::trim_artwork_cache(&mut cache, &mut requested, 100, 1_600_000);
        assert_eq!(cache.len(), 3);
    }
    #[test]
    fn artwork_resolution_preserves_physical_pixel_density() {
        assert_eq!(super::artwork_side(54., 1.), 80);
        assert_eq!(super::artwork_side(54., 2.), 160);
        assert_eq!(super::artwork_side(288., 1.), 320);
        assert_eq!(super::artwork_side(288., 2.), 640);
        assert_eq!(super::artwork_side(1000., 1.), 640);
    }
}
