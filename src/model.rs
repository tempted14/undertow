use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Track {
    pub id: u64,
    pub title: String,
    pub artist: String,
    #[serde(default)]
    pub artist_id: u64,
    pub album: String,
    pub album_id: u64,
    pub cover: String,
    pub duration: u64,
    pub quality: String,
    pub isrc: String,
    pub explicit: bool,
    #[serde(default)]
    pub local_path: String,
    #[serde(default)]
    pub playlist_item_id: String,
    #[serde(default)]
    pub playlist_index: Option<usize>,
    #[serde(default)]
    pub date_added: String,
    #[serde(default)]
    pub bit_depth: Option<u32>,
    #[serde(default)]
    pub sample_rate: Option<u32>,
    #[serde(default)]
    pub bitrate: Option<u64>,
}

impl Track {
    pub fn from_v1(v: &serde_json::Value) -> Self {
        let text = |p: &str| {
            v.pointer(p)
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_owned()
        };
        Self {
            id: v["id"].as_u64().unwrap_or_default(),
            title: text("/title"),
            artist_id: v
                .pointer("/artist/id")
                .or_else(|| v.pointer("/artists/0/id"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            artist: {
                let mut names = Vec::new();
                for artist in v["artists"].as_array().into_iter().flatten() {
                    if let Some(name) = artist["name"].as_str()
                        && !names.contains(&name)
                    {
                        names.push(name);
                    }
                }
                if names.is_empty() {
                    text("/artist/name")
                } else {
                    names.join(", ")
                }
            },
            album: text("/album/title"),
            album_id: v
                .pointer("/album/id")
                .and_then(|s| s.as_u64())
                .unwrap_or_default(),
            cover: text("/album/cover"),
            duration: v["duration"].as_u64().unwrap_or_default(),
            quality: if v
                .pointer("/mediaMetadata/tags")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|a| a.iter().any(|s| s.as_str() == Some("HIRES_LOSSLESS")))
            {
                "HI_RES_LOSSLESS".into()
            } else {
                text("/audioQuality")
            },
            isrc: text("/isrc"),
            explicit: v["explicit"].as_bool().unwrap_or(false),
            local_path: String::new(),
            playlist_item_id: String::new(),
            playlist_index: None,
            date_added: text("/dateAdded"),
            bit_depth: v["bitDepth"].as_u64().filter(|n| *n > 0).map(|n| n as u32),
            sample_rate: v["sampleRate"]
                .as_u64()
                .filter(|n| *n > 0)
                .map(|n| n as u32),
            bitrate: v["bitrate"].as_u64().filter(|n| *n > 0),
        }
    }
    pub fn cover_url(&self, size: u32) -> Option<String> {
        if let Some(id) = self.cover.strip_prefix("local-playlist:") {
            return crate::config::playlist_art_path(id)
                .ok()
                .map(|path| format!("local:{}", path.display()));
        }
        if self.cover.is_empty() {
            return None;
        }
        if !self.local_path.is_empty() && self.cover.starts_with("local:") {
            return Some(self.cover.clone());
        }
        if self.cover.starts_with("https://") {
            return Some(self.cover.clone());
        }
        Some(format!(
            "https://resources.tidal.com/images/{}/{}x{}.jpg",
            self.cover.replace('-', "/"),
            size,
            size
        ))
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub title: String,
    pub description: String,
    pub count: u64,
    pub duration: u64,
    pub image: String,
    pub owned: bool,
    #[serde(default)]
    pub creator: String,
    #[serde(default)]
    pub public: Option<bool>,
}

pub struct PlaylistEdit {
    pub id: String,
    pub title: String,
    pub description: String,
    pub public: Option<bool>,
    pub image: Option<std::sync::Arc<Vec<u8>>>,
}

#[derive(Clone, Debug, Default)]
pub struct Album {
    pub id: u64,
    pub title: String,
    pub artist: String,
    pub artist_picture: String,
    pub cover: String,
    pub year: String,
    pub quality: String,
    pub copyright: String,
    pub explicit: bool,
    pub release_type: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CreditGroup {
    pub role: String,
    pub names: String,
}

#[derive(Clone, Debug)]
pub struct CatalogItem {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub artist: String,
    pub cover: String,
    pub year: String,
    pub explicit: bool,
    pub track: Option<Track>,
}

pub fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

pub fn seek_time(pointer: f32, left: f32, width: f32, duration: f64) -> f64 {
    if width <= 0. || duration <= 0. || !duration.is_finite() {
        return 0.;
    }
    ((pointer - left) / width).clamp(0., 1.) as f64 * duration
}

pub fn queue_remaining(queue: &[Track], index: usize, elapsed: f64) -> f64 {
    queue
        .get(index..)
        .unwrap_or_default()
        .iter()
        .map(|t| t.duration as f64)
        .sum::<f64>()
        .sub(elapsed)
        .max(0.0)
}
use std::ops::Sub;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remaining_counts_current_only_once() {
        let q = vec![
            Track {
                duration: 100,
                ..Default::default()
            };
            3
        ];
        assert_eq!(queue_remaining(&q, 1, 15.0), 185.0);
        assert_eq!(queue_remaining(&q, 9, 15.0), 0.0);
    }
    #[test]
    fn metadata_is_not_fabricated() {
        let t =
            Track::from_v1(&serde_json::json!({"id": 4, "title":"A", "artists":[{"name":"B"}]}));
        assert_eq!(t.artist, "B");
        assert_eq!(t.quality, "");
        assert!(t.cover_url(320).is_none());
    }
    #[test]
    fn playlist_images_stay_in_the_managed_cover_directory() {
        let track = Track {
            cover: "local-playlist:abc-123".into(),
            ..Default::default()
        };
        let expected = crate::config::data_dir()
            .join("playlist-art")
            .join("abc-123.png");
        assert_eq!(
            track.cover_url(320),
            Some(format!("local:{}", expected.display()))
        );
        for id in ["../outside", "C:/private", "a/b", "", "a?b"] {
            assert!(crate::config::playlist_art_path(id).is_err());
            assert!(
                Track {
                    cover: format!("local-playlist:{id}"),
                    ..Default::default()
                }
                .cover_url(80)
                .is_none()
            );
        }
    }
    #[test]
    fn progress_preview_matches_the_clamped_seek_position() {
        assert_eq!(seek_time(350., 100., 500., 200.), 100.);
        assert_eq!(seek_time(0., 100., 500., 200.), 0.);
        assert_eq!(seek_time(800., 100., 500., 200.), 200.);
        assert_eq!(seek_time(100., 100., 0., 200.), 0.);
    }
}

#[derive(Clone, Debug, Default)]
pub struct Artist {
    pub id: u64,
    pub name: String,
    pub picture: String,
    pub tracks: std::sync::Arc<Vec<Track>>,
    pub albums: Vec<Album>,
    pub singles: Vec<Album>,
    pub biography: String,
    pub notices: Vec<String>,
    pub fans: Option<u64>,
    pub followed: Option<bool>,
    pub album_shelf: Vec<usize>,
    pub single_shelf: Vec<usize>,
}

/// The preview shelves show each named release once; View all retains editions.
pub fn release_shelf(albums: &[Album]) -> Vec<usize> {
    let mut seen = std::collections::HashSet::new();
    albums
        .iter()
        .enumerate()
        .filter_map(|(i, a)| {
            seen.insert((a.title.trim().to_lowercase(), a.year.clone()))
                .then_some(i)
        })
        .collect()
}

#[cfg(test)]
mod artist_tests {
    use super::*;
    #[test]
    fn release_preview_groups_editions_while_full_discography_keeps_them() {
        let releases = vec![
            Album {
                id: 1,
                title: "Fuego".into(),
                year: "2026".into(),
                explicit: true,
                ..Default::default()
            },
            Album {
                id: 2,
                title: "fuego".into(),
                year: "2026".into(),
                ..Default::default()
            },
            Album {
                id: 3,
                title: "Fuego (Live)".into(),
                year: "2026".into(),
                ..Default::default()
            },
            Album {
                id: 4,
                title: "Fuego".into(),
                year: "2025".into(),
                ..Default::default()
            },
        ];
        assert_eq!(release_shelf(&releases), vec![0, 2, 3]);
        assert_eq!(releases.len(), 4);
        assert!(releases[0].explicit);
    }
    #[test]
    fn artist_identity_survives_library_cache_and_older_records_still_load() {
        let track = Track::from_v1(
            &serde_json::json!({"id":4,"title":"A","artist":{"id":12,"name":"Artist"}}),
        );
        assert_eq!(track.artist_id, 12);
        let cached = serde_json::to_value(&track).unwrap();
        assert_eq!(
            serde_json::from_value::<Track>(cached.clone())
                .unwrap()
                .artist_id,
            12
        );
        let mut old = cached;
        old.as_object_mut().unwrap().remove("artist_id");
        assert_eq!(serde_json::from_value::<Track>(old).unwrap().artist_id, 0);
        let multiple = Track::from_v1(
            &serde_json::json!({"artists":[{"id":13,"name":"First"},{"id":14,"name":"Second"}]}),
        );
        assert_eq!(multiple.artist_id, 13);
    }
}
