//! Bounded metadata retention. Eviction never removes the ability to reopen an album.
use crate::model::Album;
use std::collections::{HashMap, VecDeque};

#[derive(Default)]
pub struct AlbumCache {
    entries: HashMap<u64, Album>,
    recent: VecDeque<u64>,
}
impl AlbumCache {
    const LIMIT: usize = 32;
    pub fn get(&mut self, id: &u64) -> Option<&Album> {
        if self.entries.contains_key(id) {
            self.recent.retain(|key| key != id);
            self.recent.push_back(*id);
        }
        self.entries.get(id)
    }
    pub fn insert(&mut self, id: u64, album: Album) {
        self.recent.retain(|key| *key != id);
        self.recent.push_back(id);
        self.entries.insert(id, album);
        while self.entries.len() > Self::LIMIT {
            if let Some(old) = self.recent.pop_front() {
                self.entries.remove(&old);
            }
        }
    }
    pub fn clear(&mut self) {
        self.entries.clear();
        self.recent.clear();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn long_browsing_sessions_remain_bounded_and_keep_recently_viewed_metadata() {
        let mut c = AlbumCache::default();
        c.insert(
            1,
            Album {
                id: 1,
                ..Default::default()
            },
        );
        for id in 2..5000 {
            assert!(c.get(&1).is_some());
            c.insert(
                id,
                Album {
                    id,
                    ..Default::default()
                },
            );
            assert!(c.entries.len() <= AlbumCache::LIMIT);
        }
        assert!(c.get(&2).is_none());
        c.insert(
            2,
            Album {
                id: 2,
                title: "Reloaded".into(),
                ..Default::default()
            },
        );
        assert_eq!(c.get(&2).unwrap().title, "Reloaded");
        c.clear();
        assert!(c.get(&1).is_none());
    }
}
