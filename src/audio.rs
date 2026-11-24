mod buffer;
use crate::{backend::StreamPlan, config::data_dir};
use anyhow::{Context, Result, ensure};
use rodio::{OutputStream, OutputStreamBuilder, Sink, Source};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex, OnceLock, Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, TryRecvError},
    },
    time::Duration,
};

const MAX_SPOOL: u64 = 512 * 1024 * 1024;
const PCM_SAMPLES: usize = 4096;
const PCM_PACKETS: usize = 16; // At most 256 KiB decoded audio, regardless of track length.

#[derive(Clone, Debug, Default)]
pub struct Format {
    pub codec: String,
    pub bit_depth: u32,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration: f64,
    pub output_rate: u32,
    pub lossless: bool,
    pub requested_quality: String,
    pub encoded_bitrate: u64,
}

#[derive(Default)]
pub struct AudioStats {
    spool: OnceLock<Weak<SpoolState>>,
    #[cfg(test)]
    stream_waits: AtomicU64,
    pub downloaded: AtomicU64,
    pub pcm_allocations: AtomicU64,
    pub decoder_waits: AtomicU64,
    played_samples: AtomicU64,
    consumed_samples: AtomicU64,
    pub underrun_samples: AtomicU64,
    pub seek_ms: AtomicU64,
    pub epoch: AtomicU64,
    played_epoch: AtomicU64,
    // Zero means open. EOF belongs to the decoder generation that reached it.
    eof_epoch: AtomicU64,
    pub cancel: AtomicBool,
    pub error: Mutex<Option<String>>,
}
impl AudioStats {
    fn position(&self, samples_per_second: f64) -> f64 {
        let epoch = self.epoch.load(Ordering::Acquire);
        if self.played_epoch.load(Ordering::Acquire) == epoch {
            self.played_samples.load(Ordering::Relaxed) as f64 / samples_per_second
        } else {
            self.seek_ms.load(Ordering::Relaxed) as f64 / 1000.
        }
    }

    /// Wake a blocked spool reader as well as setting the cancellation flag.
    /// Only control/decoder threads call this; the audio callback stays lock-free.
    fn cancel_stream(&self) {
        self.cancel.store(true, Ordering::Release);
        if let Some(state) = self.spool.get().and_then(Weak::upgrade) {
            // Synchronize with the reader's check-and-wait to avoid a lost wake.
            // Network and disk operations never hold this short progress lock.
            let _progress = state.progress.lock().unwrap_or_else(|e| e.into_inner());
            state.ready.notify_all();
        }
    }
    fn publish_sample(&self, played: u64, consumed: u64) {
        self.played_samples.store(played, Ordering::Relaxed);
        self.consumed_samples.store(consumed, Ordering::Relaxed);
    }
}

#[derive(Default)]
struct Progress {
    bytes: u64,
    expected: Option<u64>,
    finished: bool,
    error: Option<String>,
}
struct SpoolState {
    progress: Mutex<Progress>,
    ready: Condvar,
    stats: Arc<AudioStats>,
    path: PathBuf,
}
impl Drop for SpoolState {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
struct SpoolReader {
    file: File,
    state: Arc<SpoolState>,
    position: u64,
}
impl Drop for SpoolReader {
    fn drop(&mut self) {
        // Includes unused/stale Prepared results, not only actively playing tracks.
        self.state.stats.cancel_stream();
    }
}

impl Read for SpoolReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        let mut p = self
            .state
            .progress
            .lock()
            .map_err(|_| io::Error::other("Spool lock failed"))?;
        while self.position >= p.bytes
            && !p.finished
            && p.error.is_none()
            && !self.state.stats.cancel.load(Ordering::Relaxed)
        {
            #[cfg(test)]
            self.state
                .stats
                .stream_waits
                .fetch_add(1, Ordering::Release);
            let (next, result) = self
                .state
                .ready
                .wait_timeout(p, Duration::from_secs(30))
                .map_err(|_| io::Error::other("Spool wait failed"))?;
            p = next;
            if result.timed_out() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Audio download stalled for 30 seconds",
                ));
            }
        }
        if self.state.stats.cancel.load(Ordering::Relaxed) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Playback cancelled",
            ));
        }
        if let Some(e) = &p.error {
            return Err(io::Error::other(e.clone()));
        }
        let n = (p.bytes.saturating_sub(self.position) as usize).min(out.len());
        drop(p);
        if n == 0 {
            return Ok(0);
        }
        let n = self.file.read(&mut out[..n])?;
        self.position += n as u64;
        Ok(n)
    }
}
impl Seek for SpoolReader {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let target = match from {
            SeekFrom::Start(p) => p as i128,
            SeekFrom::Current(p) => self.position as i128 + p as i128,
            SeekFrom::End(p) => {
                let mut state = self
                    .state
                    .progress
                    .lock()
                    .map_err(|_| io::Error::other("Spool lock failed"))?;
                while !state.finished
                    && state.error.is_none()
                    && !self.state.stats.cancel.load(Ordering::Relaxed)
                {
                    #[cfg(test)]
                    self.state
                        .stats
                        .stream_waits
                        .fetch_add(1, Ordering::Release);
                    let (s, t) = self
                        .state
                        .ready
                        .wait_timeout(state, Duration::from_secs(30))
                        .map_err(|_| io::Error::other("Spool wait failed"))?;
                    state = s;
                    if t.timed_out() {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "Stream length not available yet",
                        ));
                    }
                }
                state.bytes as i128 + p as i128
            }
        };
        if self.state.stats.cancel.load(Ordering::Relaxed) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Playback cancelled",
            ));
        }
        if target < 0 || target > MAX_SPOOL as i128 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Seek beyond stream limits",
            ));
        }
        self.position = self.file.seek(SeekFrom::Start(target as u64))?;
        Ok(self.position)
    }
}

