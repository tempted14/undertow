//! Small, account-scoped view models for TIDAL's discovery feed.
use crate::model::{CatalogItem, Playlist, Track};
use serde_json::Value;
use std::{collections::HashSet, sync::Arc};

#[derive(Clone, Debug)]
pub struct Item {
    pub catalog: CatalogItem,
    pub list: Option<Playlist>,
}
impl Item {
    pub fn key(&self) -> String {
        match self.catalog.kind.as_str() {
            "Playlist" => self.catalog.id.clone(),
            "My Tracks" => "favorites".into(),
            kind => format!("{}:{}", kind.to_lowercase(), self.catalog.id),
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct Section {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub more: Option<String>,
    pub rows: bool,
    pub shortcuts: bool,
    pub items: Arc<Vec<Item>>,
}
#[derive(Clone, Debug, Default)]
pub struct Feed {
    pub personalized: bool,
    pub sections: Vec<Section>,
}

/// Only paths returned by the home/page APIs are accepted. A response can never
/// redirect authenticated requests to another host or another account resource.
pub fn page_route(path: &str) -> Option<String> {
    let path = path.strip_prefix('/').unwrap_or(path);
    if path.len() > 1024 || path.contains([':', '\\', '#', '%']) {
        return None;
    }
    let route = path.split('?').next()?;
    if route
        .split('/')
        .any(|p| p == "." || p == ".." || p.is_empty())
    {
        return None;
    }
    if !(route.starts_with("pages/data/")
        || route.starts_with("pages/single-module-page/")
        || (route.starts_with("home/pages/") && route.ends_with("/view-all")))
    {
        return None;
    }
    if !path
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-_/?=&.".contains(c))
    {
        return None;
    }
    Some(if route.starts_with("home/") {
        format!("../v2/{path}")
    } else {
        path.to_owned()
    })
}
fn text(v: &Value, key: &str) -> String {
    v.pointer(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .chars()
        .take(2048)
        .collect()
}
fn first_text(v: &Value, keys: &[&str]) -> String {
    keys.iter()
        .map(|key| text(v, key))
        .find(|s| !s.is_empty())
        .unwrap_or_default()
}
pub fn item(v: &Value, hint: &str) -> Option<Item> {
    let kind = v["type"].as_str().unwrap_or(hint);
    let d = v.get("data").or_else(|| v.get("item")).unwrap_or(v);
    if d["upload"] == true || matches!(kind, "VIDEO" | "UPLOAD" | "SPOTLIGHT_UPLOAD") {
        return None;
    }
    let kind = if d.get("album").is_some() && hint == "TRACK" {
        "TRACK"
    } else {
        kind
    };
    let mut id = d["id"]
        .as_u64()
        .map(|n| n.to_string())
        .unwrap_or_else(|| first_text(d, &["/uuid", "/id"]));
    let (kind, title, cover) = match kind {
        "TRACK" => ("Track", text(d, "/title"), text(d, "/album/cover")),
        "ALBUM" | "EP" | "SINGLE" if hint != "PLAYLIST" => {
            ("Album", text(d, "/title"), text(d, "/cover"))
        }
        "ARTIST" => (
            "Artist",
            text(d, "/name"),
            first_text(d, &["/picture", "/selectedAlbumCoverFallback"]),
        ),
        "MIX" => (
            "Mix",
            first_text(d, &["/titleTextInfo/text", "/title"]),
            first_text(
                d,
                &["/mixImages/1/url", "/mixImages/0/url", "/images/MEDIUM/url"],
            ),
        ),
        "DEEP_LINK" if d["url"] == "tidal://my-collection/tracks" => {
            id = "favorites".into();
            ("My Tracks", "My Tracks".into(), String::new())
        }
        _ if hint == "PLAYLIST" || kind == "PLAYLIST" => (
            "Playlist",
            text(d, "/title"),
            first_text(d, &["/squareImage", "/image"]),
        ),
        _ => return None,
    };
    if id.is_empty()
        || title.is_empty()
        || id.len() > 128
        || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return None;
    }
    if matches!(kind, "Album" | "Artist" | "Track")
        && id.parse::<u64>().ok().is_none_or(|id| id == 0)
    {
        return None;
    }
    let track = (kind == "Track").then(|| Track::from_v1(d));
    let artist = if kind == "Mix" {
        first_text(
            d,
            &[
                "/subtitleTextInfo/text",
                "/subTitle",
                "/shortSubtitleTextInfo/text",
            ],
        )
    } else if kind == "Playlist" {
        let name = first_text(d, &["/creator/name", "/creators/0/name"]);
        if name.is_empty() {
            "TIDAL".into()
        } else {
            name
        }
    } else {
        track.as_ref().map(|t| t.artist.clone()).unwrap_or_else(|| {
            let names: Vec<_> = d["artists"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v["name"].as_str())
                .collect();
            if names.is_empty() {
                text(d, "/artist/name")
            } else {
                names.join(", ")
            }
        })
    };
    let list = matches!(kind, "Playlist" | "Mix").then(|| Playlist {
        id: if kind == "Mix" {
            format!("mix:{id}")
        } else {
            id.clone()
        },
        title: title.clone(),
        description: first_text(d, &["/description", "/subtitleTextInfo/text", "/subTitle"]),
        image: cover.clone(),
        creator: artist.clone(),
        count: d["numberOfTracks"].as_u64().unwrap_or(0),
        duration: d["duration"].as_u64().unwrap_or(0),
        owned: false,
        public: None,
    });
    Some(Item {
        catalog: CatalogItem {
            kind: kind.into(),
            id,
            title,
            artist,
            cover,
            year: text(d, "/releaseDate").chars().take(4).collect(),
            explicit: d["explicit"].as_bool().unwrap_or(false),
            track,
        },
        list,
    })
}

pub fn section(v: &Value, fallback_id: &str) -> Section {
    let kind = v["type"].as_str().unwrap_or("");
    let hint = kind.strip_suffix("_LIST").unwrap_or("");
    let items = v.pointer("/pagedList/items").or_else(|| v.get("items"));
    let mut seen = HashSet::new();
    let items: Vec<_> = items
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| item(v, hint))
        .filter(|i| seen.insert(i.key()))
        .take(256)
        .collect();
    let more = first_text(
        v,
        &["/viewAll", "/showMore/apiPath", "/pagedList/dataApiPath"],
    );
    Section {
        id: first_text(v, &["/moduleId", "/id"])
            .chars()
            .take(128)
            .collect::<String>(),
        title: text(v, "/title"),
        subtitle: text(v, "/subtitle"),
        rows: kind == "TRACK_LIST"
            || v["moduleId"] == "CONTINUE_LISTEN_TO"
            || v["itemLayout"] == "LIST",
        shortcuts: kind == "SHORTCUT_LIST",
        more: page_route(&more).map(|_| more),
        items: Arc::new(items),
    }
    .with_id(fallback_id)
}
impl Section {
    fn with_id(mut self, fallback: &str) -> Self {
        if self.id.is_empty() {
            self.id = fallback.into();
        }
        self
    }
}
pub fn feed(v: &Value) -> Feed {
    let personalized = v.get("rows").is_none();
    let mut sections = Vec::new();
    let mut remaining = 768;
    let mut append = |v: &Value| {
        if sections.len() >= 24 || remaining == 0 {
            return;
        }
        let mut section = section(v, &format!("section-{}", sections.len()));
        if section.title.to_lowercase().contains("upload") {
            return;
        }
        if section.items.len() > remaining {
            Arc::make_mut(&mut section.items).truncate(remaining);
        }
        remaining -= section.items.len();
        if !section.items.is_empty() {
            sections.push(section);
        }
    };
    if let Some(rows) = v["rows"].as_array() {
        for row in rows {
            for module in row["modules"].as_array().into_iter().flatten() {
                append(module);
            }
        }
    } else {
        for module in v["items"].as_array().into_iter().flatten() {
            append(module);
        }
    }
    Feed {
        personalized,
        sections,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn personalized_sections_keep_real_targets_and_ignore_unsupported_items() {
        let f = feed(&json!({"items":[
            {"moduleId":"ALBUM_RECOMMENDATIONS","title":"Albums you’ll enjoy","type":"HORIZONTAL_LIST","viewAll":"home/pages/ALBUM_RECOMMENDATIONS/view-all","items":[
                {"type":"ALBUM","data":{"id":9,"title":"Suggested album","cover":"art"}},
                {"type":"TRACK","data":{"id":4,"title":"Track","upload":true}},
                {"type":"VIDEO","data":{"id":5,"title":"Video"}}]},
            {"moduleId":"NEW_TRACK_SUGGESTIONS","title":"New tracks","type":"TRACK_LIST","items":[{"type":"TRACK","data":{"id":7,"title":"New song","album":{"id":9,"title":"Suggested album"}}}]}]}));
        assert!(f.personalized);
        assert_eq!(f.sections.len(), 2);
        assert_eq!(f.sections[0].items.len(), 1);
        assert_eq!(f.sections[0].items[0].key(), "album:9");
        assert!(f.sections[1].rows);
        assert_eq!(
            f.sections[1].items[0]
                .catalog
                .track
                .as_ref()
                .unwrap()
                .album_id,
            9
        );
    }
    #[test]
    fn legacy_editorial_and_mix_shapes_are_supported_without_favorites_substitution() {
        let f = feed(
            &json!({"rows":[{"modules":[{"id":"p","title":"Editors","type":"PLAYLIST_LIST","pagedList":{"items":[{"uuid":"abc-def","title":"Real playlist","type":"EDITORIAL","squareImage":"art"}]}}]}]}),
        );
        assert!(!f.personalized);
        assert_eq!(f.sections[0].items[0].key(), "abc-def");
        let mix=item(&json!({"type":"MIX","data":{"id":"abc123","titleTextInfo":{"text":"My Mix 1"},"mixImages":[{"url":"https://images.tidal.com/s.jpg"}]}}),"").unwrap();
        assert_eq!(mix.key(), "mix:abc123");
        assert_eq!(mix.list.unwrap().id, "mix:abc123");
    }
    #[test]
    fn authenticated_page_paths_cannot_escape_discovery_routes() {
        for path in [
            "https://evil.test/pages/data/a",
            "//evil.test/pages/data/a",
            "pages/../sessions",
            "home/pages/a/../../sessions",
            "pages/data/%2e%2e/a",
            "users/1/favorites",
            "pages/data/a#fragment",
        ] {
            assert!(page_route(path).is_none(), "{path}");
        }
        assert_eq!(
            page_route("home/pages/ALBUM_RECOMMENDATIONS/view-all").unwrap(),
            "../v2/home/pages/ALBUM_RECOMMENDATIONS/view-all"
        );
        assert!(page_route("pages/data/id?mixId=abc").is_some());
    }
}
