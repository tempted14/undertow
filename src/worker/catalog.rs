//! Bounded read lanes keep multi-request pages away from playback and edits.
use super::*;

struct Job {
    tidal: Tidal,
    command: Command,
    epoch: u64,
    cancelled: Arc<AtomicBool>,
    search_revision: u64,
    library: bool,
}

pub(super) struct Reader {
    artists: Option<SyncSender<Job>>,
    interactive: Option<SyncSender<Job>>,
    discovery: Option<SyncSender<Job>>,
    library: Option<SyncSender<Job>>,
    library_cancelled: Arc<AtomicBool>,
    cache_writes: Arc<std::sync::Mutex<()>>,
    events: SyncSender<Event>,
    ctx: egui::Context,
    epoch: Arc<AtomicU64>,
    cancelled: Arc<AtomicBool>,
    search_revision: Arc<AtomicU64>,
}

impl Reader {
    pub(super) fn start(
        events: SyncSender<Event>,
        ctx: egui::Context,
        epoch: Arc<AtomicU64>,
    ) -> Self {
        Self {
            artists: None,
            interactive: None,
            discovery: None,
            library: None,
            library_cancelled: Arc::new(AtomicBool::new(false)),
            cache_writes: Default::default(),
            events,
            ctx,
            epoch,
            cancelled: Arc::new(AtomicBool::new(false)),
            search_revision: Arc::new(AtomicU64::new(0)),
        }
    }