fn spool(plan: StreamPlan, stats: Arc<AudioStats>, retry: bool) -> Result<SpoolReader> {
    std::fs::create_dir_all(data_dir().join("spool"))?;
    let path = data_dir()
        .join("spool")
        .join(format!("{}.part", crate::auth::random_urlsafe()));
    let write = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)?;
    let read = File::open(&path)?;
    let state = Arc::new(SpoolState {
        progress: Mutex::new(Progress::default()),
        ready: Condvar::new(),
        stats,
        path,
    });
    let _ = state.stats.spool.set(Arc::downgrade(&state));
    let state2 = state.clone();
    std::thread::Builder::new()
        .name("tidal-stream".into())
        .spawn(move || {
            let result = download_into(&plan, write, &state2, retry);
            if let Ok(mut p) = state2.progress.lock() {
                p.finished = true;
                if let Err(e) = result
                    && !state2.stats.cancel.load(Ordering::Relaxed)
                {
                    p.error = Some(e.to_string());
                }
            }
            state2.ready.notify_all();
        })?;
    Ok(SpoolReader {
        file: read,
        state,
        position: 0,
    })
}

fn download_into(plan: &StreamPlan, mut file: File, state: &SpoolState, retry: bool) -> Result<()> {
    use ctr::cipher::{KeyIvInit, StreamCipher, StreamCipherSeek};
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(45))
        .pool_max_idle_per_host(2)
        .build()?;
    let mut cipher = plan.encryption.map(|(key, nonce)| {
        let mut iv = [0u8; 16];
        iv[..8].copy_from_slice(&nonce);
        ctr::Ctr128BE::<aes::Aes128>::new(&key.into(), &iv.into())
    });
    let mut total = 0u64;
    let mut buf = [0u8; 65536];
    for url in &plan.urls {
        let mut offset = 0u64;
        let mut attempts = 0;
        loop {
            if state.stats.cancel.load(Ordering::Relaxed) {
                return Ok(());
            }
            let mut req = client.get(url);
            if offset > 0 {
                req = req.header("Range", format!("bytes={offset}-"));
            }
            let response = req.send();
            let mut response = match response {
                Ok(r) => r,
                Err(e) => {
                    if retry && attempts < 2 {
                        attempts += 1;
                        std::thread::sleep(Duration::from_millis(250 * attempts));
                        continue;
                    }
                    return Err(anyhow::anyhow!(
                        "Audio connection failed: {}",
                        e.status().map(|s| s.as_u16()).unwrap_or(0)
                    ));
                }
            };
            ensure!(
                response.status().is_success(),
                "Audio CDN returned HTTP {}",
                response.status().as_u16()
            );
            if offset > 0 {
                ensure!(
                    response.status().as_u16() == 206,
                    "CDN does not support safe resume; refusing duplicate audio bytes"
                );
                let expected = format!("bytes {offset}-");
                ensure!(
                    response
                        .headers()
                        .get("Content-Range")
                        .and_then(|h| h.to_str().ok())
                        .is_some_and(|h| h.starts_with(&expected)),
                    "CDN resumed at the wrong byte position"
                );
            }
            ensure!(
                response.content_length().unwrap_or(0).saturating_add(total) <= MAX_SPOOL,
                "Track exceeds the 512 MiB spool limit"
            );
            if plan.urls.len() == 1
                && let Ok(mut p) = state.progress.lock()
            {
                p.expected = response.content_length().map(|n| n + offset);
            }
            let mut failed = false;
            loop {
                if state.stats.cancel.load(Ordering::Relaxed) {
                    return Ok(());
                }
                match response.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        ensure!(
                            total + n as u64 <= MAX_SPOOL,
                            "Stream exceeded disk spool limit"
                        );
                        if let Some(c) = &mut cipher {
                            c.seek(total);
                            c.apply_keystream(&mut buf[..n]);
                        }
                        file.write_all(&buf[..n])?;
                        offset += n as u64;
                        total += n as u64;
                        state.stats.downloaded.store(total, Ordering::Relaxed);
                        if let Ok(mut p) = state.progress.lock() {
                            p.bytes = total;
                        }
                        state.ready.notify_all();
                    }
                    Err(_) => {
                        failed = true;
                        break;
                    }
                }
            }
            if !failed {
                break;
            }
            ensure!(
                retry && attempts < 2,
                "Audio CDN stalled after bounded retries"
            );
            attempts += 1;
        }
    }
    file.flush()?;
    Ok(())
}

pub struct Prepared {
    decoder: NativeDecoder,
    pub format: Format,
    pub stats: Arc<AudioStats>,
}
trait ReadSeek: Read + Seek + Send + Sync {
    fn byte_len(&self) -> Option<u64>;
}
impl ReadSeek for File {
    fn byte_len(&self) -> Option<u64> {
        self.metadata().ok().map(|m| m.len())
    }
}
impl ReadSeek for SpoolReader {
    fn byte_len(&self) -> Option<u64> {
        self.state.progress.lock().ok().and_then(|p| {
            if p.finished {
                Some(p.bytes)
            } else {
                p.expected
            }
        })
    }
}

struct MediaInput(Box<dyn ReadSeek>);
impl Read for MediaInput {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.0.read(b)
    }
}
impl Seek for MediaInput {
    fn seek(&mut self, s: SeekFrom) -> io::Result<u64> {
        self.0.seek(s)
    }
}
impl symphonia::core::io::MediaSource for MediaInput {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        self.0.byte_len()
    }
}

