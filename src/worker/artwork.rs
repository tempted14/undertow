//! Viewport-first artwork. Cached images have their own lane, independent of CDN requests.
use super::{ArtData, decode_art};
use crate::{auth, config::data_dir, lyrics};
use anyhow::{Result, ensure};
use eframe::egui::{self, Color32};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

pub struct Completion {
    pub url: String,
    pub image: ArtData,
}
pub const PREFETCH: u8 = 1;

struct CacheWrite {
    thumbnail: PathBuf,
    original: Option<(PathBuf, Vec<u8>)>,
}

struct Loaded {
    image: ArtData,
    save: Option<CacheWrite>,
}
#[derive(Default)]
struct State {
    desired: HashMap<String, u8>,
    disk: HashMap<String, u8>,
    network: HashMap<String, u8>,
    active: HashSet<String>,
    ready: HashSet<String>,
    failed: HashMap<String, Instant>,
    closed: bool,
}
#[derive(Default)]
struct Counters {
    disk_hits: AtomicU64,
    network: AtomicU64,
    completed: AtomicU64,
    stale: AtomicU64,
    failures: AtomicU64,
    load_us: AtomicU64,
}
struct Shared {
    state: Mutex<State>,
    wake: Condvar,
    budget: AtomicU64,
    counters: Counters,
}
pub struct Service {
    shared: Arc<Shared>,
    loader: Arc<dyn Loader>,
    pub rx: mpsc::Receiver<Completion>,
}

