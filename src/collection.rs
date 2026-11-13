//! Derived collection data is rebuilt only when its immutable source changes.
//! Weak references prevent address reuse from making a new collection look cached.
use crate::model::{CatalogItem, Track};
use std::{
    borrow::Cow,
    cmp::Reverse,
    sync::{Arc, Weak},
};

const TEXT_BUDGET: usize = 2 * 1024 * 1024;
const ROW_LIMIT: usize = 4096;

#[derive(Default)]
pub struct TrackIndex {
    source: Weak<Vec<Track>>,
    folded: Vec<Box<str>>,
    indexed: bool,
}

impl TrackIndex {
    pub fn matches(&self, tracks: &Arc<Vec<Track>>) -> bool {
        self.source.as_ptr() == Arc::as_ptr(tracks)
    }
    pub fn invalidate(&mut self) {
        *self = Self::default();
    }
    fn prepare(&mut self, tracks: &Arc<Vec<Track>>, text: bool) {
        if !self.matches(tracks) {
            self.invalidate();
            self.source = Arc::downgrade(tracks);
        }
        if text && !self.indexed {
            let mut bytes = 0;
            for track in tracks.iter().take(ROW_LIMIT) {
                let row = normalized(track);
                if bytes + row.len() > TEXT_BUDGET {
                    break;
                }
                bytes += row.len();
                self.folded.push(row.into_boxed_str());
            }
            self.indexed = true;
        }
    }
    pub fn select(
        &mut self,
        tracks: &Arc<Vec<Track>>,
        query: &str,
        sort: Option<(usize, bool)>,
    ) -> Arc<Vec<usize>> {
        let query = query.trim().to_lowercase();
        self.prepare(
            tracks,
            !query.is_empty() || sort.is_some_and(|(c, _)| (1..=3).contains(&c)),
        );
        let mut indices: Vec<usize> = tracks
            .iter()
            .enumerate()
            .filter(|(i, track)| {
                query.is_empty()
                    || self
                        .folded
                        .get(*i)
                        .map(|s| matches_query(s, &query))
                        .unwrap_or_else(|| matches_query(&normalized(track), &query))
            })
            .map(|(i, _)| i)
            .collect();
        if let Some((column, descending)) = sort {
            if (1..=3).contains(&column) {
                // Cached keys normalize each uncached row once, never inside an
                // O(n log n) comparator. Equal keys keep the service's row order.
                let key = |&i: &usize| -> Cow<'_, str> {
                    if let Some(row) = self.folded.get(i) {
                        Cow::Borrowed(row.split('\0').nth(column - 1).unwrap_or_default())
                    } else {
                        Cow::Owned(text_field(&tracks[i], column).to_lowercase())
                    }
                };
                if descending {
                    indices.sort_by_cached_key(|i| Reverse(key(i)));
                } else {
                    indices.sort_by_cached_key(key);
                }
            } else {
                indices.sort_by(|&a, &b| {
                    let (a, b) = (&tracks[a], &tracks[b]);
                    let order = match column {
                        4 => a.date_added.cmp(&b.date_added),
                        5 => a.bitrate.cmp(&b.bitrate),
                        6 => a.sample_rate.cmp(&b.sample_rate),
                        7 => a.bit_depth.cmp(&b.bit_depth),
                        8 => a.duration.cmp(&b.duration),
                        _ => a.playlist_index.cmp(&b.playlist_index),
                    };
                    if descending { order.reverse() } else { order }
                });
            }
        }
        Arc::new(indices)
    }
}

fn text_field(track: &Track, column: usize) -> &str {
    match column {
        1 => &track.title,
        2 => &track.artist,
        _ => &track.album,
    }
}
fn normalized(track: &Track) -> String {
    let mut result =
        String::with_capacity(track.title.len() + track.artist.len() + track.album.len() + 2);
    for (i, field) in [&track.title, &track.artist, &track.album]
        .into_iter()
        .enumerate()
    {
        if i > 0 {
            result.push('\0');
        }
        result.extend(field.chars().flat_map(char::to_lowercase));
    }
    result
}
fn matches_query(row: &str, query: &str) -> bool {
    row.split('\0').any(|field| field.contains(query))
}