struct NativeDecoder {
    demuxer: Box<dyn symphonia::core::formats::FormatReader>,
    codec: Box<dyn symphonia::core::codecs::Decoder>,
    buffer: buffer::DecodedSamples,
    position: usize,
    track_id: u32,
    discard: usize,
    time_base: Option<symphonia::core::units::TimeBase>,
    format_info: Format,
    stats: Arc<AudioStats>,
}
impl NativeDecoder {
    fn new(input: Box<dyn ReadSeek>, stats: Arc<AudioStats>) -> Result<Self> {
        let encoded_bytes = input.byte_len();
        use symphonia::core::{
            codecs::DecoderOptions, formats::FormatOptions, io::MediaSourceStream,
            meta::MetadataOptions, probe::Hint,
        };
        let mss = MediaSourceStream::new(Box::new(MediaInput(input)), Default::default());
        let probed = symphonia::default::get_probe().format(
            &Hint::new(),
            mss,
            &FormatOptions {
                enable_gapless: true,
                ..Default::default()
            },
            &MetadataOptions::default(),
        )?;
        let track = probed
            .format
            .default_track()
            .context("Audio container has no default track")?;
        let params = track.codec_params.clone();
        let id = track.id;
        let codec = symphonia::default::get_codecs().make(&params, &DecoderOptions::default())?;
        let descriptor = symphonia::default::get_codecs().get_codec(params.codec);
        let duration = match (params.n_frames, params.time_base) {
            (Some(n), Some(t)) => {
                let time = t.calc_time(n);
                time.seconds as f64 + time.frac
            }
            _ => 0.0,
        };
        let format_info = Format {
            encoded_bitrate: if duration > 0.0 {
                (encoded_bytes.unwrap_or(0) as f64 * 8.0 / duration) as u64
            } else {
                0
            },
            codec: descriptor
                .map(|d| d.short_name.to_uppercase())
                .unwrap_or_else(|| "UNKNOWN".into()),
            bit_depth: params
                .bits_per_sample
                .or(params.bits_per_coded_sample)
                .unwrap_or(0),
            sample_rate: params.sample_rate.context("Audio sample rate is missing")?,
            channels: params
                .channels
                .context("Audio channel layout is missing")?
                .count() as u16,
            duration,
            output_rate: 0,
            lossless: params.codec == symphonia::core::codecs::CODEC_TYPE_FLAC,
            requested_quality: String::new(),
        };
        Ok(Self {
            demuxer: probed.format,
            codec,
            buffer: Default::default(),
            position: 0,
            track_id: id,
            discard: 0,
            time_base: params.time_base,
            format_info,
            stats,
        })
    }
    fn try_seek(&mut self, d: Duration) -> Result<()> {
        use symphonia::core::{
            formats::{SeekMode, SeekTo},
            units::Time,
        };
        let result = self.demuxer.seek(
            SeekMode::Accurate,
            SeekTo::Time {
                time: Time::from(d.as_secs_f64()),
                track_id: Some(self.track_id),
            },
        )?;
        self.codec.reset();
        self.buffer.clear();
        self.position = 0;
        if let Some(tb) = self.time_base {
            let time = tb.calc_time(result.required_ts.saturating_sub(result.actual_ts));
            self.discard = ((time.seconds as f64 + time.frac) * self.format_info.sample_rate as f64)
                .round() as usize
                * self.format_info.channels as usize;
        }
        Ok(())
    }

    /// Append the same interleaved samples as the iterator, copying whole spans
    /// from each decoded packet instead of advancing one sample at a time.
    fn copy_samples(&mut self, output: &mut Vec<f32>, count: usize) {
        let end = output.len().saturating_add(count);
        while output.len() < end {
            if self.position >= self.buffer.len() && !self.decode_packet() {
                break;
            }
            let discard = self.discard.min(self.buffer.len() - self.position);
            self.position += discard;
            self.discard -= discard;
            let count = (self.buffer.len() - self.position).min(end - output.len());
            output.extend_from_slice(&self.buffer[self.position..self.position + count]);
            self.position += count;
        }
    }

    fn decode_packet(&mut self) -> bool {
        loop {
            let packet = match self.demuxer.next_packet() {
                Ok(p) => p,
                Err(symphonia::core::errors::Error::IoError(e))
                    if e.kind() == io::ErrorKind::UnexpectedEof =>
                {
                    return false;
                }
                Err(e) => {
                    if !self.stats.cancel.load(Ordering::Relaxed)
                        && let Ok(mut error) = self.stats.error.lock()
                    {
                        *error = Some(format!("Audio read failed: {e}"));
                    }
                    return false;
                }
            };
            if packet.track_id() != self.track_id {
                continue;
            }
            let decoded = match self.codec.decode(&packet) {
                Ok(d) => d,
                Err(e) => {
                    if let Ok(mut error) = self.stats.error.lock() {
                        *error = Some(format!("Audio decode failed: {e}"));
                    }
                    return false;
                }
            };
            self.buffer.copy(decoded);
            self.position = 0;
            return true;
        }
    }
}
impl Iterator for NativeDecoder {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        loop {
            if self.position < self.buffer.len() {
                let sample = self.buffer[self.position];
                self.position += 1;
                if self.discard > 0 {
                    self.discard -= 1;
                    continue;
                }
                return Some(sample);
            }
            if !self.decode_packet() {
                return None;
            }
        }
    }
}

impl Prepared {
    pub fn local(path: &Path) -> Result<Self> {
        let stats = Arc::new(AudioStats::default());
        Self::reader(Box::new(File::open(path)?), stats, None)
    }
    pub fn remote(plan: StreamPlan, retry: bool) -> Result<Self> {
        let stats = Arc::new(AudioStats::default());
        let reader = spool(plan.clone(), stats.clone(), retry)?;
        // The patched demuxer consumes fMP4 fragments as they arrive.
        let spool_state = reader.state.clone();
        let mut result = Self::reader(Box::new(reader), stats.clone(), Some(plan));
        if let Ok(prepared) = &mut result {
            let bytes = spool_state.progress.lock().ok().and_then(|p| {
                if p.finished {
                    Some(p.bytes)
                } else {
                    p.expected
                }
            });
            if prepared.format.duration > 0.0 {
                prepared.format.encoded_bitrate =
                    (bytes.unwrap_or(0) as f64 * 8.0 / prepared.format.duration) as u64;
            }
        }
        if result.is_err() {
            stats.cancel_stream();
        }
        result
    }
    fn reader(
        reader: Box<dyn ReadSeek>,
        stats: Arc<AudioStats>,
        plan: Option<StreamPlan>,
    ) -> Result<Self> {
        let mut decoder = NativeDecoder::new(reader, stats.clone())?;
        // Decode the first frame on the preparation thread, before handing audio
        // to the UI/output thread. No fixed sleep or network read on that thread.
        ensure!(
            decoder.next().is_some(),
            "The stream contains no decodable audio"
        );
        decoder.position = 0;
        let mut format = decoder.format_info.clone();
        if format.duration <= 0.0 {
            format.duration = plan.as_ref().map(|_| 0.0).unwrap_or(0.0);
        }
        if let Some(plan) = &plan {
            format.requested_quality = plan.quality.clone();
            ensure!(
                plan.bit_depth == 0 || format.bit_depth == 0 || plan.bit_depth == format.bit_depth,
                "Decoded bit depth differs from TIDAL's advertised format"
            );
        }
        Ok(Self {
            decoder,
            format,
            stats,
        })
    }
}