trait Loader: Send + Sync {
    fn stats(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
    fn disk(&self, url: &str) -> Result<Option<Loaded>>;
    fn network(&self, url: &str) -> Result<Loaded>;
    fn persist(&self, _write: CacheWrite, _image: &ArtData, _budget: u64) {}
}

fn next_job(queue: &mut HashMap<String, u8>, minimum: u8) -> Option<String> {
    let key = queue
        .iter()
        .filter(|(_, priority)| **priority >= minimum)
        .max_by_key(|(_, priority)| **priority)?
        .0
        .clone();
    queue.remove(&key);
    Some(key)
}
impl Service {
    pub fn start(ctx: egui::Context, budget: u64) -> Self {
        Self::spawn(
            ctx,
            budget,
            Arc::new(Store::new(data_dir().join("artwork"))),
        )
    }
    fn spawn(ctx: egui::Context, budget: u64, loader: Arc<dyn Loader>) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            wake: Condvar::new(),
            budget: AtomicU64::new(budget),
            counters: Counters::default(),
        });
        let (tx, rx) = mpsc::sync_channel(8);
        // The cache lane cannot be held hostage by slow/offscreen network requests.
        // Keep one CDN lane available for covers on screen. Both lanes can serve
        // visible work, but queued prefetches cannot occupy both connections.
        for (network, minimum) in [(false, PREFETCH), (true, PREFETCH), (true, PREFETCH + 1)] {
            let shared = shared.clone();
            let tx = tx.clone();
            let ctx = ctx.clone();
            let loader = loader.clone();
            std::thread::Builder::new()
                .name(
                    if network {
                        "artwork-cdn"
                    } else {
                        "artwork-disk"
                    }
                    .into(),
                )
                .stack_size(512 * 1024)
                .spawn(move || {
                    loop {
                        let url = {
                            let mut state = shared.state.lock().unwrap();
                            loop {
                                if state.closed {
                                    return;
                                }
                                let queue = if network {
                                    &mut state.network
                                } else {
                                    &mut state.disk
                                };
                                if let Some(url) = next_job(queue, minimum) {
                                    state.active.insert(url.clone());
                                    break url;
                                }
                                state = shared.wake.wait(state).unwrap();
                            }
                        };
                        let started = Instant::now();
                        let budget = shared.budget.load(Ordering::Relaxed).clamp(8, 128);
                        let result = if network {
                            shared.counters.network.fetch_add(1, Ordering::Relaxed);
                            loader.network(&url).map(Some)
                        } else {
                            loader.disk(&url)
                        };
                        let mut state = shared.state.lock().unwrap();
                        state.active.remove(&url);
                        if state.closed {
                            return;
                        }
                        match result {
                            Ok(Some(Loaded { image, save })) => {
                                let wanted = state.desired.contains_key(&url);
                                if wanted {
                                    state.ready.insert(url.clone());
                                    state.failed.remove(&url);
                                }
                                drop(state);
                                if wanted {
                                    if !network {
                                        shared.counters.disk_hits.fetch_add(1, Ordering::Relaxed);
                                    }
                                    shared.counters.completed.fetch_add(1, Ordering::Relaxed);
                                    shared.counters.load_us.fetch_add(
                                        started.elapsed().as_micros() as u64,
                                        Ordering::Relaxed,
                                    );
                                    if tx
                                        .send(Completion {
                                            url,
                                            image: image.clone(),
                                        })
                                        .is_err()
                                    {
                                        return;
                                    }
                                    ctx.request_repaint();
                                } else {
                                    shared.counters.stale.fetch_add(1, Ordering::Relaxed);
                                }
                                if let Some(write) = save {
                                    loader.persist(write, &image, budget);
                                }
                            }
                            Ok(None) if !network => {
                                if let Some(priority) = state.desired.get(&url).copied() {
                                    state.network.insert(url, priority);
                                    shared.wake.notify_all();
                                }
                            }
                            Ok(_) => {
                                shared.counters.stale.fetch_add(1, Ordering::Relaxed);
                            }
                            Err(_) => {
                                state
                                    .failed
                                    .insert(url, Instant::now() + Duration::from_secs(3));
                                shared.counters.failures.fetch_add(1, Ordering::Relaxed);
                                drop(state);
                                ctx.request_repaint_after(Duration::from_secs(3));
                            }
                        }
                    }
                })
                .expect("artwork worker");
        }
        Self { shared, loader, rx }
    }
    pub fn submit(&self, desired: &HashMap<String, u8>, budget: u64) -> bool {
        self.shared.budget.store(budget, Ordering::Relaxed);
        // Never park the input thread behind a worker. A repaint retries a busy submission.
        let Ok(mut state) = self.shared.state.try_lock() else {
            return false;
        };
        state.desired.clone_from(desired);
        state.disk.retain(|url, priority| {
            if let Some(p) = desired.get(url) {
                *priority = *p;
                true
            } else {
                false
            }
        });
        state.network.retain(|url, priority| {
            if let Some(p) = desired.get(url) {
                *priority = *p;
                true
            } else {
                false
            }
        });
        let now = Instant::now();
        state.failed.retain(|_, next| *next > now);
        state.ready.retain(|url| desired.contains_key(url));
        let mut enqueue = |url: &String, priority: u8| {
            if !state.active.contains(url)
                && !state.disk.contains_key(url)
                && !state.network.contains_key(url)
                && !state.ready.contains(url)
                && !state.failed.contains_key(url)
            {
                state.disk.insert(url.clone(), priority);
            }
        };
        const MAX_PENDING: usize = 160;
        if desired.len() > MAX_PENDING {
            let mut requests: Vec<_> = desired.iter().collect();
            requests.select_nth_unstable_by_key(MAX_PENDING - 1, |(_, p)| std::cmp::Reverse(**p));
            for &(url, priority) in &requests[..MAX_PENDING] {
                enqueue(url, *priority);
            }
        } else {
            for (url, &priority) in desired {
                enqueue(url, priority);
            }
        }
        self.shared.wake.notify_all();
        true
    }
    pub fn acknowledge(&self, url: &str) {
        if let Ok(mut state) = self.shared.state.try_lock() {
            state.ready.remove(url);
        }
    }
    pub fn stats(&self) -> serde_json::Value {
        let c = &self.shared.counters;
        let done = c.completed.load(Ordering::Relaxed);
        serde_json::json!({"disk_hits":c.disk_hits.load(Ordering::Relaxed),"network_requests":c.network.load(Ordering::Relaxed),"completed":done,"stale_results_discarded":c.stale.load(Ordering::Relaxed),"failed_requests":c.failures.load(Ordering::Relaxed),"average_load_ms":c.load_us.load(Ordering::Relaxed) as f64 / done.max(1) as f64 / 1000.,"cache_workers":1,"network_workers":2,"cache_detail":self.loader.stats()})
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        if let Ok(mut state) = self.shared.state.lock() {
            state.closed = true;
            self.shared.wake.notify_all();
        }
    }
}

