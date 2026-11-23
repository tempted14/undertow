use crate::{
    backend::Tidal,
    config::{data_dir, read_json, write_json},
    model::Track,
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, io::Read, path::Path, sync::Mutex};

mod ttml;

/// Exact critically damped spring: consistent motion at 30, 60, or 120 Hz.
pub fn follow_scroll(position: &mut f32, velocity: &mut f32, target: f32, dt: f32) {
    let omega = 20.;
    let dt = dt.clamp(0., 0.1);
    let offset = *position - target;
    let c = *velocity + omega * offset;
    let decay = (-omega * dt).exp();
    *position = target + (offset + c * dt) * decay;
    *velocity = (*velocity - omega * c * dt) * decay;
    if (*position - target).abs() < 0.02 && velocity.abs() < 0.05 {
        *position = target;
        *velocity = 0.;
    }
}

/// A sleeping UI must not consume the idle interval as the first spring step.
pub fn follow_step(elapsed: f32, line_age: f32, changed: bool) -> f32 {
    let dt = elapsed.clamp(0., 0.1);
    if changed {
        dt.min(line_age.max(0.))
    } else {
        dt
    }
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Line {
    pub text: String,
    pub start: f64,
    pub translated: String,
    pub romanized: String,
}
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Lyrics {
    pub lines: Vec<Line>,
    pub synced: bool,
    pub source: String,
    pub instrumental: bool,
}
impl Lyrics {
    pub fn active(&self, seconds: f64) -> Option<usize> {
        if !self.synced {
            return None;
        }
        self.lines
            .partition_point(|l| l.start <= seconds)
            .checked_sub(1)
    }
    pub fn plain(&self) -> String {
        self.lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub fn next_line_change(&self, seconds: f64) -> Option<f64> {
        if !self.synced || !seconds.is_finite() {
            return None;
        }
        self.lines
            .get(self.active(seconds).map_or(0, |i| i + 1))
            .map(|line| line.start - seconds)
    }
}

fn stamp(s: &str) -> Option<f64> {
    let mut p = s.split(':');
    let m = p.next()?.parse::<f64>().ok()?;
    let sec = p.next()?.parse::<f64>().ok()?;
    if p.next().is_some() || !(0.0..60.0).contains(&sec) || !m.is_finite() || m < 0.0 {
        return None;
    }
    Some(m * 60.0 + sec)
}
pub fn parse_lrc(s: &str) -> Lyrics {
    let mut result = Lyrics {
        synced: true,
        ..Default::default()
    };
    let mut offset = 0.0;
    for raw in s.lines().take(4000) {
        let raw = raw.trim().trim_start_matches('\u{feff}');
        if let Some(o) = raw
            .strip_prefix("[offset:")
            .and_then(|x| x.strip_suffix(']'))
            .and_then(|x| x.parse::<f64>().ok())
        {
            if o.is_finite() {
                offset = o / 1000.0;
            }
            continue;
        }
        let mut rest = raw;
        let mut starts = Vec::new();
        while let Some(x) = rest.strip_prefix('[') {
            let Some(end) = x.find(']') else {
                break;
            };
            if let Some(t) = stamp(&x[..end]) {
                starts.push(t);
                rest = &x[end + 1..];
            } else {
                break;
            }
        }
        if starts.is_empty() {
            continue;
        }
        let text = strip_inline_timing(rest);
        for start in starts {
            result.lines.push(Line {
                text: text.clone(),
                start,
                ..Default::default()
            });
        }
    }
    for line in &mut result.lines {
        line.start = (line.start + offset).max(0.);
    }
    result.lines.sort_by(|a, b| a.start.total_cmp(&b.start));
    result
}

/// Keep older enhanced LRC files readable using their line timestamps.
fn strip_inline_timing(line: &str) -> String {
    let mut text = String::new();
    let mut scan = 0;
    let mut text_start = 0;
    while let Some(begin) = line[scan..].find('<').map(|i| scan + i) {
        let Some(end) = line[begin + 1..].find('>').map(|i| begin + 1 + i) else {
            break;
        };
        scan = end + 1;
        if stamp(&line[begin + 1..end]).is_some() {
            text.push_str(&line[text_start..begin]);
            text_start = scan;
        }
    }
    text.push_str(&line[text_start..]);
    text.trim().into()
}

fn plain(s: &str, source: &str) -> Lyrics {
    Lyrics {
        lines: s
            .lines()
            .take(4000)
            .map(|t| Line {
                text: t.to_string(),
                ..Default::default()
            })
            .collect(),
        synced: false,
        source: source.into(),
        instrumental: false,
    }
}
fn key(track: &Track) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(format!(
        "{}\0{}\0{}\0{}",
        track.id, track.title, track.artist, track.duration
    ));
    format!("{:x}", hash)
}
pub fn cache_path(track: &Track) -> std::path::PathBuf {
    data_dir()
        .join("lyrics")
        .join(format!("{}.json", key(track)))
}
pub fn cached(track: &Track) -> Option<Lyrics> {
    read_json(&cache_path(track)).ok()
}
fn keep_best(path: &Path, lyrics: Lyrics) -> Lyrics {
    static WRITES: Mutex<()> = Mutex::new(());
    let _guard = WRITES.lock().unwrap();
    if let Ok(saved) = read_json::<Lyrics>(path)
        && saved.synced
        && !lyrics.synced
    {
        return saved;
    }
    if write_json(path, &lyrics).is_ok()
        && let Some(parent) = path.parent()
    {
        let _ = prune_cache(parent, 16 * 1024 * 1024);
    }
    lyrics
}

pub fn fetch(track: &Track, tidal: Option<&mut Tidal>) -> Result<Lyrics> {
    if let Some(saved) = cached(track) {
        return Ok(saved);
    }
    fetch_fresh(track, tidal)
}

/// An explicit refresh rereads the source and keeps a usable cache on failure.
fn fetch_fresh(track: &Track, tidal: Option<&mut Tidal>) -> Result<Lyrics> {
    let base = tidal
        .filter(|_| track.local_path.is_empty())
        .and_then(|t| t.tidal_lyrics(track.id).ok())
        .and_then(from_tidal);
    fetch_with_base(track, base)
}

pub(crate) fn from_tidal((synced, text): (String, String)) -> Option<Lyrics> {
    let mut lyrics = if synced.trim_start().starts_with('<') {
        ttml::parse(&synced).unwrap_or_default()
    } else {
        parse_lrc(&synced)
    };
    if lyrics.lines.is_empty() {
        lyrics = plain(&text, "TIDAL");
    }
    lyrics.source = "TIDAL".into();
    (!lyrics.lines.is_empty()).then_some(lyrics)
}

pub(crate) fn fetch_with_base(track: &Track, base: Option<Lyrics>) -> Result<Lyrics> {
    if !track.local_path.is_empty() {
        let p = Path::new(&track.local_path).with_extension("lrc");
        if p.exists() {
            let text = std::fs::read_to_string(p)?;
            ensure!(text.len() < 1024 * 1024, "LRC file is too large");
            let mut l = parse_lrc(&text);
            l.source = "Local LRC".into();
            return Ok(l);
        }
    }
    if let Some(lyrics) = base {
        return Ok(keep_best(&cache_path(track), lyrics));
    }
    fetch_lrclib(track).or_else(|error| cached(track).ok_or(error))
}

fn fetch_lrclib(track: &Track) -> Result<Lyrics> {
    let client = crate::auth::http()?;
    let r = client
        .get("https://lrclib.net/api/get")
        .query(&[
            ("track_name", track.title.clone()),
            ("artist_name", track.artist.clone()),
            ("album_name", track.album.clone()),
            ("duration", track.duration.to_string()),
        ])
        .send()?;
    let v = limited_json(r)?;
    let mut l = if let Some(s) = v["syncedLyrics"].as_str().filter(|s| !s.is_empty()) {
        parse_lrc(s)
    } else {
        plain(v["plainLyrics"].as_str().unwrap_or(""), "LRCLIB")
    };
    l.source = "LRCLIB".into();
    l.instrumental = v["instrumental"].as_bool().unwrap_or(false);
    ensure!(
        !l.lines.is_empty() || l.instrumental,
        "No lyrics available from TIDAL or LRCLIB"
    );
    Ok(keep_best(&cache_path(track), l))
}

fn limited_json(r: reqwest::blocking::Response) -> Result<Value> {
    ensure!(
        r.status().is_success(),
        "Lyrics provider returned HTTP {}",
        r.status().as_u16()
    );
    let mut bytes = Vec::new();
    r.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 1024 * 1024, "Lyrics response too large");
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn translate(lyrics: &Lyrics, language: &str, romanize: bool) -> Result<Lyrics> {
    ensure!(
        language.len() <= 12
            && language
                .chars()
                .all(|c| c.is_ascii_alphabetic() || c == '-'),
        "Invalid translation language"
    );
    let client = crate::auth::http()?;
    let mut out = lyrics.clone();
    let mut dedupe: HashMap<String, String> = HashMap::new();
    for line in &mut out.lines {
        if line.text.trim().is_empty() {
            continue;
        }
        let text = if let Some(v) = dedupe.get(&line.text) {
            v.clone()
        } else {
            // One source line per request guarantees alignment even if the provider strips newlines.
            let dt = if romanize { "rm" } else { "t" };
            let r = client
                .post("https://translate.googleapis.com/translate_a/single")
                .query(&[
                    ("client", "gtx"),
                    ("sl", "auto"),
                    ("tl", language),
                    ("dj", "1"),
                    ("dt", dt),
                ])
                .form(&[("q", &line.text)])
                .send()?;
            let v = limited_json(r)?;
            let field = if romanize { "src_translit" } else { "trans" };
            let s = v["sentences"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|s| s[field].as_str())
                .collect::<String>();
            let s = if s.trim().is_empty() {
                line.text.clone()
            } else {
                s.replace('\n', " ")
            };
            dedupe.insert(line.text.clone(), s.clone());
            s
        };
        if romanize {
            line.romanized = text;
        } else {
            line.translated = text;
        }
        if dedupe.len() > 1000 {
            anyhow::bail!("Translation exceeds the 1,000 unique line limit");
        }
    }
    Ok(out)
}

pub fn prune_cache(dir: &Path, budget: u64) -> Result<()> {
    let mut entries = std::fs::read_dir(dir)?
        .filter_map(|x| x.ok())
        .filter_map(|e| {
            e.metadata()
                .ok()
                .map(|m| (e.path(), m.len(), m.modified().ok()))
        })
        .collect::<Vec<_>>();
    let mut total: u64 = entries.iter().map(|e| e.1).sum();
    entries.sort_by_key(|e| e.2);
    for (p, len, _) in entries {
        if total <= budget {
            break;
        }
        if std::fs::remove_file(p).is_ok() {
            total = total.saturating_sub(len);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scroll_spring_is_frame_rate_independent_without_overshoot() {
        let run = |fps: u32| {
            let (mut p, mut v) = (0., 0.);
            for _ in 0..fps {
                follow_scroll(&mut p, &mut v, 500., 1. / fps as f32);
                assert!((0. ..=500.001).contains(&p));
            }
            (p, v)
        };
        let a = run(30);
        let b = run(60);
        let c = run(120);
        assert!((a.0 - c.0).abs() < 0.01 && (b.0 - c.0).abs() < 0.01);
        assert!((c.0 - 500.).abs() < 0.02);
    }
    #[test]
    fn offsets_duplicates_and_prelude() {
        let l = parse_lrc("[offset:-500]\n[00:02.00][00:04.00]hello\n[00:06.00]bye");
        assert_eq!(l.lines.len(), 3);
        assert_eq!(l.active(0.0), None);
        assert_eq!(l.active(1.6), Some(0));
        assert_eq!(l.active(4.0), Some(1));
    }
    #[test]
    fn enhanced_lrc_remains_readable_with_line_timing() {
        let lyrics = parse_lrc(
            "[offset:100]\n[00:02.00][00:12.00]Em<00:02.45>bers<00:02.80>\n[00:20.00]Next",
        );
        assert_eq!(lyrics.lines[0].text, "Embers");
        assert_eq!(lyrics.lines[1].text, "Embers");
        assert_eq!(lyrics.lines[0].start, 2.1);
        assert_eq!(lyrics.lines[1].start, 12.1);
        assert_eq!(lyrics.active(2.8), Some(0));
        assert_eq!(lyrics.active(12.8), Some(1));
    }

    #[test]
    fn old_cached_lyrics_load_without_retaining_subline_data() {
        let old = serde_json::json!({
            "lines": [{
                "text": "Embers", "start": 2.0,
                "words": [{"text":"Em", "start":2.0, "end":2.5}],
                "translated":"", "romanized":""
            }],
            "synced":true, "source":"Cached", "instrumental":false,
            "timing_catalog_checked_at":123
        });
        let lyrics: Lyrics = serde_json::from_value(old).unwrap();
        assert_eq!(lyrics.active(2.1), Some(0));
        assert_eq!(lyrics.plain(), "Embers");
        assert!(
            serde_json::to_value(&lyrics).unwrap()["lines"][0]
                .get("words")
                .is_none()
        );
    }

    #[test]
    fn refreshing_with_plain_lyrics_keeps_cached_line_sync() {
        let dir =
            std::env::temp_dir().join(format!("undertow-lyrics-cache-{}", rand::random::<u64>()));
        let path = dir.join("timing.json");
        keep_best(&path, parse_lrc("[00:02.00]Embers"));
        let result = keep_best(&path, plain("Embers", "Test"));
        assert!(result.synced);
        assert_eq!(result.lines[0].start, 2.);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn plain_lyrics_do_not_get_invented_timestamps() {
        let l = plain("a\nb", "test");
        assert!(!l.synced);
        assert_eq!(l.active(100.0), None);
    }

    #[test]
    fn fresh_lyrics_reread_corrected_local_timing() {
        let file =
            std::env::temp_dir().join(format!("undertow-resync-{}.wav", rand::random::<u64>()));
        let lrc = file.with_extension("lrc");
        let track = crate::model::Track {
            local_path: file.to_string_lossy().into(),
            ..Default::default()
        };
        std::fs::write(&lrc, "[00:02.00]Original timing").unwrap();
        let first = fetch_fresh(&track, None).unwrap();
        std::fs::write(&lrc, "[00:04.00]Corrected timing").unwrap();
        let refreshed = fetch_fresh(&track, None).unwrap();
        std::fs::remove_file(lrc).unwrap();
        assert_eq!(first.active(3.), Some(0));
        assert_eq!(refreshed.active(3.), None);
        assert_eq!(refreshed.active(5.), Some(0));
        assert_eq!(refreshed.lines[0].text, "Corrected timing");
    }
}

#[cfg(test)]
mod pacing_tests {
    use super::*;
    #[test]
    fn settled_lyrics_wait_for_the_next_line() {
        assert_eq!(follow_step(0.2, 0.001, true), 0.001);
        assert_eq!(follow_step(0.2, 3., false), 0.1);
        let mut lyrics = parse_lrc("[00:00.00]First line\n[00:06.00]Second line");
        assert_eq!(lyrics.next_line_change(1.), Some(5.));
        assert_eq!(lyrics.next_line_change(-1.), Some(1.));
        assert_eq!(lyrics.next_line_change(7.), None);
        assert_eq!(lyrics.next_line_change(f64::NAN), None);
        lyrics.synced = false;
        assert_eq!(lyrics.next_line_change(1.), None);
    }
}