struct Packet {
    samples: Vec<f32>,
    epoch: u64,
    start_sample: u64,
}
struct PcmSource {
    rx: mpsc::Receiver<Packet>,
    recycle: mpsc::SyncSender<Vec<f32>>,
    decoder_thread: std::thread::Thread,
    buffer: Vec<f32>,
    buffer_start: u64,
    pos: usize,
    consumed: u64,
    epoch: u64,
    stats: Arc<AudioStats>,
    rate: u32,
    channels: u16,
    duration: Option<Duration>,
}
impl Iterator for PcmSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.stats.cancel.load(Ordering::Relaxed) {
            return None;
        }
        let epoch = self.stats.epoch.load(Ordering::Acquire);
        if epoch != self.epoch {
            self.buffer.clear();
            self.pos = 0;
            self.epoch = epoch;
        }
        if self.pos >= self.buffer.len() {
            if self.buffer.capacity() > 0 {
                let _ = self.recycle.try_send(std::mem::take(&mut self.buffer));
            }
            let mut confirmed_eof = false;
            loop {
                match self.rx.try_recv() {
                    Ok(p) if p.epoch == epoch => {
                        self.decoder_thread.unpark();
                        self.buffer = p.samples;
                        self.buffer_start = p.start_sample;
                        self.pos = 0;
                        break;
                    }
                    Ok(stale) => {
                        let _ = self.recycle.try_send(stale.samples);
                        self.decoder_thread.unpark();
                        continue;
                    }
                    Err(TryRecvError::Disconnected) => return None,
                    Err(TryRecvError::Empty) => {
                        if self.stats.eof_epoch.load(Ordering::Acquire) == epoch.wrapping_add(1)
                            && self.stats.epoch.load(Ordering::Acquire) == epoch
                        {
                            if confirmed_eof {
                                return None;
                            }
                            // A final packet may have arrived before EOF was published.
                            confirmed_eof = true;
                            continue;
                        }
                        self.stats.underrun_samples.fetch_add(1, Ordering::Relaxed);
                        return Some(0.0);
                    }
                }
            }
        }
        let s = *self.buffer.get(self.pos)?;
        self.pos += 1;
        self.consumed += 1;
        // The decoder tags packets with their seek origin, letting the source
        // own both clocks without synchronizing each increment with the decoder.
        self.stats
            .publish_sample(self.buffer_start + self.pos as u64, self.consumed);
        if self.pos == 1 {
            self.stats.played_epoch.store(epoch, Ordering::Release);
        }
        Some(s)
    }
}
impl Drop for PcmSource {
    fn drop(&mut self) {
        self.stats.cancel.store(true, Ordering::Relaxed);
        self.decoder_thread.unpark();
    }
}
impl Source for PcmSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.channels
    }
    fn sample_rate(&self) -> u32 {
        self.rate
    }
    fn total_duration(&self) -> Option<Duration> {
        self.duration
    }
}

