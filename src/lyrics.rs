use crate::{
    backend::Tidal,
    config::{data_dir, read_json, write_json},
    model::Track,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, io::Read, path::Path, sync::Mutex, time::Duration};

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
pub struct Word {
    pub text: String,
    pub start: f64,
    pub end: f64,
}
impl Word {
    pub fn end_time(&self) -> f64 {
        if self.end > self.start {
            self.end
        } else {
            self.start + 0.06
        }
    }
}
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Line {
    pub text: String,
    pub start: f64,
    pub words: Vec<Word>,
    pub translated: String,
    pub romanized: String,
}
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Lyrics {
    pub lines: Vec<Line>,
    pub synced: bool,
    pub source: String,
    pub instrumental: bool,
    #[serde(default)]
    pub(crate) radiant_checked: bool,
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
    /// Zero means a word fill is moving; otherwise wake at the next visible
    /// timestamp. Playing audio alone does not require a display-rate redraw.
    pub fn next_visual_change(&self, seconds: f64, animate_words: bool) -> Option<f64> {
        if !self.synced || !seconds.is_finite() {
            return None;
        }
        let active = self.active(seconds);
        let next_line = self.lines.get(active.map_or(0, |i| i + 1)).map(|l| l.start);
        let mut next = next_line;
        if let Some(line) = active.and_then(|i| self.lines.get(i)) {
            for word in &line.words {
                if word.text.trim().is_empty() {
                    continue;
                }
                let end = word.end_time();
                if animate_words && seconds >= word.start && seconds < end {
                    return Some(0.);
                }
                if word.start > seconds && next.is_none_or(|n| word.start < n) {
                    next = Some(word.start);
                }
            }
        }
        next.filter(|n| n.is_finite())
            .map(|n| (n - seconds).max(0.))
    }
    pub fn word_synced(&self) -> bool {
        self.synced
            && self
                .lines
                .iter()
                .any(|line| line.words.iter().any(|w| !w.text.trim().is_empty()))
    }
    pub(crate) fn timing_quality(&self) -> u8 {
        u8::from(self.synced) + u8::from(self.word_synced())
    }
    pub(crate) fn needs_timing_upgrade(&self, radiant: bool) -> bool {
        radiant
            && !self.radiant_checked
            && !self.instrumental
            && self.source != "Local LRC"
            && !self.word_synced()
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
        let mut words = Vec::new();
        let mut text = String::new();
        let mut r = rest;
        while let Some(begin) = r.find('<') {
            text.push_str(&r[..begin]);
            let Some(end) = r[begin + 1..].find('>') else {
                break;
            };
            let end = begin + 1 + end;
            let Some(t) = stamp(&r[begin + 1..end]) else {
                break;
            };
            r = &r[end + 1..];
            let next = r.find('<').unwrap_or(r.len());
            let w = &r[..next];
            text.push_str(w);
            words.push(Word {
                text: w.into(),
                start: t,
                end: t,
            });
            r = &r[next..];
        }
        text.push_str(r);
        if words.is_empty() {
            text = rest.into();
        }
        for i in 0..words.len().saturating_sub(1) {
            words[i].end = words[i + 1].start;
        }
        for start in starts {
            result.lines.push(Line {
                text: text.trim().into(),
                start,
                words: words.clone(),
                ..Default::default()
            });
        }
    }
    for l in &mut result.lines {
        l.start = (l.start + offset).max(0.0);
        for w in &mut l.words {
            w.start = (w.start + offset).max(0.0);
            w.end = (w.end + offset).max(0.0);
        }
    }
    result.lines.sort_by(|a, b| a.start.total_cmp(&b.start));
    for i in 0..result.lines.len() {
        let end = result
            .lines
            .get(i + 1)
            .map(|l| l.start)
            .unwrap_or(result.lines[i].start + 10.0);
        if let Some(w) = result.lines[i].words.last_mut() {
            w.end = end.max(w.start);
        }
    }
    result
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
        radiant_checked: false,
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
pub fn save(track: &Track, l: &Lyrics) -> Result<()> {
    static WRITES: Mutex<()> = Mutex::new(());
    let _guard = WRITES.lock().unwrap();
    if cached(track).is_some_and(|saved| saved.timing_quality() > l.timing_quality()) {
        return Ok(());
    }
    let p = cache_path(track);
    write_json(&p, l)?;
    prune_cache(p.parent().unwrap(), 16 * 1024 * 1024)?;
    Ok(())
}

pub fn fetch(track: &Track, tidal: Option<&mut Tidal>, radiant: bool) -> Result<Lyrics> {
    if let Some(saved) = cached(track) {
        return if saved.needs_timing_upgrade(radiant) {
            fetch_with_base(track, Some(saved), radiant)
        } else {
            Ok(saved)
        };
    }
    fetch_fresh(track, tidal, radiant)
}

/// Explicit refresh bypasses saved timing data; a failed refresh keeps the old cache intact.
pub fn fetch_fresh(track: &Track, tidal: Option<&mut Tidal>, radiant: bool) -> Result<Lyrics> {
    let base = tidal
        .filter(|_| track.local_path.is_empty())
        .and_then(|t| t.tidal_lyrics(track.id).ok())
        .and_then(from_tidal);
    fetch_with_base(track, base, radiant)
}

pub(crate) fn from_tidal((synced, text): (String, String)) -> Option<Lyrics> {
    let mut lyrics = parse_lrc(&synced);
    if lyrics.lines.is_empty() {
        lyrics = plain(&text, "TIDAL");
    }
    lyrics.source = "TIDAL".into();
    (!lyrics.lines.is_empty()).then_some(lyrics)
}

pub(crate) fn fetch_with_base(
    track: &Track,
    mut base: Option<Lyrics>,
    radiant: bool,
) -> Result<Lyrics> {
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
    let checked_radiant = radiant && base.as_ref().is_none_or(|l| l.needs_timing_upgrade(true));
    if checked_radiant
        && let Ok(candidate) = fetch_radiant(track)
        && base
            .as_ref()
            .is_none_or(|current| candidate.timing_quality() > current.timing_quality())
    {
        base = Some(candidate);
    }
    if let Some(mut l) = base {
        l.radiant_checked |= checked_radiant;
        let _ = save(track, &l);
        return Ok(l);
    }
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
    l.radiant_checked = checked_radiant;
    ensure!(
        !l.lines.is_empty() || l.instrumental,
        "No lyrics available from TIDAL, Radiant or LRCLIB"
    );
    save(track, &l)?;
    Ok(l)
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
fn fetch_radiant(t: &Track) -> Result<Lyrics> {
    // The public fallback used by Radiant. No extracted provider credentials or IP lookup.
    let v = limited_json(
        crate::auth::http()?
            .get("https://rl-api.kineticsand.net/lyrics")
            .timeout(Duration::from_secs(5))
            .query(&[
                ("title", &t.title),
                ("artist", &t.artist),
                ("isrc", &t.isrc),
                ("ai-preference", &"false".to_string()),
            ])
            .send()?,
    )?;
    parse_radiant(&v)
}

fn parse_radiant(v: &Value) -> Result<Lyrics> {
    ensure!(
        v["_synthesized"].as_bool() != Some(true),
        "Synthesized lyric timings were not accepted"
    );
    let data = v["data"]
        .as_array()
        .context("Unsupported Radiant response")?;
    ensure!(data.len() <= 4000, "Too many lyric lines");
    let mut lines = Vec::new();
    for l in data {
        // Radiant line timestamps are seconds; syllabus timestamps are milliseconds.
        let start = l["startTime"].as_f64().unwrap_or(0.0);
        ensure!(start.is_finite() && start >= 0.0, "Invalid lyric timestamp");
        let words = l["syllabus"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|w| {
                let text = w["text"].as_str()?;
                let time = w["time"].as_f64()?;
                let duration = w["duration"].as_f64().unwrap_or(0.);
                let end = time + duration;
                (!text.trim().is_empty() && time >= 0. && duration >= 0. && end.is_finite()).then(
                    || Word {
                        text: text.into(),
                        start: time / 1000.,
                        end: end / 1000.,
                    },
                )
            })
            .collect();
        lines.push(Line {
            text: l["text"].as_str().unwrap_or("").into(),
            start,
            words,
            translated: l["translation"].as_str().unwrap_or("").into(),
            romanized: l["romanized"].as_str().unwrap_or("").into(),
        });
    }
    lines.sort_by(|a, b| a.start.total_cmp(&b.start));
    ensure!(
        lines.iter().any(|line| !line.text.trim().is_empty()),
        "No Radiant lyrics found"
    );
    Ok(Lyrics {
        lines,
        synced: true,
        source: "Radiant public provider".into(),
        instrumental: false,
        radiant_checked: true,
    })
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
    fn genuine_word_times() {
        let l = parse_lrc("[00:01.00]<00:01.00>Hello <00:02.00>世界\n[00:04.00]next");
        assert_eq!(l.lines[0].text, "Hello 世界");
        assert_eq!(l.lines[0].words[0].end, 2.0);
        assert_eq!(l.lines[0].words[1].end, 4.0);
    }
    #[test]
    fn radiant_keeps_short_syllables_and_ignores_invalid_cues() {
        let mut payload = serde_json::json!({
            "data": [{
                "startTime": 1.0,
                "text": "Hello again",
                "syllabus": [
                    {"text": "Hel", "time": 1000, "duration": 32},
                    {"text": "lo", "time": 1080, "duration": 200},
                    {"text": "again", "time": 1450, "duration": 300},
                    {"text": "invalid", "duration": 100}
                ]
            }]
        });
        let lyrics = parse_radiant(&payload).unwrap();
        assert_eq!(lyrics.lines[0].words.len(), 3);
        assert_eq!(lyrics.lines[0].words[0].text, "Hel");
        assert_eq!(lyrics.lines[0].words[0].end_time(), 1.032);
        assert_eq!(lyrics.next_visual_change(1.025, true), Some(0.));
        assert!((lyrics.next_visual_change(1.05, true).unwrap() - 0.03).abs() < 1e-9);
        assert!(lyrics.timing_quality() > parse_lrc("[00:01.00]Hello again").timing_quality());
        assert!(!lyrics.needs_timing_upgrade(true));
        payload["_synthesized"] = true.into();
        assert!(parse_radiant(&payload).is_err());
    }

    #[test]
    fn an_old_line_only_cache_can_upgrade_when_the_provider_is_enabled() {
        let mut lyrics: Lyrics = serde_json::from_value(serde_json::json!({
            "lines": [{"text": "Original lyrics", "start": 1.0, "words": [], "translated": "", "romanized": ""}],
            "synced": true, "source": "TIDAL", "instrumental": false
        })).unwrap();
        assert!(lyrics.needs_timing_upgrade(true));
        assert!(!lyrics.needs_timing_upgrade(false));
        lyrics.radiant_checked = true;
        assert!(!lyrics.needs_timing_upgrade(true));
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
        let first = fetch_fresh(&track, None, false).unwrap();
        std::fs::write(&lrc, "[00:04.00]Corrected timing").unwrap();
        let refreshed = fetch_fresh(&track, None, false).unwrap();
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
    fn settled_lines_sleep_until_the_next_timestamp_but_words_keep_their_fill() {
        assert_eq!(follow_step(0.2, 0.001, true), 0.001);
        assert_eq!(follow_step(0.2, 3., false), 0.1);
        let mut data = parse_lrc("[00:00.00]First line\n[00:06.00]Second line");
        assert_eq!(data.next_visual_change(1., true), Some(5.));
        assert_eq!(data.next_visual_change(-1., true), Some(1.));
        assert_eq!(data.next_visual_change(7., true), None);
        data.lines[0].words = vec![
            Word {
                text: "First".into(),
                start: 1.,
                end: 2.,
            },
            Word {
                text: "line".into(),
                start: 3.,
                end: 4.,
            },
        ];
        assert_eq!(data.next_visual_change(1.5, true), Some(0.));
        assert_eq!(data.next_visual_change(2.5, true), Some(0.5));
        assert_eq!(data.next_visual_change(1.5, false), Some(1.5));
        assert_eq!(data.next_visual_change(f64::NAN, true), None);
        data.synced = false;
        assert_eq!(data.next_visual_change(1., true), None);
    }
}