#[derive(Default)]
pub struct CatalogIndex {
    source: Weak<Vec<CatalogItem>>,
    tab: String,
    sort: usize,
    indices: Arc<Vec<usize>>,
    revision: u64,
}
impl CatalogIndex {
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn get(
        &mut self,
        items: &Arc<Vec<CatalogItem>>,
        tab: &str,
        sort: usize,
    ) -> Arc<Vec<usize>> {
        if self.source.as_ptr() != Arc::as_ptr(items) || self.tab != tab || self.sort != sort {
            let mut indices: Vec<usize> = items
                .iter()
                .enumerate()
                .filter(|(_, item)| match tab {
                    "Profiles" => item.kind == "Artist",
                    "Albums" => item.kind == "Album",
                    "Tracks" => item.kind == "Track",
                    "Playlists" => item.kind == "Playlist",
                    _ => true,
                })
                .map(|(i, _)| i)
                .collect();
            match sort {
                1 => indices.sort_by_cached_key(|&i| items[i].title.to_lowercase()),
                2 => indices.sort_by_cached_key(|&i| items[i].artist.to_lowercase()),
                3 => indices.sort_by(|&a, &b| items[b].year.cmp(&items[a].year)),
                _ => {}
            }
            self.source = Arc::downgrade(items);
            self.tab = tab.into();
            self.sort = sort;
            self.indices = Arc::new(indices);
            self.revision = self.revision.wrapping_add(1);
        }
        self.indices.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Arc<Vec<Track>> {
        Arc::new(vec![
            Track {
                title: "Été".into(),
                artist: "RAY".into(),
                album: "First".into(),
                album_id: 1,
                duration: 180,
                bitrate: Some(980_000),
                playlist_index: Some(0),
                ..Default::default()
            },
            Track {
                title: "addiction".into(),
                artist: "ray".into(),
                album: "First".into(),
                album_id: 1,
                duration: 150,
                playlist_index: Some(1),
                ..Default::default()
            },
            Track {
                title: "Addiction".into(),
                artist: "MINAMI".into(),
                album: "Second".into(),
                album_id: 2,
                duration: 210,
                playlist_index: Some(2),
                ..Default::default()
            },
            Track {
                title: "ਪਿਆਰ".into(),
                artist: "RAY".into(),
                album: "Third".into(),
                album_id: 3,
                duration: 200,
                playlist_index: Some(3),
                ..Default::default()
            },
        ])
    }
    #[test]
    fn catalog_frames_reuse_order_and_tab_sort_refreshes_invalidate_it() {
        let mut items = Arc::new(vec![
            CatalogItem {
                kind: "Track".into(),
                id: "1".into(),
                title: "Zulu".into(),
                artist: "B".into(),
                cover: String::new(),
                year: "2024".into(),
                explicit: false,
                track: None,
            },
            CatalogItem {
                kind: "Artist".into(),
                id: "2".into(),
                title: "Alpha".into(),
                artist: "A".into(),
                cover: String::new(),
                year: String::new(),
                explicit: false,
                track: None,
            },
            CatalogItem {
                kind: "Track".into(),
                id: "3".into(),
                title: "Alpha".into(),
                artist: "A".into(),
                cover: String::new(),
                year: "2026".into(),
                explicit: false,
                track: None,
            },
        ]);
        let mut cache = CatalogIndex::default();
        let first = cache.get(&items, "Tracks", 0);
        assert_eq!(*first, vec![0, 2]);
        let revision = cache.revision();
        assert!(Arc::ptr_eq(&first, &cache.get(&items, "Tracks", 0)));
        assert_eq!(revision, cache.revision());
        assert_eq!(*cache.get(&items, "Tracks", 1), vec![2, 0]);
        assert_eq!(*cache.get(&items, "Profiles", 0), vec![1]);
        assert_eq!(*cache.get(&items, "Top results", 3), vec![2, 0, 1]);
        Arc::make_mut(&mut items)[2].kind = "Album".into();
        assert_eq!(*cache.get(&items, "Tracks", 0), vec![0]);
        assert_eq!(*cache.get(&items, "Albums", 0), vec![2]);
    }
    #[test]
    fn filters_all_fields_without_crossing_field_boundaries_and_keeps_source_indices() {
        let tracks = sample();
        let mut index = TrackIndex::default();
        for (query, expected) in [
            ("  ÉTÉ ", vec![0]),
            ("ray", vec![0, 1, 3]),
            ("second", vec![2]),
            ("ਪਿਆਰ", vec![3]),
            ("té\0ray", vec![]),
            (" ", vec![0, 1, 2, 3]),
        ] {
            assert_eq!(*index.select(&tracks, query, None), expected, "{query}");
        }
    }
    #[test]
    fn sort_is_stable_in_both_directions_and_retains_missing_metadata_order() {
        let tracks = sample();
        let mut index = TrackIndex::default();
        assert_eq!(
            *index.select(&tracks, "", Some((1, false))),
            vec![1, 2, 0, 3]
        );
        assert_eq!(
            *index.select(&tracks, "", Some((1, true))),
            vec![3, 0, 1, 2]
        );
        assert_eq!(
            *index.select(&tracks, "ray", Some((8, false))),
            vec![1, 0, 3]
        );
        assert_eq!(
            *index.select(&tracks, "", Some((5, false))),
            vec![1, 2, 3, 0]
        );
        assert_eq!(
            *index.select(&tracks, "", Some((0, true))),
            vec![3, 2, 1, 0]
        );
    }
    #[test]
    fn source_refresh_and_copy_on_write_never_reuse_stale_text_indices() {
        let mut tracks = sample();
        let mut filter = TrackIndex::default();
        assert_eq!(*filter.select(&tracks, "ÉTÉ", None), vec![0]);
        Arc::make_mut(&mut tracks)[0].title = "Changed".into();
        Arc::make_mut(&mut tracks)[0].album_id = 4;
        assert!(!filter.matches(&tracks));
        assert!(filter.select(&tracks, "ÉTÉ", None).is_empty());
        assert_eq!(*filter.select(&tracks, "changed", None), vec![0]);
        drop(tracks);
        assert!(filter.source.upgrade().is_none());
    }
    #[test]
    fn text_cache_is_bounded_and_uncached_tail_remains_searchable_and_sortable() {
        let tracks = Arc::new(
            (0..ROW_LIMIT + 40)
                .map(|i| Track {
                    title: format!("Song {i:05}"),
                    artist: "a".repeat(1500),
                    ..Default::default()
                })
                .collect(),
        );
        let mut index = TrackIndex::default();
        let result = index.select(&tracks, "song 04135", Some((1, true)));
        assert_eq!(*result, vec![4135]);
        assert!(index.folded.iter().map(|s| s.len()).sum::<usize>() <= TEXT_BUDGET);
        assert!(index.folded.len() < ROW_LIMIT);
        let cached = index.folded[0].as_ptr();
        index.select(&tracks, "song", Some((1, false)));
        assert_eq!(cached, index.folded[0].as_ptr());
    }
}