pub struct Audio {
    _stream: OutputStream,
    decoder_thread: std::thread::Thread,
    pub sink: Sink,
    pub stats: Arc<AudioStats>,
    pub format: Format,
}
impl Audio {
    pub fn start(mut p: Prepared, volume: f32) -> Result<Self> {
        let stream = OutputStreamBuilder::open_default_stream()
            .context("No Windows audio output is available")?;
        p.format.output_rate = stream.config().sample_rate();
        let sink = Sink::connect_new(stream.mixer());
        sink.set_volume(volume);
        let (tx, rx) = mpsc::sync_channel::<Packet>(PCM_PACKETS);
        let (recycle_tx, recycle_rx) = mpsc::sync_channel::<Vec<f32>>(PCM_PACKETS + 2);
        let stats = p.stats.clone();
        let stats2 = stats.clone();
        let rate = p.format.sample_rate;
        let channels = p.format.channels;
        let initial = p
            .decoder
            .buffer
            .len()
            .saturating_sub(p.decoder.position)
            .min(PCM_SAMPLES);
        let mut samples = Vec::with_capacity(PCM_SAMPLES);
        p.decoder.copy_samples(&mut samples, initial);
        stats.pcm_allocations.store(1, Ordering::Relaxed);
        let first = Packet {
            samples,
            epoch: 0,
            start_sample: 0,
        };
        tx.try_send(first)
            .map_err(|_| anyhow::anyhow!("Cannot prime the audio queue"))?;
        let decoder = std::thread::Builder::new()
            .name("flac-decode".into())
            .spawn(move || {
                let mut epoch = 0;
                let mut next_sample = initial as u64;
                let mut spare = None;
                'decode: loop {
                    if stats2.cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let current = stats2.epoch.load(Ordering::Acquire);
                    if current != epoch {
                        epoch = current;
                        let ms = stats2.seek_ms.load(Ordering::Acquire);
                        if let Err(error) = p.decoder.try_seek(Duration::from_millis(ms)) {
                            if let Ok(mut problem) = stats2.error.lock() {
                                *problem = Some(format!("Audio seek failed: {error}"));
                            }
                            break;
                        }
                        next_sample = ms * rate as u64 * channels as u64 / 1000;
                    }
                    let mut samples = spare
                        .take()
                        .or_else(|| recycle_rx.try_recv().ok())
                        .unwrap_or_else(|| {
                            stats2.pcm_allocations.fetch_add(1, Ordering::Relaxed);
                            Vec::with_capacity(PCM_SAMPLES)
                        });
                    samples.clear();
                    p.decoder.copy_samples(&mut samples, PCM_SAMPLES);
                    if samples.is_empty() {
                        spare = Some(samples);
                        stats2
                            .eof_epoch
                            .store(epoch.wrapping_add(1), Ordering::Release);
                        loop {
                            if stats2.cancel.load(Ordering::Relaxed) {
                                break 'decode;
                            }
                            if stats2.epoch.load(Ordering::Acquire) != epoch {
                                continue 'decode;
                            }
                            stats2.decoder_waits.fetch_add(1, Ordering::Relaxed);
                            std::thread::park();
                        }
                    }
                    let mut packet = Packet {
                        samples,
                        epoch,
                        start_sample: next_sample,
                    };
                    next_sample += packet.samples.len() as u64;
                    loop {
                        match tx.try_send(packet) {
                            Ok(()) => break,
                            Err(mpsc::TrySendError::Disconnected(_)) => break 'decode,
                            Err(mpsc::TrySendError::Full(back)) => {
                                packet = back;
                                if stats2.cancel.load(Ordering::Relaxed) {
                                    break 'decode;
                                }
                                if stats2.epoch.load(Ordering::Acquire) != epoch {
                                    spare = Some(packet.samples);
                                    continue 'decode;
                                }
                                stats2.decoder_waits.fetch_add(1, Ordering::Relaxed);
                                std::thread::park();
                            }
                        }
                    }
                }
                stats2
                    .eof_epoch
                    .store(epoch.wrapping_add(1), Ordering::Release);
            })?;
        let decoder_thread = decoder.thread().clone();
        // Start after a bounded pre-roll. The audio callback never waits on a network read.
        let source = PcmSource {
            rx,
            recycle: recycle_tx,
            decoder_thread: decoder_thread.clone(),
            buffer: Vec::new(),
            buffer_start: 0,
            pos: 0,
            consumed: 0,
            epoch: 0,
            stats: stats.clone(),
            rate,
            channels,
            duration: if p.format.duration > 0.0 {
                Some(Duration::from_secs_f64(p.format.duration))
            } else {
                None
            },
        };
        sink.append(source);
        Ok(Self {
            _stream: stream,
            decoder_thread,
            sink,
            stats,
            format: p.format,
        })
    }
    pub fn position(&self) -> f64 {
        self.stats
            .position((self.format.sample_rate as f64 * self.format.channels as f64).max(1.0))
    }
    pub fn consumed_seconds(&self) -> f64 {
        self.stats.consumed_samples.load(Ordering::Relaxed) as f64
            / (self.format.sample_rate as f64 * self.format.channels as f64).max(1.0)
    }
    pub fn seek(&self, seconds: f64) {
        let seconds = seconds.max(0.);
        // The one-past-end timestamp is not seekable in every container.
        let seconds = if self.format.duration > 0. {
            seconds.min((self.format.duration - 0.001).max(0.))
        } else {
            seconds
        };
        self.stats
            .seek_ms
            .store((seconds * 1000.) as u64, Ordering::Release);
        self.stats.epoch.fetch_add(1, Ordering::AcqRel);
        self.decoder_thread.unpark();
    }
}
impl Drop for Audio {
    fn drop(&mut self) {
        self.stats.cancel_stream();
        self.decoder_thread.unpark();
        self.sink.stop();
    }
}

