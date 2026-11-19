//! Generated, isolated UI evidence. Called only for background benchmarks.
use super::*;
impl App {
    pub(super) fn preview_audit(&mut self, args: &[String]) {
        let Some(screen) = args
            .iter()
            .position(|s| s == "--preview-screen")
            .and_then(|i| args.get(i + 1))
        else {
            return;
        };
        let mut fixture_args = args.to_vec();
        fixture_args.push("--preview-library".into());
        self.preview_library(&fixture_args);
        let Some(local) = self.current.clone() else {
            return;
        };
        let cover_id = "audit-cover";
        if let Some(source) = local.cover.strip_prefix("local:")
            && let Ok(path) = crate::config::playlist_art_path(cover_id)
        {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::copy(source, path);
        }
        let cover = format!("local-playlist:{cover_id}");
        if matches!(screen.as_str(), "search" | "search-preview") {
            for item in Arc::make_mut(&mut self.catalog) {
                item.cover.clone_from(&cover);
            }
        }
        let album = Album {
            id: 20,
            title: "Evening Sessions".into(),
            artist: "Northbound".into(),
            cover: cover.clone(),
            year: "2026".into(),
            quality: "HI_RES_LOSSLESS".into(),
            ..Default::default()
        };
        self.albums.insert(20, album.clone());
        let tracks = self.tracks.get("favorites").cloned().unwrap_or_default();
        self.tracks.insert("album:20".into(), tracks.clone());
        self.cache_artist(10,Ok(Arc::new(Artist{id:10,name:"Northbound".into(),picture:cover.clone(),tracks:tracks.clone(),albums:vec![album.clone();6],singles:vec![album;4],biography:"Generated artist biography for repeatable interface checks. All artwork and audio are original local test fixtures.".into(),followed:Some(false),fans:Some(1200),..Default::default()})));
        self.credits.insert(
            local.id,
            Ok(vec![
                CreditGroup {
                    role: "Producer".into(),
                    names: "Example producer".into(),
                },
                CreditGroup {
                    role: "Engineer".into(),
                    names: "Example engineer".into(),
                },
            ]),
        );
        self.related_tracks.insert(local.id, Ok(tracks.clone()));
        let items = |kind: &str| {
            (0..12).map(|i|serde_json::json!({"type":kind,"data":{"id":20+i,"title":format!("Evening Sessions {}",i+1),"name":format!("Northbound {}",i+1),"cover":cover,"picture":cover,"artists":[{"name":"Northbound"}]}})).collect::<Vec<_>>()
        };
        let shortcuts = serde_json::json!([
            {"type":"DEEP_LINK","data":{"url":"tidal://my-collection/tracks","title":"My Tracks"}},
            {"type":"ARTIST","data":{"id":10,"name":"Northbound","picture":cover}},
            {"type":"PLAYLIST","data":{"uuid":"preview-playlist","title":"Evening rotation","squareImage":cover,"creator":{"name":"Example"}}},
            {"type":"ALBUM","data":{"id":20,"title":"Evening Sessions","cover":cover,"artists":[{"name":"Northbound"}]}},
            {"type":"ARTIST","data":{"id":11,"name":"Stillwater","picture":cover}},
            {"type":"ALBUM","data":{"id":21,"title":"Harbor Lights","cover":cover,"artists":[{"name":"Stillwater"}]}}
        ]);
        self.discovery.home_result(Ok(crate::discovery::feed(&serde_json::json!({"items":[
            {"type":"SHORTCUT_LIST","moduleId":"shortcuts","title":"Shortcuts","items":shortcuts},
            {"type":"HORIZONTAL_LIST","moduleId":"albums","title":"Albums you’ll enjoy","items":items("ALBUM")},
            {"type":"HORIZONTAL_LIST","moduleId":"CONTINUE_LISTEN_TO","title":"Recently played","items":shortcuts},
            {"type":"HORIZONTAL_LIST","moduleId":"artists","title":"Artists to explore","items":items("ARTIST")}
        ]}))));
        self.settings.profile_name = "Example listener".into();
        self.last_sync = Instant::now();
        self.busy = false;
        self.page = match screen.as_str() {
            "home" => Page::Home,
            "playlist" => Page::Playlist("preview-playlist".into()),
            "album" => Page::Album(20, "Evening Sessions".into()),
            "artist" => Page::Artist(10),
            "search" | "search-preview" => {
                self.search_preview = screen == "search-preview";
                Page::Search
            }
            "lyrics" | "credits" | "similar" => {
                self.now_tab = match screen.as_str() {
                    "credits" => 1,
                    "similar" => 0,
                    _ => 2,
                };
                Page::Lyrics
            }
            "general" | "account" | "plugins" | "about" => {
                self.settings_tab = match screen.as_str() {
                    "account" => 1,
                    "plugins" => 2,
                    "about" => 3,
                    _ => 0,
                };
                Page::Settings
            }
            "profile" => Page::Profile,
            "create" | "edit" | "delete" => {
                self.dialog = Some(Dialog {
                    kind: screen.clone(),
                    id: if screen == "create" {
                        String::new()
                    } else {
                        "preview-playlist".into()
                    },
                    title: if screen == "create" {
                        String::new()
                    } else {
                        "Evening rotation".into()
                    },
                    description: "Original generated playlist for UI checks.".into(),
                    ..Default::default()
                });
                Page::Favorites
            }
            "queue" | "saved" => {
                self.queue = Arc::new(tracks.iter().take(12).cloned().collect());
                self.play_history = Arc::new(tracks.iter().skip(12).take(8).cloned().collect());
                self.saved_later = Arc::new(tracks.iter().take(4).cloned().collect());
                self.queue_open = true;
                self.show_saved_later = screen == "saved";
                Page::Favorites
            }
            "volume" => {
                self.volume_pinned = true;
                Page::Favorites
            }
            "error" => {
                self.toast("This is a generated recoverable error. Try again when the connection is available.",true);
                Page::Favorites
            }
            _ => Page::Favorites,
        };
    }
    pub(super) fn preview_discovery(&mut self, args: &[String]) {
        let value = |flag: &str| {
            args.iter()
                .position(|s| s == flag)
                .and_then(|i| args.get(i + 1))
        };
        let Some(path) = value("--preview-discovery") else {
            return;
        };
        let load = |path: &str| -> Option<serde_json::Value> {
            let file = std::fs::File::open(path).ok()?;
            if file.metadata().ok()?.len() > 8 * 1024 * 1024 {
                return None;
            }
            serde_json::from_reader(file).ok()
        };
        let Some(v) = load(path) else {
            return;
        };
        self.discovery.home_result(Ok(crate::discovery::feed(&v)));
        self.connected = true;
        self.page = Page::Home;
        if let Some(path) = value("--preview-discovery-section")
            && let Some(document) = load(path)
            && let Some(source) = self
                .discovery
                .feed
                .sections
                .iter()
                .find(|s| !s.shortcuts)
                .cloned()
        {
            let mut section = source;
            section.items = crate::discovery::section(&document, &section.id).items;
            self.page = Page::Discovery(section.id.clone());
            self.discovery
                .section_result(section.id.clone(), Ok(section));
        }
        if let Some(offset) = value("--preview-home-offset").and_then(|s| s.parse::<f32>().ok()) {
            self.home_scroll = offset;
            self.home_content_height = 10000.;
        }
    }
    pub(super) fn preview_lyric_navigation(&mut self, ctx: &egui::Context) {
        static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if self.benchmark.is_none()
            || !*ENABLED.get_or_init(|| {
                std::env::args().any(|a| a == "--background-benchmark")
                    && std::env::args()
                        .any(|a| a == "--preview-lyric-navigation" || a == "--preview-lyric-clicks")
            })
        {
            return;
        }
        static CLICKS: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if *CLICKS.get_or_init(|| std::env::args().any(|a| a == "--preview-lyric-clicks")) {
            let time = self.metrics.elapsed();
            let id = Id::new("lyric-click-preview-step");
            let step = ctx.data_mut(|d| *d.get_temp_mut_or_default::<u8>(id));
            let action = match step {
                0 if time > 2.2 => Some(20.),
                1 if time > 4.0 => Some(8.),
                2 if time > 5.0 => Some(22.),
                _ => None,
            };
            if let Some(seconds) = action {
                self.seek_lyric(seconds);
                ctx.data_mut(|d| d.insert_temp(id, step + 1));
            } else if step == 3 && time > 6.5 {
                self.lyric_manual_scroll = true;
                self.lyric_jump = None;
                self.lyric_scroll = 500.;
                ctx.data_mut(|d| d.insert_temp(id, 4_u8));
            } else if step == 4 && time > 7.0 {
                self.resync_lyric_view();
                ctx.data_mut(|d| d.insert_temp(id, 5_u8));
            }
            if self.lyric_jump_trace.len() < 512
                && self
                    .lyric_jump_trace
                    .last()
                    .is_none_or(|r| time - r.0 > 0.02)
            {
                self.lyric_jump_trace.push((
                    time,
                    self.lyric_position(),
                    self.lyric_scroll,
                    self.lyric_jump.is_some(),
                    self.audio
                        .as_ref()
                        .map_or(0, |a| a.stats.underrun_samples.load(Ordering::Relaxed)),
                ));
            }
            return;
        }
        let id = Id::new("lyric-navigation-preview-step");
        let step = ctx.data_mut(|d| *d.get_temp_mut_or_default::<u8>(id));
        if step == 0 && self.metrics.elapsed() > 2. {
            self.lyric_manual_scroll = true;
            self.lyric_scroll = 1000.;
            self.lyric_velocity = 0.;
            ctx.data_mut(|d| d.insert_temp(id, 1_u8));
        } else if step == 1 && self.metrics.elapsed() > 7. {
            self.resync_lyric_view();
            ctx.data_mut(|d| d.insert_temp(id, 2_u8));
        }
    }
    pub(super) fn preview_library(&mut self, args: &[String]) {
        if !args.iter().any(|a| a == "--preview-library") {
            return;
        }
        let Some(local) = self.current.clone() else {
            return;
        };
        let names = [
            "Night Swim",
            "First Light",
            "Afterglow",
            "Quiet Hours",
            "Blue Hour",
            "City Lights",
        ];
        let tracks = Arc::new(
            (0..1200)
                .map(|i| Track {
                    id: if i == 0 { 0 } else { 100_000 + i },
                    title: format!("{} {:04}", names[i as usize % names.len()], i + 1),
                    artist: ["Northbound", "Stillwater", "Low Tide"][i as usize % 3].into(),
                    artist_id: 10 + i % 3,
                    album: format!("Evening Sessions {}", 1 + i % 12),
                    album_id: 20 + i % 12,
                    duration: 180 + i % 90,
                    playlist_index: Some(i as usize),
                    date_added: "2026-11-09T12:00:00Z".into(),
                    bit_depth: Some(if i % 3 == 0 { 24 } else { 16 }),
                    sample_rate: Some(44_100),
                    bitrate: Some(960_000 + i * 100),
                    ..local.clone()
                })
                .collect::<Vec<_>>(),
        );
        self.favorite_ids = tracks.iter().map(|t| t.id).collect();
        self.tracks.insert("favorites".into(), tracks.clone());
        self.tracks
            .insert("preview-playlist".into(), tracks.clone());
        self.playlists = Arc::new(vec![Playlist {
            id: "preview-playlist".into(),
            title: "Evening rotation".into(),
            description: "Generated UI fixture — no account or service writes.".into(),
            image: local.cover.clone(),
            count: tracks.len() as u64,
            duration: tracks.iter().map(|t| t.duration).sum(),
            owned: true,
            creator: "Undertow preview".into(),
            public: Some(false),
        }]);
        self.connected = true;
        self.account = "Local preview fixture".into();
        self.page = if args.iter().any(|a| a == "--preview-home") {
            Page::Home
        } else if args.iter().any(|a| a == "--preview-playlist") {
            Page::Playlist("preview-playlist".into())
        } else if args.iter().any(|a| a == "--preview-search") {
            Page::Search
        } else {
            Page::Favorites
        };
        self.playlist_filter_id = if matches!(self.page, Page::Playlist(_)) {
            "preview-playlist".into()
        } else {
            "favorites".into()
        };
        if let Some(query) = args
            .iter()
            .position(|a| a == "--preview-filter")
            .and_then(|i| args.get(i + 1))
        {
            self.playlist_filter = query.clone();
        }
        self.playlist_sort = Some((1, false));
        self.playlist_index.invalidate();
        self.catalog = Arc::new(
            tracks
                .iter()
                .take(120)
                .enumerate()
                .map(|(i, t)| CatalogItem {
                    kind: if i % 5 == 0 {
                        "Artist"
                    } else if i % 4 == 0 {
                        "Album"
                    } else {
                        "Track"
                    }
                    .into(),
                    id: t.id.to_string(),
                    title: t.title.clone(),
                    artist: t.artist.clone(),
                    cover: t.cover.clone(),
                    year: "2026".into(),
                    explicit: false,
                    track: Some(t.clone()),
                })
                .collect(),
        );
        self.query = "Evening".into();
        self.catalog_query = self.query.clone();
        self.search_sent = self.query.clone();
    }
}