struct Store {
    root: PathBuf,
    client: Option<reqwest::blocking::Client>,
    maintenance: Mutex<(Instant, usize)>,
    decoded_hits: AtomicU64,
    original_hits: AtomicU64,
}
impl Store {
    fn new(root: PathBuf) -> Self {
        Self {
            root,
            client: reqwest::blocking::Client::builder()
                .user_agent(auth::UA)
                .connect_timeout(Duration::from_secs(2))
                .timeout(Duration::from_secs(5))
                .pool_max_idle_per_host(2)
                .build()
                .ok(),
            maintenance: Mutex::new((Instant::now(), 0)),
            decoded_hits: AtomicU64::new(0),
            original_hits: AtomicU64::new(0),
        }
    }
    fn paths(&self, raw: &str) -> Result<(String, PathBuf, PathBuf, u32)> {
        let (source, side) = split_size(raw)?;
        let original = if let Some(path) = source.strip_prefix("local:") {
            PathBuf::from(path)
        } else {
            self.root
                .join(format!("{:x}.jpg", Sha256::digest(source.as_bytes())))
        };
        let stamp = if source.starts_with("local:") {
            std::fs::metadata(&original)
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|t| format!("{t:?}"))
                .unwrap_or_default()
        } else {
            String::new()
        };
        let thumbnail = self.root.join("decoded-v1").join(format!(
            "{:x}.utart",
            Sha256::digest(format!("{source}|{side}|{stamp}").as_bytes())
        ));
        Ok((source, original, thumbnail, side))
    }
    fn persist_thumbnail(&self, path: &Path, image: &ArtData, budget: u64) {
        // A raw, validated RGBA thumbnail avoids decoding a large JPEG on each return scroll.
        let _ = write_decoded(path, image);
        let mut maintenance = self.maintenance.lock().unwrap();
        maintenance.1 += 1;
        if maintenance.1 >= 32 || maintenance.0.elapsed() >= Duration::from_secs(30) {
            *maintenance = (Instant::now(), 0);
            drop(maintenance);
            let bytes = budget * 1024 * 1024;
            let _ = lyrics::prune_cache(&self.root.join("decoded-v1"), bytes / 4);
            let _ = lyrics::prune_cache(&self.root, bytes * 3 / 4);
        }
    }
}
impl Loader for Store {
    fn stats(&self) -> serde_json::Value {
        serde_json::json!({"decoded_thumbnail_hits":self.decoded_hits.load(Ordering::Relaxed),"original_file_hits":self.original_hits.load(Ordering::Relaxed)})
    }
    fn disk(&self, raw: &str) -> Result<Option<Loaded>> {
        let (_, original, thumbnail, side) = self.paths(raw)?;
        if let Ok(image) = read_decoded(&thumbnail) {
            self.decoded_hits.fetch_add(1, Ordering::Relaxed);
            return Ok(Some(Loaded { image, save: None }));
        }
        if !original.is_file() {
            return Ok(None);
        }
        ensure!(
            std::fs::metadata(&original)?.len() <= 8 * 1024 * 1024,
            "Artwork exceeds 8 MiB"
        );
        match decode_art(&std::fs::read(&original)?, side) {
            Ok(image) => {
                self.original_hits.fetch_add(1, Ordering::Relaxed);
                Ok(Some(Loaded {
                    image,
                    save: Some(CacheWrite {
                        thumbnail,
                        original: None,
                    }),
                }))
            }
            Err(_) if !raw.starts_with("local:") => {
                let _ = std::fs::remove_file(original);
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }
    fn network(&self, raw: &str) -> Result<Loaded> {
        let (source, original, thumbnail, side) = self.paths(raw)?;
        ensure!(!source.starts_with("local:"), "Local cover is unavailable");
        let mut bytes = Vec::new();
        self.client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Artwork connection unavailable"))?
            .get(source)
            .send()?
            .error_for_status()?
            .take(8 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= 8 * 1024 * 1024, "Artwork exceeds 8 MiB");
        let image = decode_art(&bytes, side)?;
        Ok(Loaded {
            image,
            save: Some(CacheWrite {
                thumbnail,
                original: Some((original, bytes)),
            }),
        })
    }
    fn persist(&self, write: CacheWrite, image: &ArtData, budget: u64) {
        if let Some((original, bytes)) = write.original {
            // Different requested sizes can refer to the same service URL.
            let temp = original.with_extension(format!("{:x}.tmp", rand::random::<u64>()));
            if std::fs::create_dir_all(&self.root).is_ok()
                && (std::fs::write(&temp, bytes).is_err()
                    || std::fs::rename(&temp, &original).is_err())
            {
                let _ = std::fs::remove_file(temp);
            }
        }
        self.persist_thumbnail(&write.thumbnail, image, budget);
    }
}
fn split_size(raw: &str) -> Result<(String, u32)> {
    if let Some(local) = raw.strip_prefix("local:") {
        let (path, side) = local
            .rsplit_once("#undertow-size=")
            .map(|(p, s)| (p, s.parse().unwrap_or(640)))
            .unwrap_or((local, 640));
        ensure!(matches!(side, 80 | 160 | 320 | 640), "Invalid artwork size");
        return Ok((format!("local:{path}"), side));
    }
    let mut url = url::Url::parse(raw)?;
    ensure!(
        url.scheme() == "https"
            && url.username().is_empty()
            && url.password().is_none()
            && url.port().is_none()
            && url
                .host_str()
                .is_some_and(|h| h == "resources.tidal.com" || h.ends_with(".tidal.com")),
        "Untrusted artwork URL"
    );
    let side = url
        .fragment()
        .and_then(|f| f.strip_prefix("undertow-size="))
        .and_then(|s| s.parse().ok())
        .unwrap_or(640);
    ensure!(matches!(side, 80 | 160 | 320 | 640), "Invalid artwork size");
    url.set_fragment(None);
    Ok((url.into(), side))
}
fn write_decoded(path: &Path, image: &ArtData) -> Result<()> {
    std::fs::create_dir_all(path.parent().unwrap())?;
    let mut data =
        Vec::with_capacity(16 + (image.cover.pixels.len() + image.blur.pixels.len()) * 4);
    data.extend_from_slice(b"UTART001");
    data.extend_from_slice(&(image.cover.width() as u16).to_le_bytes());
    data.extend_from_slice(&(image.cover.height() as u16).to_le_bytes());
    data.extend_from_slice(&image.palette);
    data.push(0);
    for p in image.cover.pixels.iter().chain(&image.blur.pixels) {
        data.extend_from_slice(&p.to_array());
    }
    let temp = path.with_extension(format!("{:x}.tmp", rand::random::<u64>()));
    std::fs::write(&temp, data)?;
    if std::fs::rename(&temp, path).is_err() {
        let _ = std::fs::remove_file(temp);
    }
    Ok(())
}
fn read_decoded(path: &Path) -> Result<ArtData> {
    ensure!(
        std::fs::metadata(path)?.len() <= 2 * 1024 * 1024,
        "Oversized thumbnail cache"
    );
    let data = std::fs::read(path)?;
    ensure!(
        data.len() >= 16 && &data[..8] == b"UTART001",
        "Invalid thumbnail cache"
    );
    let w = u16::from_le_bytes([data[8], data[9]]) as usize;
    let h = u16::from_le_bytes([data[10], data[11]]) as usize;
    ensure!(
        (1..=640).contains(&w)
            && (1..=640).contains(&h)
            && data.len() == 16 + (w * h + 48 * 48) * 4,
        "Invalid thumbnail dimensions"
    );
    let pixels = |slice: &[u8]| {
        slice
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| Color32::from_rgba_premultiplied(p[0], p[1], p[2], p[3]))
            .collect()
    };
    Ok(ArtData {
        cover: Arc::new(egui::ColorImage::new(
            [w, h],
            pixels(&data[16..16 + w * h * 4]),
        )),
        blur: Arc::new(egui::ColorImage::new(
            [48, 48],
            pixels(&data[16 + w * h * 4..]),
        )),
        palette: [data[12], data[13], data[14]],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn image() -> ArtData {
        ArtData {
            cover: Arc::new(egui::ColorImage::filled([80, 80], Color32::LIGHT_BLUE)),
            blur: Arc::new(egui::ColorImage::filled([48, 48], Color32::DARK_BLUE)),
            palette: [10, 80, 140],
        }
    }
    struct Mock {
        started: mpsc::SyncSender<String>,
        gate: Arc<(Mutex<bool>, Condvar)>,
    }
    impl Loader for Mock {
        fn disk(&self, url: &str) -> Result<Option<Loaded>> {
            Ok(url.starts_with("cached:").then(|| Loaded {
                image: image(),
                save: None,
            }))
        }
        fn network(&self, url: &str) -> Result<Loaded> {
            self.started.send(url.to_owned()).unwrap();
            if !url.starts_with("slow-") {
                return Ok(Loaded {
                    image: image(),
                    save: None,
                });
            }
            let (lock, wake) = &*self.gate;
            let mut open = lock.lock().unwrap();
            while !*open {
                open = wake.wait(open).unwrap();
            }
            Ok(Loaded {
                image: image(),
                save: None,
            })
        }
    }
    #[test]
    fn a_slow_cdn_cannot_block_current_cached_art_and_stale_queue_is_dropped() {
        let (tx, rx) = mpsc::sync_channel(8);
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let service = Service::spawn(
            egui::Context::default(),
            64,
            Arc::new(Mock {
                started: tx,
                gate: gate.clone(),
            }),
        );
        service.submit(
            &HashMap::from([
                ("slow-a".into(), 5),
                ("slow-b".into(), 5),
                ("obsolete".into(), 1),
            ]),
            64,
        );
        let first = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let second = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(first.starts_with("slow-") && second.starts_with("slow-"));
        service.submit(&HashMap::from([("cached:current".into(), 5)]), 64);
        let current = service
            .rx
            .recv_timeout(Duration::from_secs(2))
            .expect("disk lane must finish while both CDN lanes are blocked");
        assert_eq!(current.url, "cached:current");
        assert_eq!(current.image.cover.size, [80, 80]);
        service.acknowledge(&current.url);
        assert_eq!(service.stats()["network_requests"], 2);
        *gate.0.lock().unwrap() = true;
        gate.1.notify_all();
        drop(service);
    }
    #[test]
    fn visible_art_downloads_while_speculative_work_is_blocked() {
        let (tx, rx) = mpsc::sync_channel(8);
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let service = Service::spawn(
            egui::Context::default(),
            64,
            Arc::new(Mock {
                started: tx,
                gate: gate.clone(),
            }),
        );
        let mut desired = HashMap::from([
            ("slow-neighbor-a".into(), PREFETCH),
            ("slow-neighbor-b".into(), PREFETCH),
        ]);
        assert!(service.submit(&desired, 64));
        assert!(
            rx.recv_timeout(Duration::from_secs(2))
                .unwrap()
                .starts_with("slow-")
        );
        desired.insert("visible".into(), PREFETCH + 1);
        let deadline = Instant::now() + Duration::from_secs(2);
        while !service.submit(&desired, 64) {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        let result = service.rx.recv_timeout(Duration::from_secs(2));
        *gate.0.lock().unwrap() = true;
        gate.1.notify_all();
        assert_eq!(result.unwrap().url, "visible");
    }

    #[test]
    fn sends_decoded_art_before_cache_persistence_finishes() {
        // Model the persistence gate with a mutex, keeping the fake loader Sync.
        struct LoaderWithWrite(Mutex<mpsc::Receiver<()>>);
        impl Loader for LoaderWithWrite {
            fn disk(&self, _: &str) -> Result<Option<Loaded>> {
                Ok(Some(Loaded {
                    image: image(),
                    save: Some(CacheWrite {
                        thumbnail: PathBuf::new(),
                        original: None,
                    }),
                }))
            }
            fn network(&self, _: &str) -> Result<Loaded> {
                unreachable!()
            }
            fn persist(&self, _: CacheWrite, _: &ArtData, _: u64) {
                let _ = self.0.lock().unwrap().recv_timeout(Duration::from_secs(5));
            }
        }
        let (tx, rx) = mpsc::channel();
        let service = Service::spawn(
            egui::Context::default(),
            64,
            Arc::new(LoaderWithWrite(Mutex::new(rx))),
        );
        let desired = HashMap::from([("visible".into(), PREFETCH + 1)]);
        let deadline = Instant::now() + Duration::from_secs(2);
        while !service.submit(&desired, 64) {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        let result = service.rx.recv_timeout(Duration::from_secs(2));
        let _ = tx.send(());
        assert_eq!(result.unwrap().url, "visible");
    }
    #[test]
    fn thumbnail_round_trip_preserves_pixels_and_corruption_is_rejected() {
        let path = std::env::temp_dir().join(format!(
            "undertow-art-test-{:x}.utart",
            rand::random::<u64>()
        ));
        let source = image();
        write_decoded(&path, &source).unwrap();
        let cached = read_decoded(&path).unwrap();
        assert_eq!(source.cover.pixels, cached.cover.pixels);
        assert_eq!(source.blur.pixels, cached.blur.pixels);
        assert_eq!(source.palette, cached.palette);
        std::fs::write(&path, b"UTART001\xff\xff\xff\xffbroken").unwrap();
        assert!(read_decoded(&path).is_err());
        std::fs::remove_file(path).unwrap();
        assert!(split_size("https://resources.tidal.com.evil.test/a.jpg").is_err());
        assert!(split_size("https://user:pass@resources.tidal.com/a.jpg").is_err());
    }
}