pub fn save_download(plan: StreamPlan, path: &Path) -> Result<()> {
    let stats = Arc::new(AudioStats::default());
    let mut reader = spool(plan, stats, true)?;
    let parent = path.parent().context("Invalid download path")?;
    std::fs::create_dir_all(parent)?;
    ensure!(
        !path.exists(),
        "Download already exists; choose a different filename"
    );
    let part = path.with_extension("download.part");
    let result = (|| -> Result<()> {
        let mut out = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&part)?;
        io::copy(&mut reader, &mut out)?;
        out.sync_all()?;
        drop(out);
        std::fs::rename(&part, path)?;
        Ok(())
    })();
    if result.is_err() {
        reader.state.stats.cancel_stream();
        let _ = std::fs::remove_file(part);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_wakes_stalled_reads_and_length_seeks_without_retaining_spools() {
        for seek in [false, true] {
            let path = std::env::temp_dir().join(format!(
                "undertow-cancel-{}.part",
                crate::auth::random_urlsafe()
            ));
            File::create(&path).unwrap();
            let stats = Arc::new(AudioStats::default());
            let state = Arc::new(SpoolState {
                progress: Mutex::new(Progress::default()),
                ready: Condvar::new(),
                stats: stats.clone(),
                path: path.clone(),
            });
            let weak = Arc::downgrade(&state);
            let _ = stats.spool.set(weak.clone());
            let mut reader = SpoolReader {
                file: File::open(&path).unwrap(),
                state: state.clone(),
                position: 0,
            };
            let (finished, result) = mpsc::channel();
            let thread = std::thread::spawn(move || {
                let error = if seek {
                    reader.seek(SeekFrom::End(0)).unwrap_err()
                } else {
                    reader.read(&mut [0; 8]).unwrap_err()
                };
                finished.send(error.kind()).unwrap();
            });
            let start = std::time::Instant::now();
            while stats.stream_waits.load(Ordering::Acquire) == 0 {
                assert!(start.elapsed() < Duration::from_secs(2));
                std::thread::yield_now();
            }
            stats.cancel_stream();
            let outcome = result.recv_timeout(Duration::from_secs(1));
            // Also clean up promptly if a regression loses the cancellation wake.
            if outcome.is_err() {
                state.ready.notify_all();
            }
            assert_eq!(outcome.unwrap(), io::ErrorKind::Interrupted);
            thread.join().unwrap();
            drop(state);
            assert!(
                weak.upgrade().is_none(),
                "Cancellation introduced a strong ownership cycle"
            );
            assert!(!path.exists(), "Unused spool file was retained");
        }
    }
    #[test]
    #[ignore = "Explicit alternating-order hot-path benchmark; not a timing assertion"]
    fn pcm_hot_path_benchmark() {
        use std::{hint::black_box, time::Instant};
        use symphonia::core::audio::{AudioBuffer, AudioBufferRef, Channels, Signal, SignalSpec};
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/24bit-192khz.flac");
        let mut decoder = Prepared::local(&path).unwrap().decoder;
        let mut packet = AudioBuffer::<f32>::new(
            2048,
            SignalSpec::new(192000, Channels::FRONT_LEFT | Channels::FRONT_RIGHT),
        );
        packet.render_reserved(None);
        for channel in 0..2 {
            for (i, sample) in packet.chan_mut(channel).iter_mut().enumerate() {
                *sample = (i as f32 + channel as f32) / 8192.;
            }
        }
        decoder
            .buffer
            .copy(AudioBufferRef::F32(std::borrow::Cow::Borrowed(&packet)));
        let mut copies = [vec![], vec![]];
        let mut clocks = [vec![], vec![]];
        let batches = 2000;
        let samples = 4096 * batches;
        for round in 0..7 {
            for old in if round % 2 == 0 {
                [true, false]
            } else {
                [false, true]
            } {
                let mut output = Vec::with_capacity(4096);
                let started = Instant::now();
                for _ in 0..batches {
                    decoder.position = 0;
                    output.clear();
                    if old {
                        output.extend(decoder.by_ref().take(4096));
                    } else {
                        decoder.copy_samples(&mut output, 4096);
                    }
                    black_box(output.as_slice());
                }
                copies[usize::from(!old)].push(started.elapsed().as_secs_f64() * 1000.);
                let stats = black_box(AudioStats::default());
                let started = Instant::now();
                for sample in 1..=samples as u64 {
                    let stats = black_box(&stats);
                    if old {
                        stats.played_samples.fetch_add(1, Ordering::Relaxed);
                        let consumed = stats.consumed_samples.load(Ordering::Relaxed);
                        stats
                            .consumed_samples
                            .store(consumed + 1, Ordering::Relaxed);
                    } else {
                        stats.publish_sample(sample, sample);
                    }
                }
                clocks[usize::from(!old)].push(started.elapsed().as_secs_f64() * 1000.);
                assert_eq!(stats.played_samples.load(Ordering::Relaxed), samples as u64);
                assert_eq!(
                    stats.consumed_samples.load(Ordering::Relaxed),
                    samples as u64
                );
            }
        }
        for times in copies.iter_mut().chain(clocks.iter_mut()) {
            times.sort_by(f64::total_cmp);
        }
        let report = serde_json::json!({"scope":"Seven alternating-order batches; 2000 already-decoded 4096-float stereo packet copies and 8192000 exact sample-clock updates per batch. Isolated hot paths, not total playback CPU or decode throughput.","copy_scalar_median_ms":copies[0][3],"copy_bulk_median_ms":copies[1][3],"clock_previous_median_ms":clocks[0][3],"clock_single_writer_median_ms":clocks[1][3]});
        println!("{report}");
        if let Ok(path) = std::env::var("UNDERTOW_BENCH_REPORT") {
            crate::config::write_json(Path::new(&path), &report).unwrap();
        }
    }
    #[test]
    fn bulk_pcm_matches_scalar_across_packets_partial_batches_seeks_and_eof() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/24bit-192khz.flac");
        for seek_ms in [0, 2, 8] {
            let mut scalar = Prepared::local(&path).unwrap().decoder;
            let mut bulk = Prepared::local(&path).unwrap().decoder;
            scalar.try_seek(Duration::from_millis(seek_ms)).unwrap();
            bulk.try_seek(Duration::from_millis(seek_ms)).unwrap();
            let expected: Vec<_> = scalar.collect();
            let mut actual = Vec::new();
            for count in [1, 7, 4095, 4096, 4097, 32_768] {
                bulk.copy_samples(&mut actual, count);
            }
            while actual.len() < expected.len() {
                let before = actual.len();
                bulk.copy_samples(&mut actual, 4096);
                assert!(
                    actual.len() > before,
                    "Bulk decoder stopped before scalar EOF"
                );
            }
            assert_eq!(actual, expected);
            let end = actual.len();
            bulk.copy_samples(&mut actual, 4096);
            assert_eq!(actual.len(), end);
        }
    }
    #[test]
    fn pcm_recycles_consumed_and_stale_packets_without_changing_samples_or_clocks() {
        let (tx, rx) = mpsc::sync_channel(4);
        let (recycle, reused) = mpsc::sync_channel(4);
        let stats = Arc::new(AudioStats::default());
        let mut source = PcmSource {
            rx,
            recycle,
            decoder_thread: std::thread::current(),
            buffer: vec![],
            buffer_start: 0,
            pos: 0,
            consumed: 0,
            epoch: 0,
            stats: stats.clone(),
            rate: 44100,
            channels: 2,
            duration: None,
        };
        tx.send(Packet {
            samples: vec![1., 2.],
            epoch: 0,
            start_sample: 0,
        })
        .unwrap();
        tx.send(Packet {
            samples: vec![3., 4.],
            epoch: 0,
            start_sample: 2,
        })
        .unwrap();
        assert_eq!(
            source.by_ref().take(3).collect::<Vec<_>>(),
            vec![1., 2., 3.]
        );
        assert_eq!(reused.try_recv().unwrap(), vec![1., 2.]);
        stats.epoch.store(1, Ordering::Release);
        tx.send(Packet {
            samples: vec![5., 6.],
            epoch: 0,
            start_sample: 4,
        })
        .unwrap();
        tx.send(Packet {
            samples: vec![7., 8.],
            epoch: 1,
            start_sample: 100,
        })
        .unwrap();
        assert_eq!(source.by_ref().take(2).collect::<Vec<_>>(), vec![7., 8.]);
        assert_eq!(stats.played_samples.load(Ordering::Relaxed), 102);
        assert_eq!(stats.consumed_samples.load(Ordering::Relaxed), 5);
        assert_eq!(stats.underrun_samples.load(Ordering::Relaxed), 0);
        assert_eq!(reused.try_iter().count(), 2);
    }

    #[test]
    fn timeline_holds_latest_seek_until_matching_pcm_is_consumed() {
        let rate = 88_200.;
        let stats = AudioStats::default();
        assert_eq!(stats.position(rate), 0.);
        stats.publish_sample(88_200, 88_200);
        assert_eq!(stats.position(rate), 1.);
        stats.seek_ms.store(38_500, Ordering::Relaxed);
        stats.epoch.store(2, Ordering::Release);
        stats.publish_sample(500_000, 88_201);
        stats.played_epoch.store(1, Ordering::Release);
        assert_eq!(stats.position(rate), 38.5);
        let played = (38.5 * rate) as u64 + 1;
        stats.publish_sample(played, 88_202);
        stats.played_epoch.store(2, Ordering::Release);
        assert_eq!(stats.position(rate), played as f64 / rate);
    }

    #[test]
    fn eof_from_before_a_seek_cannot_end_the_new_generation() {
        let (tx, rx) = mpsc::sync_channel(1);
        let (recycle, _reused) = mpsc::sync_channel(1);
        let stats = Arc::new(AudioStats::default());
        let mut source = PcmSource {
            rx,
            recycle,
            decoder_thread: std::thread::current(),
            buffer: vec![],
            buffer_start: 0,
            pos: 0,
            consumed: 0,
            epoch: 0,
            stats: stats.clone(),
            rate: 44100,
            channels: 2,
            duration: None,
        };
        stats.epoch.store(1, Ordering::Release);
        stats.eof_epoch.store(1, Ordering::Release);
        assert_eq!(source.next(), Some(0.));
        tx.send(Packet {
            samples: vec![0.5],
            epoch: 1,
            start_sample: 0,
        })
        .unwrap();
        stats.eof_epoch.store(2, Ordering::Release);
        assert_eq!(source.next(), Some(0.5));
        assert_eq!(source.next(), None);
    }
    #[test]
    fn consuming_pcm_wakes_the_waiting_producer_and_drop_cancels_it() {
        let (tx, rx) = mpsc::sync_channel(1);
        let (ready_tx, ready_rx) = mpsc::channel();
        let (awake_tx, awake_rx) = mpsc::channel();
        let producer = std::thread::spawn(move || {
            tx.send(Packet {
                samples: vec![0.5],
                epoch: 0,
                start_sample: 0,
            })
            .unwrap();
            ready_tx.send(()).unwrap();
            std::thread::park();
            awake_tx.send(()).unwrap();
        });
        ready_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let stats = Arc::new(AudioStats::default());
        let mut source = PcmSource {
            rx,
            recycle: mpsc::sync_channel(1).0,
            decoder_thread: producer.thread().clone(),
            buffer: vec![],
            buffer_start: 0,
            pos: 0,
            consumed: 0,
            epoch: 0,
            stats: stats.clone(),
            rate: 44100,
            channels: 1,
            duration: None,
        };
        assert_eq!(source.next(), Some(0.5));
        awake_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        drop(source);
        assert!(stats.cancel.load(Ordering::Relaxed));
        producer.join().unwrap();
    }
    #[test]
    fn fragmented_flac_plays_and_seeks_before_final_segment_arrives() {
        use std::net::TcpListener;
        let bytes = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/24bit-192khz.m4a"),
        )
        .unwrap();
        let mut cut = 0;
        while cut + 8 < bytes.len() {
            let size = u32::from_be_bytes(bytes[cut..cut + 4].try_into().unwrap()) as usize;
            if &bytes[cut + 4..cut + 8] == b"moof" {
                break;
            }
            assert!(size >= 8);
            cut += size;
        }
        assert!(cut > 0 && cut < bytes.len());
        let segments = [
            bytes[..cut].to_vec(),
            bytes[cut..].to_vec(),
            vec![0, 0, 0, 8, b'f', b'r', b'e', b'e'],
        ];
        let total = segments.iter().map(|s| s.len() as u64).sum::<u64>();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let (release, gate) = mpsc::channel();
        let server = std::thread::spawn(move || {
            for (i, segment) in segments.iter().enumerate() {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut request = Vec::new();
                let mut b = [0; 1];
                while !request.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut b).unwrap();
                    request.push(b[0]);
                }
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    segment.len()
                )
                .unwrap();
                if i == 2 {
                    let _ = gate.recv_timeout(Duration::from_secs(3));
                }
                stream.write_all(segment).unwrap();
            }
        });
        let plan = StreamPlan {
            urls: (0..3).map(|n| format!("{base}/{n}")).collect(),
            encryption: None,
            quality: "HI_RES_LOSSLESS".into(),
            codec: "flac".into(),
            bit_depth: 24,
            sample_rate: 192000,
        };
        let mut prepared = Prepared::remote(plan, true).unwrap();
        let mut samples = Vec::new();
        prepared.decoder.copy_samples(&mut samples, 4096);
        assert_eq!(samples.len(), 4096);
        assert!(samples.iter().any(|s| s.abs() > 0.0001));
        assert!(
            prepared.stats.downloaded.load(Ordering::Relaxed) < total,
            "Playback waited for the final segment"
        );
        prepared.decoder.try_seek(Duration::from_millis(2)).unwrap();
        assert!(prepared.decoder.next().is_some());
        release.send(()).unwrap();
        server.join().unwrap();
    }
    #[test]
    fn encrypted_flac_resumes_at_the_exact_byte_after_a_cdn_drop() {
        use ctr::cipher::{KeyIvInit, StreamCipher};
        use std::{net::TcpListener, time::Instant};
        let bytes = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/24bit-192khz.flac"),
        )
        .unwrap();
        let key = [0x11u8; 16];
        let nonce = [0x22u8; 8];
        let mut iv = [0; 16];
        iv[..8].copy_from_slice(&nonce);
        let mut encrypted = bytes.clone();
        ctr::Ctr128BE::<aes::Aes128>::new(&key.into(), &iv.into()).apply_keystream(&mut encrypted);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/track", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let cut = encrypted.len() / 2 + 3;
            for call in 0..2 {
                let deadline = Instant::now() + Duration::from_secs(5);
                let (mut s, _) = loop {
                    if let Ok(c) = listener.accept() {
                        break c;
                    }
                    assert!(Instant::now() < deadline, "Expected safe resume request");
                    std::thread::sleep(Duration::from_millis(5));
                };
                // Accepted sockets can inherit the listener's nonblocking mode on Windows.
                s.set_nonblocking(false).unwrap();
                s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
                let mut headers = Vec::new();
                let mut one = [0; 1];
                while !headers.ends_with(b"\r\n\r\n") {
                    s.read_exact(&mut one).unwrap();
                    headers.push(one[0]);
                }
                if call == 0 {
                    write!(
                        s,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        encrypted.len()
                    )
                    .unwrap();
                    s.write_all(&encrypted[..cut]).unwrap();
                } else {
                    let request = String::from_utf8(headers).unwrap().to_lowercase();
                    assert!(request.contains(&format!("range: bytes={cut}-")));
                    write!(s,"HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {}-{}/{}\r\nConnection: close\r\n\r\n",encrypted.len()-cut,cut,encrypted.len()-1,encrypted.len()).unwrap();
                    s.write_all(&encrypted[cut..]).unwrap();
                }
            }
        });
        let plan = StreamPlan {
            urls: vec![url],
            encryption: Some((key, nonce)),
            quality: "LOSSLESS".into(),
            codec: "flac".into(),
            bit_depth: 24,
            sample_rate: 192000,
        };
        let mut remote = Prepared::remote(plan, true).unwrap();
        let mut local = Prepared::local(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/24bit-192khz.flac"),
        )
        .unwrap();
        let remote_samples = remote.decoder.by_ref().collect::<Vec<_>>();
        let local_samples = local.decoder.by_ref().collect::<Vec<_>>();
        assert!(
            remote_samples == local_samples,
            "Resumed PCM differs from local decode ({} versus {} samples)",
            remote_samples.len(),
            local_samples.len()
        );
        assert!(!remote_samples.is_empty());
        server.join().unwrap();
    }
    #[test]
    fn flac_and_fragmented_mp4_are_verified_from_codec_headers() {
        for file in ["24bit-192khz.flac", "24bit-192khz.m4a"] {
            let p = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(file);
            let mut prepared = Prepared::local(&p).unwrap();
            assert_eq!(prepared.format.sample_rate, 192000);
            assert_eq!(prepared.format.bit_depth, 24);
            assert_eq!(prepared.format.channels, 2);
            assert!(prepared.format.lossless);
            let samples: Vec<_> = prepared.decoder.by_ref().take(4096).collect();
            assert_eq!(samples.len(), 4096);
            assert!(samples.iter().all(|s| s.is_finite()));
            assert!(samples.iter().any(|s| s.abs() > 0.00001));
        }
    }
    #[test]
    fn decoder_seek_returns_audio_after_the_target() {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/24bit-192khz.flac");
        let mut prepared = Prepared::local(&p).unwrap();
        prepared
            .decoder
            .try_seek(Duration::from_millis(150))
            .unwrap();
        assert!(prepared.decoder.next().is_some());
    }
    #[test]
    fn pcm_underrun_does_not_block_or_advance_lyrics() {
        let (_tx, rx) = mpsc::sync_channel(1);
        let stats = Arc::new(AudioStats::default());
        let mut s = PcmSource {
            rx,
            recycle: mpsc::sync_channel(1).0,
            decoder_thread: std::thread::current(),
            buffer: vec![],
            buffer_start: 0,
            pos: 0,
            consumed: 0,
            epoch: 0,
            stats: stats.clone(),
            rate: 44100,
            channels: 2,
            duration: None,
        };
        assert_eq!(s.next(), Some(0.0));
        assert_eq!(stats.played_samples.load(Ordering::Relaxed), 0);
        assert_eq!(stats.consumed_samples.load(Ordering::Relaxed), 0);
        assert_eq!(stats.underrun_samples.load(Ordering::Relaxed), 1);
    }
    #[test]
    fn consumed_audio_excludes_seek_offsets_and_underrun_silence() {
        let (tx, rx) = mpsc::sync_channel(2);
        let stats = Arc::new(AudioStats::default());
        tx.send(Packet {
            samples: vec![0.1, 0.2, 0.3, 0.4],
            epoch: 0,
            start_sample: 0,
        })
        .unwrap();
        let mut source = PcmSource {
            rx,
            recycle: mpsc::sync_channel(1).0,
            decoder_thread: std::thread::current(),
            buffer: vec![],
            buffer_start: 0,
            pos: 0,
            consumed: 0,
            epoch: 0,
            stats: stats.clone(),
            rate: 44100,
            channels: 2,
            duration: None,
        };
        for _ in 0..4 {
            assert!(source.next().is_some());
        }
        // A decoder seek changes the position clock but is not consumed audio.
        stats.seek_ms.store(60_000, Ordering::Relaxed);
        stats.epoch.store(1, Ordering::Release);
        assert_eq!(source.next(), Some(0.));
        assert_eq!(stats.consumed_samples.load(Ordering::Relaxed), 4);
        assert_eq!(stats.position(88_200.), 60.);
        tx.send(Packet {
            samples: vec![0.5, 0.6],
            epoch: 1,
            start_sample: 88_200 * 60,
        })
        .unwrap();
        assert_eq!(source.next(), Some(0.5));
        assert_eq!(source.next(), Some(0.6));
        assert_eq!(stats.position(88_200.), (88_200. * 60. + 2.) / 88_200.);
        assert_eq!(stats.consumed_samples.load(Ordering::Relaxed), 6);
        assert_eq!(stats.underrun_samples.load(Ordering::Relaxed), 1);
    }
    #[test]
    fn cancelled_source_ends() {
        let (_tx, rx) = mpsc::sync_channel(1);
        let stats = Arc::new(AudioStats::default());
        stats.cancel.store(true, Ordering::Relaxed);
        let mut s = PcmSource {
            rx,
            recycle: mpsc::sync_channel(1).0,
            decoder_thread: std::thread::current(),
            buffer: vec![],
            buffer_start: 0,
            pos: 0,
            consumed: 0,
            epoch: 0,
            stats,
            rate: 44100,
            channels: 2,
            duration: None,
        };
        assert_eq!(s.next(), None);
    }
}