    fn lane(&self, name: &str) -> SyncSender<Job> {
        let (tx, rx) = mpsc::sync_channel::<Job>(8);
        let events = self.events.clone();
        let ctx = self.ctx.clone();
        let latest = self.search_revision.clone();
        let cache_writes = self.cache_writes.clone();
        std::thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                while let Ok(mut job) = rx.recv() {
                    if job.cancelled.load(Ordering::Acquire)
                        || (job.search_revision > 0
                            && job.search_revision != latest.load(Ordering::Acquire))
                    {
                        continue;
                    }
                    let deliver = |event| {
                        if !job.cancelled.load(Ordering::Acquire)
                            && (job.search_revision == 0
                                || job.search_revision == latest.load(Ordering::Acquire))
                        {
                            let event = if job.library {
                                Event::LibraryRead(
                                    job.epoch,
                                    job.cancelled.clone(),
                                    Box::new(event),
                                )
                            } else {
                                Event::CatalogRead(job.epoch, Box::new(event))
                            };
                            emit(&events, &ctx, event);
                        }
                    };
                    let result = (|| -> anyhow::Result<()> {
                        match job.command {
                            Command::Sync => {
                                let playlists = playlist_covers(job.tidal.playlists()?);
                                let guard = cache_writes.lock().unwrap();
                                if !job.cancelled.load(Ordering::Acquire) {
                                    crate::config::write_json(
                                        &data_dir().join("playlists.json"),
                                        &playlists,
                                    )?;
                                }
                                drop(guard);
                                deliver(Event::Playlists(playlists));
                            }
                            Command::Favorites => {
                                deliver(Event::Tracks("favorites".into(), job.tidal.favorites()?))
                            }
                            Command::CollectionState => deliver(Event::CollectionState(
                                job.tidal.collection_state().map_err(|e| e.to_string()),
                            )),
                            Command::Home => {
                                deliver(Event::Home(job.tidal.home().map_err(|e| e.to_string())))
                            }
                            Command::DiscoverySection(section) => deliver(Event::DiscoverySection(
                                section.id.clone(),
                                job.tidal
                                    .discovery_section(&section)
                                    .map_err(|e| e.to_string()),
                            )),
                            Command::Mix(id) => deliver(Event::Mix(
                                id.clone(),
                                job.tidal.mix_tracks(&id).map_err(|e| e.to_string()),
                            )),
                            Command::Artist(id) => {
                                let result = job
                                    .tidal
                                    .artist_with_preview(id, |artist| {
                                        deliver(Event::ArtistPreview(id, Box::new(artist.clone())));
                                    })
                                    .map(Box::new)
                                    .map_err(|e| e.to_string());
                                deliver(Event::Artist(id, result));
                            }
                            Command::Album(id) => {
                                let tracks = job.tidal.album_tracks(id)?;
                                // Deliver playable rows before the supplementary metadata request.
                                deliver(Event::Tracks(format!("album:{id}"), tracks));
                                if let Ok(info) = job.tidal.album_info(id) {
                                    deliver(Event::AlbumInfo(Box::new(info)));
                                }
                            }
                            Command::CatalogSearch(query) => {
                                let items = job.tidal.catalog_search(&query)?;
                                deliver(Event::Catalog(query, items));
                            }
                            Command::Credits(track) => deliver(Event::Credits(
                                track.id,
                                job.tidal.credits(&track).map_err(|e| e.to_string()),
                            )),
                            Command::Similar(id) => deliver(Event::Similar(
                                id,
                                job.tidal.similar_tracks(id).map_err(|e| e.to_string()),
                            )),
                            _ => unreachable!("only read commands enter catalog lanes"),
                        }
                        Ok(())
                    })();
                    if let Err(error) = result {
                        deliver(Event::Error(error.to_string()));
                    }
                }
            })
            .expect("catalog read worker");
        tx
    }

    pub(super) fn invalidate_library(&mut self) {
        // Serialize invalidation with cache commits: an old account/read can never
        // write its cache after a sign-out or overwrite a mutation's fresh state.
        let _guard = self.cache_writes.lock().unwrap();
        self.library_cancelled.store(true, Ordering::Release);
        self.library_cancelled = Arc::new(AtomicBool::new(false));
    }
    pub(super) fn reset(&mut self) {
        self.invalidate_library();
        self.cancelled.store(true, Ordering::Release);
        self.epoch.fetch_add(1, Ordering::AcqRel);
        self.cancelled = Arc::new(AtomicBool::new(false));
        self.artists = None;
        self.interactive = None;
        self.discovery = None;
        self.library = None;
    }

    pub(super) fn submit(&mut self, tidal: &Tidal, command: Command) {
        let search_revision = if matches!(command, Command::CatalogSearch(_)) {
            self.search_revision.fetch_add(1, Ordering::AcqRel) + 1
        } else {
            0
        };
        let library = matches!(
            command,
            Command::Sync | Command::Favorites | Command::CollectionState
        );
        let cancelled = if library {
            self.library_cancelled.clone()
        } else {
            self.cancelled.clone()
        };
        let lane = if library {
            if self.library.is_none() {
                self.library = Some(self.lane("library-refresh"));
            }
            self.library.as_ref().unwrap()
        } else if matches!(
            command,
            Command::Home | Command::DiscoverySection(_) | Command::Mix(_)
        ) {
            if self.discovery.is_none() {
                self.discovery = Some(self.lane("tidal-discovery"));
            }
            self.discovery.as_ref().unwrap()
        } else if matches!(command, Command::Artist(_)) {
            if self.artists.is_none() {
                self.artists = Some(self.lane("artist-metadata"));
            }
            self.artists.as_ref().unwrap()
        } else {
            if self.interactive.is_none() {
                self.interactive = Some(self.lane("catalog-interactive"));
            }
            self.interactive.as_ref().unwrap()
        };
        let job = Job {
            tidal: tidal.fork_read(cancelled.clone()),
            command,
            epoch: self.epoch.load(Ordering::Acquire),
            cancelled,
            search_revision,
            library,
        };
        if let Err(error) = lane.try_send(job) {
            let (mpsc::TrySendError::Full(job) | mpsc::TrySendError::Disconnected(job)) = error;
            let message = "Please wait for the current catalog requests, then try again".to_owned();
            let event = match job.command {
                Command::Home => Event::Home(Err(message)),
                Command::CollectionState => Event::CollectionState(Err(message)),
                Command::Mix(id) => Event::Mix(id, Err(message)),
                Command::DiscoverySection(section) => {
                    Event::DiscoverySection(section.id, Err(message))
                }
                Command::Artist(id) => Event::Artist(id, Err(message)),
                Command::Credits(track) => Event::Credits(track.id, Err(message)),
                Command::Similar(id) => Event::Similar(id, Err(message)),
                _ => Event::Error(message),
            };
            let event = if job.library {
                Event::LibraryRead(job.epoch, job.cancelled, Box::new(event))
            } else {
                Event::CatalogRead(job.epoch, Box::new(event))
            };
            emit(&self.events, &self.ctx, event);
        }
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.library_cancelled.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use serde_json::json;
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        time::Duration,
    };

    const WAIT: Duration = Duration::from_secs(5);

    fn request(stream: &mut TcpStream) -> String {
        stream.set_read_timeout(Some(WAIT)).unwrap();
        let mut bytes = Vec::new();
        let mut byte = [0];
        while !bytes.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            bytes.push(byte[0]);
        }
        String::from_utf8(bytes).unwrap()
    }
    fn respond(stream: &mut TcpStream, body: serde_json::Value) {
        let body = body.to_string();
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    }
    fn tidal(base: String) -> Tidal {
        let mut t = Tidal::new(
            auth::Session {
                access_token: "test-only".into(),
                refresh_token: String::new(),
                expires_at: u64::MAX,
                user_id: 1,
                country: "CA".into(),
                compatibility: true,
                client_id: "test-only".into(),
            },
            Settings::default(),
        )
        .unwrap();
        t.test_base = Some(base);
        t
    }
    fn receive(rx: &Receiver<Event>, epoch: &AtomicU64) -> Event {
        rx.recv_timeout(WAIT)
            .unwrap()
            .for_catalog_epoch(epoch.load(Ordering::Acquire))
            .unwrap()
    }

    #[test]
    fn refresh_already_queued_for_ui_is_rejected_after_a_mutation() {
        let (events, rx) = mpsc::sync_channel(1);
        let epoch = Arc::new(AtomicU64::new(0));
        let mut reads = Reader::start(events.clone(), egui::Context::default(), epoch.clone());
        let previous = reads.library_cancelled.clone();
        events
            .send(Event::LibraryRead(
                0,
                previous,
                Box::new(Event::Playlists(vec![])),
            ))
            .unwrap();
        reads.invalidate_library();
        assert!(rx.recv().unwrap().for_catalog_epoch(0).is_none());
        let fresh = Event::LibraryRead(
            0,
            reads.library_cancelled.clone(),
            Box::new(Event::Playlists(vec![])),
        );
        assert!(matches!(
            fresh.for_catalog_epoch(0),
            Some(Event::Playlists(_))
        ));
        assert!(matches!(
            Event::CatalogRead(0, Box::new(Event::Notice("catalog".into()))).for_catalog_epoch(0),
            Some(Event::Notice(_))
        ));
        assert!(
            Event::LibraryRead(
                0,
                reads.library_cancelled.clone(),
                Box::new(Event::Playlists(vec![]))
            )
            .for_catalog_epoch(1)
            .is_none()
        );
    }

    #[test]
    fn blocked_library_refresh_does_not_block_playback_and_cannot_overwrite_later_edits() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut foreground = tidal(format!("http://{}/", listener.local_addr().unwrap()));
        let (seen_tx, seen_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut blocked, _) = listener.accept().unwrap();
            assert!(request(&mut blocked).starts_with("GET /users/1/playlists?"));
            let sync = std::thread::spawn(move || {
                seen_tx.send(()).unwrap();
                release_rx.recv_timeout(WAIT).unwrap();
                respond(&mut blocked, json!({"items":[],"totalNumberOfItems":0}));
            });
            let (mut audio, _) = listener.accept().unwrap();
            assert!(request(&mut audio).starts_with("GET /tracks/3/playbackinfopostpaywall?"));
            let manifest = base64::engine::general_purpose::STANDARD.encode(
                json!({"codecs":"flac","urls":["https://audio.tidal.com/test.flac"]}).to_string(),
            );
            respond(
                &mut audio,
                json!({"manifest":manifest,"manifestMimeType":"application/vnd.tidal.bts","audioQuality":"LOSSLESS"}),
            );
            sync.join().unwrap();
        });
        let (events, rx) = mpsc::sync_channel(24);
        let epoch = Arc::new(AtomicU64::new(0));
        let mut reads = Reader::start(events, egui::Context::default(), epoch);
        reads.submit(&foreground, Command::Sync);
        seen_rx.recv_timeout(WAIT).unwrap();
        assert_eq!(foreground.stream(3, false).unwrap().codec, "flac");
        // A playlist edit invalidates an in-flight refresh before its cache commit.
        reads.invalidate_library();
        release_tx.send(()).unwrap();
        server.join().unwrap();
        assert!(rx.recv_timeout(Duration::from_millis(30)).is_err());
    }
    #[test]
    fn slow_artist_does_not_block_search_or_playback_manifest() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut foreground = tidal(format!("http://{}/", listener.local_addr().unwrap()));
        let (seen_tx, seen_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut blocked, _) = listener.accept().unwrap();
            assert!(request(&mut blocked).starts_with("GET /artists/1?"));
            seen_tx.send(()).unwrap();
            let artist = std::thread::spawn(move || {
                release_rx.recv_timeout(WAIT).unwrap();
                respond(&mut blocked, json!({"name":"Slow artist"}));
            });
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let req = request(&mut stream);
                if req.starts_with("GET /search?") {
                    respond(
                        &mut stream,
                        json!({"artists":{"items":[{"id":2,"name":"Fast search"}]}}),
                    );
                } else {
                    assert!(req.starts_with("GET /tracks/3/playbackinfopostpaywall?"));
                    let manifest = base64::engine::general_purpose::STANDARD.encode(
                        json!({"codecs":"flac","urls":["https://audio.tidal.com/test.flac"]})
                            .to_string(),
                    );
                    respond(
                        &mut stream,
                        json!({"manifest":manifest,"manifestMimeType":"application/vnd.tidal.bts","audioQuality":"HI_RES_LOSSLESS"}),
                    );
                }
            }
            artist.join().unwrap();
        });
        let (events, rx) = mpsc::sync_channel(24);
        let epoch = Arc::new(AtomicU64::new(0));
        let mut reads = Reader::start(events, egui::Context::default(), epoch.clone());
        reads.submit(&foreground, Command::Artist(1));
        seen_rx.recv_timeout(WAIT).unwrap();
        reads.submit(&foreground, Command::CatalogSearch("fast".into()));
        assert!(
            matches!(receive(&rx, &epoch), Event::Catalog(q, items) if q == "fast" && items[0].title == "Fast search")
        );
        assert_eq!(foreground.stream(3, false).unwrap().codec, "flac");
        // Signing out invalidates an in-flight response and its next HTTP request.
        reads.reset();
        release_tx.send(()).unwrap();
        server.join().unwrap();
        assert!(
            Event::CatalogRead(0, Box::new(Event::Notice("old account".into())))
                .for_catalog_epoch(epoch.load(Ordering::Acquire))
                .is_none()
        );
    }

    #[test]
    fn slow_home_never_blocks_search_and_old_account_feed_is_discarded() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let t = tidal(format!("http://{}/v1/", listener.local_addr().unwrap()));
        let (seen_tx, seen_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut blocked, _) = listener.accept().unwrap();
            assert!(request(&mut blocked).starts_with("GET /v2/home/feed/static?"));
            seen_tx.send(()).unwrap();
            let home = std::thread::spawn(move || {
                release_rx.recv_timeout(WAIT).unwrap();
                respond(
                    &mut blocked,
                    json!({"items":[{"type":"HORIZONTAL_LIST","title":"Old account","items":[{"type":"ALBUM","data":{"id":8,"title":"Old recommendation"}}]}]}),
                );
            });
            let (mut search, _) = listener.accept().unwrap();
            assert!(request(&mut search).starts_with("GET /v1/search?"));
            respond(
                &mut search,
                json!({"albums":{"items":[{"id":9,"title":"Search still responds"}]}}),
            );
            home.join().unwrap();
        });
        let (events, rx) = mpsc::sync_channel(24);
        let epoch = Arc::new(AtomicU64::new(0));
        let mut reads = Reader::start(events, egui::Context::default(), epoch.clone());
        reads.submit(&t, Command::Home);
        seen_rx.recv_timeout(WAIT).unwrap();
        reads.submit(&t, Command::CatalogSearch("fast".into()));
        assert!(
            matches!(receive(&rx,&epoch),Event::Catalog(q,items) if q=="fast" && items[0].id=="9")
        );
        reads.reset();
        release_tx.send(()).unwrap();
        server.join().unwrap();
        assert!(
            Event::CatalogRead(0, Box::new(Event::Home(Ok(Default::default()))))
                .for_catalog_epoch(epoch.load(Ordering::Acquire))
                .is_none()
        );
        assert!(rx.recv_timeout(Duration::from_millis(30)).is_err());
    }

    #[test]
    fn artist_header_is_delivered_before_slow_sections_finish() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let t = tidal(format!("http://{}/", listener.local_addr().unwrap()));
        let (release_tx, release_rx) = mpsc::channel();
        let server = std::thread::spawn(move || {
            for call in 0..6 {
                let (mut stream, _) = listener.accept().unwrap();
                request(&mut stream);
                if call == 1 {
                    release_rx.recv_timeout(WAIT).unwrap();
                }
                respond(
                    &mut stream,
                    match call {
                        0 => json!({"name":"Early header","picture":"portrait-id"}),
                        1 => json!({"items":[{"id":3,"title":"Top track"}]}),
                        4 => json!({"text":"Biography"}),
                        _ => json!({"items":[]}),
                    },
                );
            }
        });
        let (events, rx) = mpsc::sync_channel(24);
        let epoch = Arc::new(AtomicU64::new(0));
        let mut reads = Reader::start(events, egui::Context::default(), epoch.clone());
        reads.submit(&t, Command::Artist(1));
        assert!(
            matches!(receive(&rx, &epoch), Event::ArtistPreview(1, a) if a.name == "Early header" && a.tracks.is_empty())
        );
        release_tx.send(()).unwrap();
        assert!(matches!(receive(&rx, &epoch), Event::ArtistPreview(1, a) if a.tracks.len() == 1));
        assert!(
            matches!(receive(&rx, &epoch), Event::Artist(1, Ok(a)) if a.tracks.len() == 1 && a.biography == "Biography")
        );
        server.join().unwrap();
    }

    #[test]
    fn queued_searches_skip_obsolete_queries_and_results() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let t = tidal(format!("http://{}/", listener.local_addr().unwrap()));
        let (seen_tx, seen_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            assert!(request(&mut stream).contains("query=first"));
            seen_tx.send(()).unwrap();
            release_rx.recv_timeout(WAIT).unwrap();
            respond(&mut stream, json!({}));
            let (mut stream, _) = listener.accept().unwrap();
            assert!(request(&mut stream).contains("query=latest"));
            respond(&mut stream, json!({}));
        });
        let (events, rx) = mpsc::sync_channel(24);
        let epoch = Arc::new(AtomicU64::new(0));
        let mut reads = Reader::start(events, egui::Context::default(), epoch.clone());
        reads.submit(&t, Command::CatalogSearch("first".into()));
        seen_rx.recv_timeout(WAIT).unwrap();
        reads.submit(&t, Command::CatalogSearch("obsolete".into()));
        reads.submit(&t, Command::CatalogSearch("latest".into()));
        release_tx.send(()).unwrap();
        assert!(matches!(receive(&rx, &epoch), Event::Catalog(q, _) if q == "latest"));
        server.join().unwrap();
    }

    #[test]
    fn cancelled_read_cannot_refresh_or_send_even_with_a_valid_token() {
        let cancelled = Arc::new(AtomicBool::new(true));
        let mut t = tidal("http://127.0.0.1:1/".into()).fork_read(cancelled);
        assert!(
            t.catalog_search("cancelled")
                .unwrap_err()
                .to_string()
                .contains("cancelled")
        );
        t.session.expires_at = 0;
        assert!(
            t.catalog_search("expired")
                .unwrap_err()
                .to_string()
                .contains("cancelled")
        );
    }
}
