mod discovery;
use crate::{
    auth::{self, Session},
    config::Settings,
    model::{Album, Artist, CatalogItem, CreditGroup, Playlist, Track},
};
use anyhow::{Context, Result, bail, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use reqwest::{
    Method,
    blocking::{Client, Response},
    header::ETAG,
};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    io::Read,
};

pub struct Tidal {
    pub session: Session,
    pub settings: Settings,
    client: Client,
    etags: HashMap<String, String>,
    cancelled: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    artist_hints: std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<ArtistHint>>>,
    #[cfg(test)]
    pub(crate) test_base: Option<String>,
}

#[derive(Clone)]
struct ArtistHint {
    id: u64,
    picture: String,
    fans: Option<u64>,
}
fn artist_picture(raw: &str) -> String {
    if raw.is_empty() {
        String::new()
    } else if raw.starts_with("https://") {
        raw.to_owned()
    } else {
        format!(
            "https://resources.tidal.com/images/{}/750x750.jpg",
            raw.replace('-', "/")
        )
    }
}

#[derive(Clone)]
pub struct StreamPlan {
    pub urls: Vec<String>,
    pub encryption: Option<([u8; 16], [u8; 8])>,
    pub quality: String,
    pub codec: String,
    pub bit_depth: u32,
    pub sample_rate: u32,
}

fn read_json(r: Response) -> Result<Value> {
    let status = r.status();
    if status.as_u16() == 401 {
        bail!("TIDAL session expired or revoked. Sign in again.");
    }
    if status.as_u16() == 403 {
        bail!(
            "TIDAL denied access (403). The app token may lack the required scope, playback entitlement or subscription."
        );
    }
    if status.as_u16() == 412 || status.as_u16() == 409 {
        bail!("This playlist changed elsewhere. Sync it before editing again.");
    }
    ensure!(
        status.is_success(),
        "TIDAL request returned HTTP {}",
        status.as_u16()
    );
    let mut bytes = Vec::new();
    r.take(8 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 8 * 1024 * 1024,
        "TIDAL response exceeded 8 MiB"
    );
    if bytes.is_empty() {
        Ok(Value::Null)
    } else {
        Ok(serde_json::from_slice(&bytes)?)
    }
}

impl Tidal {
    pub(crate) fn discovery_document(&mut self, path: &str) -> Result<Value> {
        self.get(
            path,
            &[
                ("deviceType", "BROWSER".into()),
                ("locale", "en_US".into()),
                ("platform", "WEB".into()),
                ("limit", "50".into()),
            ],
        )
    }
    pub fn new(session: Session, settings: Settings) -> Result<Self> {
        Ok(Self {
            session,
            settings,
            client: auth::http()?,
            etags: HashMap::new(),
            cancelled: None,
            artist_hints: Default::default(),
            #[cfg(test)]
            test_base: None,
        })
    }
    /// Read lanes share the connection pool, but never playlist edit revisions.
    pub(crate) fn fork_read(
        &self,
        cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        Self {
            session: self.session.clone(),
            settings: self.settings.clone(),
            client: self.client.clone(),
            etags: HashMap::new(),
            cancelled: Some(cancelled),
            artist_hints: self.artist_hints.clone(),
            #[cfg(test)]
            test_base: self.test_base.clone(),
        }
    }
    fn request(
        &mut self,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<Value>,
        form: Option<Vec<(&str, String)>>,
        etag: Option<String>,
    ) -> Result<Response> {
        auth::refresh_cancellable(&mut self.session, &self.settings, self.cancelled.as_deref())?;
        let base = if self.session.compatibility {
            "https://api.tidal.com/v1/"
        } else {
            "https://openapi.tidal.com/v2/"
        };
        #[cfg(test)]
        let base = self.test_base.as_deref().unwrap_or(base);
        let mut req = self
            .client
            .request(method, format!("{base}{path}"))
            .bearer_auth(&self.session.access_token)
            .query(query);
        if self.session.compatibility {
            req = req
                .query(&[("countryCode", &self.session.country)])
                .header("x-tidal-client-version", "2025.7.16");
        } else {
            req = req
                .header("Accept", "application/vnd.api+json")
                .header("Content-Type", "application/vnd.api+json");
        }
        if let Some(b) = body {
            req = req.json(&b);
        }
        if let Some(f) = form {
            req = req.form(&f);
        }
        if let Some(e) = etag {
            req = req.header("If-None-Match", e);
        }
        Ok(req.send()?)
    }
    fn get(&mut self, path: &str, query: &[(&str, String)]) -> Result<Value> {
        let r = self.request(Method::GET, path, query, None, None, None)?;
        read_json(r)
    }

    pub fn identify(&mut self) -> Result<String> {
        if self.session.compatibility {
            let v = self.get("sessions", &[])?;
            self.session.user_id = v["userId"]
                .as_u64()
                .context("TIDAL session omitted user id")?;
            if let Some(c) = v["countryCode"].as_str() {
                self.session.country = c.into();
            }
            auth::save_session(&self.session)?;
            Ok(format!(
                "Connected · {} · {}",
                self.session.country,
                if self.session.client_id == auth::WEB_PKCE_CLIENT_ID {
                    "lossless web sign-in"
                } else if self.session.client_id == auth::DEVICE_CLIENT_ID {
                    "website code"
                } else {
                    "desktop compatibility"
                }
            ))
        } else {
            let v = self.get("users/me", &[])?;
            self.session.user_id = v["data"]["id"].as_str().unwrap_or("0").parse().unwrap_or(0);
            if let Some(c) = v
                .pointer("/data/attributes/country")
                .and_then(Value::as_str)
            {
                self.session.country = c.into();
            }
            auth::save_session(&self.session)?;
            Ok(format!(
                "Connected · {} · registered app",
                self.session.country
            ))
        }
    }
    fn v1_pages(&mut self, path: &str, query: &[(&str, String)]) -> Result<Vec<Value>> {
        let mut result = Vec::new();
        for page in 0..1000 {
            let mut q = query.to_vec();
            q.push(("limit", "100".into()));
            q.push(("offset", (page * 100).to_string()));
            let v = self.get(path, &q)?;
            let items = v["items"]
                .as_array()
                .context("TIDAL omitted paginated items")?;
            let n = items.len();
            result.extend(items.iter().cloned());
            if n == 0
                || result.len() as u64 >= v["totalNumberOfItems"].as_u64().unwrap_or(u64::MAX)
                || n < 100
            {
                return Ok(result);
            }
        }
        bail!(
            "Library exceeds the 100,000 item metadata safety limit; no incomplete sync was saved"
        )
    }
    fn v2_pages(&mut self, path: &str, query: &[(&str, String)]) -> Result<Vec<Value>> {
        let mut pages = Vec::new();
        let mut cursor = String::new();
        let mut seen = HashSet::new();
        for _ in 0..1000 {
            let mut q = query.to_vec();
            if !cursor.is_empty() {
                q.push(("page[cursor]", cursor.clone()));
            }
            let v = self.get(path, &q)?;
            let next = v
                .pointer("/links/next")
                .and_then(|v| v.as_str().or_else(|| v["href"].as_str()))
                .unwrap_or("");
            cursor = if next.is_empty() {
                String::new()
            } else {
                let u = url::Url::parse(next)
                    .or_else(|_| url::Url::parse(&format!("https://openapi.tidal.com{next}")))?;
                ensure!(
                    u.host_str() == Some("openapi.tidal.com"),
                    "Untrusted pagination host"
                );
                u.query_pairs()
                    .find(|(k, _)| k == "page[cursor]")
                    .map(|(_, v)| v.into_owned())
                    .context("Unrecognized pagination cursor")?
            };
            pages.push(v);
            if cursor.is_empty() {
                return Ok(pages);
            }
            ensure!(
                seen.insert(cursor.clone()),
                "TIDAL repeated a pagination cursor"
            );
        }
        bail!("Library exceeds pagination safety limit")
    }
    pub fn playlists(&mut self) -> Result<Vec<Playlist>> {
        if self.session.compatibility {
            let uid = self.session.user_id;
            let mut values = self.v1_pages(&format!("users/{uid}/playlists"), &[])?;
            // Followed playlists are a separate collection in the legacy API.
            if let Ok(followed) = self.v1_pages(&format!("users/{uid}/favorites/playlists"), &[]) {
                values.extend(followed.into_iter().map(|v| v["item"].clone()));
            }
            let mut seen = HashSet::new();
            Ok(values
                .into_iter()
                .filter_map(|v| {
                    let id = v["uuid"].as_str()?.to_owned();
                    if !seen.insert(id.clone()) {
                        return None;
                    }
                    Some(Playlist {
                        id,
                        title: v["title"].as_str().unwrap_or("").into(),
                        description: v["description"].as_str().unwrap_or("").into(),
                        count: v["numberOfTracks"].as_u64().unwrap_or(0),
                        duration: v["duration"].as_u64().unwrap_or(0),
                        image: v["squareImage"]
                            .as_str()
                            .or(v["image"].as_str())
                            .unwrap_or("")
                            .into(),
                        owned: v.pointer("/creator/id").and_then(Value::as_u64) == Some(uid),
                        creator: v
                            .pointer("/creator/name")
                            .or_else(|| v.pointer("/creator/userName"))
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .into(),
                        public: v["publicPlaylist"].as_bool(),
                    })
                })
                .collect())
        } else {
            let pages = self.v2_pages(
                "userCollectionPlaylists/me/relationships/items",
                &[
                    ("include", "items.owners,items.coverArt".into()),
                    ("collectionView", "FLAT".into()),
                ],
            )?;
            let uid = self.session.user_id.to_string();
            let mut out = Vec::new();
            let mut seen = HashSet::new();
            for page in pages {
                for p in included(&page).filter(|v| v["type"] == "playlists") {
                    let id = p["id"].as_str().unwrap_or("").to_string();
                    if seen.insert(id.clone()) {
                        out.push(Playlist {
                            id,
                            title: p["attributes"]["name"].as_str().unwrap_or("").into(),
                            description: p["attributes"]["description"]
                                .as_str()
                                .unwrap_or("")
                                .into(),
                            count: p["attributes"]["numberOfItems"].as_u64().unwrap_or(0),
                            duration: iso_duration(
                                p["attributes"]["duration"].as_str().unwrap_or(""),
                            ),
                            image: String::new(),
                            owned: p
                                .pointer("/relationships/owners/data")
                                .and_then(Value::as_array)
                                .is_some_and(|a| a.iter().any(|o| o["id"].as_str() == Some(&uid))),
                            creator: String::new(),
                            public: p
                                .pointer("/attributes/accessType")
                                .and_then(Value::as_str)
                                .map(|s| s == "PUBLIC"),
                        });
                    }
                }
            }
            Ok(out)
        }
    }
    pub fn playlist_tracks(&mut self, id: &str) -> Result<Vec<Track>> {
        valid_id(id)?;
        if self.session.compatibility {
            let r = self.request(
                Method::GET,
                &format!("playlists/{id}"),
                &[],
                None,
                None,
                None,
            )?;
            if let Some(e) = r.headers().get(ETAG).and_then(|v| v.to_str().ok()) {
                self.etags.insert(id.into(), e.into());
            }
            read_json(r)?;
            Ok(self
                .v1_pages(&format!("playlists/{id}/items"), &[])?
                .into_iter()
                .enumerate()
                .filter_map(|(index, v)| {
                    if v["type"]
                        .as_str()
                        .is_some_and(|t| !t.eq_ignore_ascii_case("track"))
                    {
                        return None;
                    }
                    let mut track = Track::from_v1(v.get("item").unwrap_or(&v));
                    track.playlist_index = Some(index);
                    track.date_added = v["created"]
                        .as_str()
                        .or_else(|| v["dateAdded"].as_str())
                        .unwrap_or(&track.date_added)
                        .into();
                    Some(track)
                })
                .collect())
        } else {
            let pages = self.v2_pages(
                &format!("playlists/{id}/relationships/items"),
                &[
                    ("include", "items.artists,items.albums.coverArt".into()),
                    ("countryCode", self.session.country.clone()),
                ],
            )?;
            let mut out = Vec::new();
            for p in pages {
                if let Some(rows) = p["data"].as_array() {
                    for row in rows {
                        if row["type"] != "tracks" {
                            continue;
                        }
                        if let Some(v) =
                            included(&p).find(|v| v["type"] == "tracks" && v["id"] == row["id"])
                        {
                            let mut t = track_v2(v, &p);
                            t.playlist_item_id =
                                row["meta"]["itemId"].as_str().unwrap_or("").into();
                            out.push(t);
                        }
                    }
                }
            }
            Ok(out)
        }
    }
    pub fn album_tracks(&mut self, id: u64) -> Result<Vec<Track>> {
        ensure!(id > 0, "Missing album identifier");
        if self.session.compatibility {
            Ok(self
                .v1_pages(&format!("albums/{id}/tracks"), &[])?
                .iter()
                .map(Track::from_v1)
                .collect())
        } else {
            let pages = self.v2_pages(
                &format!("albums/{id}/relationships/items"),
                &[
                    ("include", "items.artists,items.albums.coverArt".into()),
                    ("countryCode", self.session.country.clone()),
                ],
            )?;
            let mut out = Vec::new();
            for page in pages {
                if let Some(items) = page["data"].as_array() {
                    for item in items {
                        if let Some(track) =
                            included(&page).find(|v| v["type"] == "tracks" && v["id"] == item["id"])
                        {
                            out.push(track_v2(track, &page));
                        }
                    }
                }
            }
            Ok(out)
        }
    }
    pub fn favorites(&mut self) -> Result<Vec<Track>> {
        if self.session.compatibility {
            let uid = self.session.user_id;
            Ok(self
                .v1_pages(&format!("users/{uid}/favorites/tracks"), &[])?
                .into_iter()
                .map(|v| {
                    let mut track = Track::from_v1(&v["item"]);
                    track.date_added = v["created"]
                        .as_str()
                        .or_else(|| v["dateAdded"].as_str())
                        .unwrap_or("")
                        .into();
                    track
                })
                .collect())
        } else {
            let pages = self.v2_pages(
                "userCollectionTracks/me/relationships/items",
                &[
                    ("include", "items.artists,items.albums.coverArt".into()),
                    ("countryCode", self.session.country.clone()),
                ],
            )?;
            Ok(pages
                .iter()
                .flat_map(|p| {
                    included(p)
                        .filter(|v| v["type"] == "tracks")
                        .map(|v| track_v2(v, p))
                })
                .collect())
        }
    }
    pub fn credits(&mut self, track: &Track) -> Result<Vec<CreditGroup>> {
        ensure!(
            self.session.compatibility,
            "Credits require the TIDAL web connection"
        );
        let credits = self.get(&format!("tracks/{}/credits", track.id), &[])?;
        let mut groups = parse_credits(&credits);
        if track.album_id > 0
            && let Ok(album) = self.get(&format!("albums/{}", track.album_id), &[])
        {
            if let Some(date) = album["releaseDate"].as_str().filter(|d| !d.is_empty()) {
                groups.push(CreditGroup {
                    role: "Released".into(),
                    names: date.chars().take(4).collect(),
                });
            }
            if let Some(label) = album
                .pointer("/recordLabel/name")
                .and_then(Value::as_str)
                .or_else(|| album["copyright"].as_str())
                .filter(|s| !s.is_empty())
            {
                groups.push(CreditGroup {
                    role: "Label".into(),
                    names: label.into(),
                });
            }
        }
        Ok(groups)
    }
    pub fn similar_tracks(&mut self, id: u64) -> Result<Vec<Track>> {
        ensure!(
            self.session.compatibility,
            "Similar tracks require the TIDAL web connection"
        );
        let items = self.v1_pages(&format!("tracks/{id}/radio"), &[])?;
        Ok(items
            .iter()
            .map(|v| Track::from_v1(v.get("item").unwrap_or(v)))
            .filter(|t| t.id > 0 && t.id != id)
            .take(50)
            .collect())
    }
    pub fn search(&mut self, q: &str) -> Result<Vec<Track>> {
        if self.session.compatibility {
            let v = self.get(
                "search/tracks",
                &[("query", q.into()), ("limit", "60".into())],
            )?;
            Ok(v["items"]
                .as_array()
                .context("Search returned no items")?
                .iter()
                .map(Track::from_v1)
                .collect())
        } else {
            let encoded: String = url::form_urlencoded::byte_serialize(q.as_bytes()).collect();
            let v = self.get(
                &format!("searchResults/{encoded}/relationships/tracks"),
                &[
                    ("include", "tracks.artists,tracks.albums.coverArt".into()),
                    ("countryCode", self.session.country.clone()),
                ],
            )?;
            Ok(included(&v)
                .filter(|v| v["type"] == "tracks")
                .map(|t| track_v2(t, &v))
                .collect())
        }
    }

    pub fn catalog_search(&mut self, q: &str) -> Result<Vec<CatalogItem>> {
        if !self.session.compatibility {
            return Ok(self
                .search(q)?
                .into_iter()
                .map(|t| CatalogItem {
                    kind: "Track".into(),
                    id: t.id.to_string(),
                    title: t.title.clone(),
                    artist: t.artist.clone(),
                    cover: t.cover.clone(),
                    year: String::new(),
                    explicit: t.explicit,
                    track: Some(t),
                })
                .collect());
        }
        let v = self.get(
            "search",
            &[
                ("query", q.into()),
                ("types", "ARTISTS,ALBUMS,TRACKS,PLAYLISTS".into()),
                ("limit", "40".into()),
            ],
        )?;
        let mut out = Vec::new();
        for (key, kind) in [
            ("artists", "Artist"),
            ("albums", "Album"),
            ("tracks", "Track"),
            ("playlists", "Playlist"),
        ] {
            for item in v[key]["items"].as_array().into_iter().flatten() {
                let text = |k: &str| item[k].as_str().unwrap_or("").to_owned();
                let track = (kind == "Track").then(|| Track::from_v1(item));
                if kind == "Artist" {
                    self.remember_artist(item);
                }
                out.push(CatalogItem {
                    kind: kind.into(),
                    id: item["id"]
                        .as_u64()
                        .map(|i| i.to_string())
                        .unwrap_or_else(|| text("uuid")),
                    title: if kind == "Artist" {
                        text("name")
                    } else {
                        text("title")
                    },
                    artist: track.as_ref().map(|t| t.artist.clone()).unwrap_or_else(|| {
                        item.pointer("/artist/name")
                            .or_else(|| item.pointer("/artists/0/name"))
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .into()
                    }),
                    cover: track.as_ref().map(|t| t.cover.clone()).unwrap_or_else(|| {
                        text(if kind == "Artist" {
                            "picture"
                        } else if kind == "Playlist" {
                            "squareImage"
                        } else {
                            "cover"
                        })
                    }),
                    year: text("releaseDate").chars().take(4).collect(),
                    explicit: item["explicit"].as_bool().unwrap_or(false),
                    track,
                });
            }
        }
        // Interleave the first-ranked matches from each category for Top results.
        let mut ranks = std::collections::HashMap::<String, usize>::new();
        let mut ranked = out
            .into_iter()
            .map(|item| {
                let rank = ranks.entry(item.kind.clone()).or_default();
                let key = (
                    *rank,
                    match item.kind.as_str() {
                        "Artist" => 0,
                        "Album" => 1,
                        "Track" => 2,
                        _ => 3,
                    },
                );
                *rank += 1;
                (key, item)
            })
            .collect::<Vec<_>>();
        ranked.sort_by_key(|(key, _)| *key);
        let out = ranked.into_iter().map(|(_, item)| item).collect();
        Ok(out)
    }

    pub fn artist(&mut self, id: u64) -> Result<Artist> {
        self.artist_with_preview(id, |_| {})
    }
    fn remember_artist(&self, item: &Value) {
        let id = item["id"].as_u64().unwrap_or(0);
        let raw = item["picture"].as_str().unwrap_or("");
        if id == 0 || raw.is_empty() || raw.len() > 2048 {
            return;
        }
        if let Ok(mut hints) = self.artist_hints.lock() {
            hints.retain(|h| h.id != id);
            if hints.len() >= 64 {
                hints.pop_front();
            }
            hints.push_back(ArtistHint {
                id,
                picture: artist_picture(raw),
                fans: item["numberOfFans"].as_u64(),
            });
        }
    }
    pub(crate) fn artist_header(&mut self, id: u64) -> Result<Artist> {
        ensure!(id > 0, "Missing artist identifier");
        ensure!(
            self.session.compatibility,
            "Artist pages require the TIDAL web connection"
        );
        let info = self.get(&format!("artists/{id}"), &[])?;
        let mut artist = Artist {
            id,
            name: info["name"].as_str().unwrap_or("").into(),
            picture: artist_picture(info["picture"].as_str().unwrap_or("")),
            fans: info["numberOfFans"].as_u64(),
            ..Default::default()
        };
        if artist.picture.is_empty() {
            let hint = self
                .artist_hints
                .lock()
                .ok()
                .and_then(|h| h.iter().find(|h| h.id == id).cloned());
            let hint = hint.or_else(|| {
                if artist.name.is_empty() {
                    return None;
                }
                // Some v1 detail responses omit portraits. Search still provides
                // them; match IDs rather than choosing a same-named performer.
                let search = self
                    .get(
                        "search",
                        &[
                            ("query", artist.name.clone()),
                            ("types", "ARTISTS".into()),
                            ("limit", "40".into()),
                        ],
                    )
                    .ok()?;
                let matched = search
                    .pointer("/artists/items")?
                    .as_array()?
                    .iter()
                    .find(|a| a["id"].as_u64() == Some(id))?;
                self.remember_artist(matched);
                Some(ArtistHint {
                    id,
                    picture: artist_picture(matched["picture"].as_str().unwrap_or("")),
                    fans: matched["numberOfFans"].as_u64(),
                })
            });
            if let Some(hint) = hint {
                artist.picture = hint.picture;
                artist.fans = artist.fans.or(hint.fans);
            }
        }
        Ok(artist)
    }
    pub(crate) fn artist_with_preview(
        &mut self,
        id: u64,
        mut preview: impl FnMut(&Artist),
    ) -> Result<Artist> {
        let mut artist = self.artist_header(id)?;
        // Start the portrait/header immediately, while the remaining sections load.
        preview(&artist);
        match self.get(
            &format!("artists/{id}/toptracks"),
            &[("limit", "20".into())],
        ) {
            Ok(v) => {
                artist.tracks = std::sync::Arc::new(
                    v["items"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(Track::from_v1)
                        .collect(),
                )
            }
            Err(_) => artist
                .notices
                .push("Top tracks are unavailable from TIDAL.".into()),
        }
        preview(&artist);
        for (filter, singles) in [("ALBUMS", false), ("EPSANDSINGLES", true)] {
            match self.get(
                &format!("artists/{id}/albums"),
                &[("limit", "50".into()), ("filter", filter.into())],
            ) {
                Ok(v) => {
                    let albums = v["items"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|v| Album {
                            id: v["id"].as_u64().unwrap_or(0),
                            title: v["title"].as_str().unwrap_or("").into(),
                            artist: artist.name.clone(),
                            cover: v["cover"].as_str().unwrap_or("").into(),
                            year: v["releaseDate"]
                                .as_str()
                                .unwrap_or("")
                                .chars()
                                .take(4)
                                .collect(),
                            explicit: v["explicit"].as_bool().unwrap_or(false),
                            release_type: v["type"].as_str().unwrap_or("").into(),
                            ..Default::default()
                        })
                        .filter(|a| a.id > 0)
                        .collect();
                    if singles {
                        artist.singles = albums;
                    } else {
                        artist.albums = albums;
                    }
                }
                Err(_) => artist.notices.push(format!(
                    "{} are unavailable from TIDAL.",
                    if singles { "Singles" } else { "Albums" }
                )),
            }
        }
        if let Ok(bio) = self.get(&format!("artists/{id}/bio"), &[]) {
            let raw = bio["text"]
                .as_str()
                .or_else(|| bio["summary"].as_str())
                .unwrap_or("");
            artist.biography = biography_text(raw);
        }
        if let Ok(followed) = self.v1_pages(
            &format!("users/{}/favorites/artists", self.session.user_id),
            &[],
        ) {
            artist.followed = Some(followed.iter().any(|v| {
                v.pointer("/item/id")
                    .or_else(|| v.get("id"))
                    .and_then(Value::as_u64)
                    == Some(id)
            }));
        }
        artist.album_shelf = crate::model::release_shelf(&artist.albums);
        artist.single_shelf = crate::model::release_shelf(&artist.singles);
        Ok(artist)
    }

    pub fn album_info(&mut self, id: u64) -> Result<Album> {
        ensure!(id > 0, "Missing album identifier");
        if !self.session.compatibility {
            return Ok(Album {
                id,
                ..Default::default()
            });
        }
        let v = self.get(&format!("albums/{id}"), &[])?;
        let artist = v
            .get("artist")
            .or_else(|| v.pointer("/artists/0"))
            .unwrap_or(&Value::Null);
        let mut picture = artist["picture"].as_str().unwrap_or("").to_owned();
        if picture.is_empty()
            && let Some(aid) = artist["id"].as_u64()
            && let Ok(a) = self.get(&format!("artists/{aid}"), &[])
        {
            picture = a["picture"].as_str().unwrap_or("").into();
        }
        Ok(Album {
            id,
            title: v["title"].as_str().unwrap_or("").into(),
            artist: artist["name"].as_str().unwrap_or("").into(),
            artist_picture: picture,
            cover: v["cover"].as_str().unwrap_or("").into(),
            year: v["releaseDate"]
                .as_str()
                .unwrap_or("")
                .chars()
                .take(4)
                .collect(),
            quality: if v
                .pointer("/mediaMetadata/tags")
                .and_then(Value::as_array)
                .is_some_and(|t| t.iter().any(|t| t == "HIRES_LOSSLESS"))
            {
                "HI_RES_LOSSLESS".into()
            } else {
                v["audioQuality"].as_str().unwrap_or("").into()
            },
            copyright: v["copyright"].as_str().unwrap_or("").into(),
            explicit: v["explicit"].as_bool().unwrap_or(false),
            release_type: v["type"].as_str().unwrap_or("").into(),
        })
    }

    pub fn save_album(&mut self, id: u64) -> Result<()> {
        ensure!(
            self.session.compatibility,
            "Saving albums requires the lossless web connection"
        );
        let uid = self.session.user_id;
        read_json(self.request(
            Method::POST,
            &format!("users/{uid}/favorites/albums"),
            &[],
            None,
            Some(vec![("albumId", id.to_string())]),
            None,
        )?)?;
        Ok(())
    }
    pub fn collection_state(&mut self) -> Result<Vec<String>> {
        ensure!(
            self.session.compatibility,
            "Collection requires the lossless web connection"
        );
        let uid = self.session.user_id;
        Ok(self
            .v1_pages(&format!("users/{uid}/favorites/albums"), &[])?
            .iter()
            .filter_map(|v| {
                v.pointer("/item/id")
                    .or_else(|| v.get("id"))
                    .and_then(Value::as_u64)
            })
            .map(|id| format!("album:{id}"))
            .collect())
    }
    pub fn collection_set(&mut self, key: &str, save: bool) -> Result<()> {
        ensure!(
            self.session.compatibility,
            "Collection requires the lossless web connection"
        );
        if save && let Some(id) = key.strip_prefix("album:") {
            return self.save_album(id.parse()?);
        }
        let (kind, id, field) = if let Some(id) = key.strip_prefix("album:") {
            ("albums", id, "albumId")
        } else if let Some(id) = key.strip_prefix("artist:") {
            ("artists", id, "artistId")
        } else {
            ("playlists", key, "uuid")
        };
        valid_id(id)?;
        let uid = self.session.user_id;
        let path = if save {
            format!("users/{uid}/favorites/{kind}")
        } else {
            format!("users/{uid}/favorites/{kind}/{id}")
        };
        read_json(self.request(
            if save { Method::POST } else { Method::DELETE },
            &path,
            &[],
            None,
            if save {
                Some(vec![(field, id.to_owned())])
            } else {
                None
            },
            None,
        )?)?;
        Ok(())
    }
    pub fn create_playlist(&mut self, title: &str, description: &str) -> Result<String> {
        ensure!(
            !title.trim().is_empty() && title.chars().count() <= 250,
            "Playlist name must contain 1–250 characters"
        );
        ensure!(
            description.chars().count() <= 500,
            "Playlist description exceeds 500 characters"
        );
        let r = if self.session.compatibility {
            self.request(
                Method::POST,
                &format!("users/{}/playlists", self.session.user_id),
                &[],
                None,
                Some(vec![
                    ("title", title.into()),
                    ("description", description.into()),
                ]),
                None,
            )?
        } else {
            self.request(Method::POST,"playlists",&[],Some(json!({"data":{"type":"playlists","attributes":{"name":title,"description":description,"accessType":"UNLISTED"}}})),None,None)?
        };
        let result = read_json(r)?;
        let id = result["uuid"].as_str().or_else(|| result.pointer("/data/id").and_then(Value::as_str))
            .context("Playlist was created but TIDAL omitted its identifier. Sync your collection before trying again.")?;
        valid_id(id)?;
        Ok(id.to_owned())
    }
    pub fn edit_playlist(&mut self, id: &str, title: &str, description: &str) -> Result<()> {
        valid_id(id)?;
        ensure!(
            !title.trim().is_empty() && title.chars().count() <= 250,
            "Playlist name must contain 1–250 characters"
        );
        ensure!(
            description.chars().count() <= 500,
            "Playlist description exceeds 500 characters"
        );
        let r = if self.session.compatibility {
            self.request(
                Method::POST,
                &format!("playlists/{id}"),
                &[],
                None,
                Some(vec![
                    ("title", title.into()),
                    ("description", description.into()),
                ]),
                None,
            )?
        } else {
            self.request(Method::PATCH,&format!("playlists/{id}"),&[],Some(json!({"data":{"type":"playlists","id":id,"attributes":{"name":title,"description":description}}})),None,None)?
        };
        read_json(r)?;
        Ok(())
    }
    pub fn playlist_visibility(&mut self, id: &str, public: bool) -> Result<()> {
        valid_id(id)?;
        let response = if self.session.compatibility {
            // Resolve the compatibility API's v2 route against its v1 base URL.
            self.request(
                Method::PUT,
                &format!(
                    "../v2/playlists/{id}/set-{}",
                    if public { "public" } else { "private" }
                ),
                &[],
                None,
                None,
                None,
            )?
        } else {
            self.request(Method::PATCH, &format!("playlists/{id}"), &[], Some(json!({"data":{"type":"playlists","id":id,"attributes":{"accessType":if public {"PUBLIC"} else {"UNLISTED"}}}})), None, None)?
        };
        read_json(response)?;
        Ok(())
    }
    pub fn delete_playlist(&mut self, id: &str) -> Result<()> {
        valid_id(id)?;
        let r = self.request(
            Method::DELETE,
            &format!("playlists/{id}"),
            &[],
            None,
            None,
            None,
        )?;
        read_json(r)?;
        Ok(())
    }
    pub fn add_track(&mut self, id: &str, track: u64) -> Result<()> {
        self.add_tracks(id, &[track])
    }
    pub fn add_tracks(&mut self, id: &str, tracks: &[u64]) -> Result<()> {
        for chunk in tracks.chunks(50) {
            self.add_track_batch(id, chunk)?;
        }
        Ok(())
    }
    fn add_track_batch(&mut self, id: &str, tracks: &[u64]) -> Result<()> {
        valid_id(id)?;
        let r = if self.session.compatibility {
            let e = self.etags.get(id).cloned();
            self.request(
                Method::POST,
                &format!("playlists/{id}/items"),
                &[],
                None,
                Some(vec![
                    (
                        "trackIds",
                        tracks
                            .iter()
                            .map(u64::to_string)
                            .collect::<Vec<_>>()
                            .join(","),
                    ),
                    ("onDupes", "SKIP".into()),
                    ("onArtifactNotFound", "FAIL".into()),
                ]),
                e,
            )?
        } else {
            self.request(Method::POST,&format!("playlists/{id}/relationships/items"),&[],Some(json!({"data":tracks.iter().map(|t|json!({"type":"tracks","id":t.to_string()})).collect::<Vec<_>>(),"meta":{"onDuplicates":"SKIP"}})),None,None)?
        };
        if let Some(revision) = r.headers().get(ETAG).and_then(|v| v.to_str().ok()) {
            self.etags.insert(id.into(), revision.into());
        } else {
            self.etags.remove(id);
        }
        read_json(r)?;
        Ok(())
    }
    pub fn remove_track(&mut self, id: &str, _index: usize, track: &Track) -> Result<()> {
        valid_id(id)?;
        let r = if self.session.compatibility {
            let index = track
                .playlist_index
                .context("Sync this playlist before removing an item")?;
            let e = self
                .etags
                .get(id)
                .cloned()
                .context("Sync playlist before removing tracks; no revision available")?;
            self.request(
                Method::DELETE,
                &format!("playlists/{id}/items/{index}"),
                &[],
                None,
                None,
                Some(e),
            )?
        } else {
            ensure!(
                !track.playlist_item_id.is_empty(),
                "Missing playlist item identifier; sync first"
            );
            self.request(Method::DELETE,&format!("playlists/{id}/relationships/items"),&[],Some(json!({"data":[{"type":"tracks","id":track.id.to_string(),"meta":{"itemId":track.playlist_item_id}}]})),None,None)?
        };
        read_json(r)?;
        Ok(())
    }
    pub fn move_track(
        &mut self,
        id: &str,
        index: usize,
        to: usize,
        tracks: &[Track],
    ) -> Result<()> {
        valid_id(id)?;
        ensure!(
            index < tracks.len() && to < tracks.len(),
            "Invalid playlist position"
        );
        let r = if self.session.compatibility {
            let index = tracks[index]
                .playlist_index
                .context("Sync this playlist before reordering")?;
            let to = tracks[to]
                .playlist_index
                .context("Sync this playlist before reordering")?;
            let e = self
                .etags
                .get(id)
                .cloned()
                .context("Sync playlist before reordering; no revision available")?;
            self.request(
                Method::POST,
                &format!("playlists/{id}/items/{index}"),
                &[],
                None,
                Some(vec![("toIndex", to.to_string())]),
                Some(e),
            )?
        } else {
            let t = &tracks[index];
            let before = if to > index {
                tracks.get(to + 1)
            } else {
                tracks.get(to)
            };
            let before=before.context("Official API requires a positionBefore item; moving to the end is not supported yet")?;
            ensure!(
                !t.playlist_item_id.is_empty() && !before.playlist_item_id.is_empty(),
                "Sync playlist first"
            );
            self.request(Method::PATCH,&format!("playlists/{id}/relationships/items"),&[],Some(json!({"data":[{"type":"tracks","id":t.id.to_string(),"meta":{"itemId":t.playlist_item_id}}],"meta":{"positionBefore":before.playlist_item_id}})),None,None)?
        };
        read_json(r)?;
        Ok(())
    }
    pub fn favorite(&mut self, id: u64, save: bool) -> Result<()> {
        let method = if save { Method::POST } else { Method::DELETE };
        let r = if self.session.compatibility {
            let path = if save {
                format!("users/{}/favorites/tracks", self.session.user_id)
            } else {
                format!("users/{}/favorites/tracks/{id}", self.session.user_id)
            };
            self.request(
                method,
                &path,
                &[],
                None,
                if save {
                    Some(vec![("trackIds", id.to_string())])
                } else {
                    None
                },
                None,
            )?
        } else {
            self.request(
                method,
                "userCollectionTracks/me/relationships/items",
                &[],
                Some(json!({"data":[{"type":"tracks","id":id.to_string()}]})),
                None,
                None,
            )?
        };
        read_json(r)?;
        Ok(())
    }
    pub fn best_version(&mut self, track: &Track) -> Result<Track> {
        if track.isrc.is_empty()
            || !self.settings.real_max
            || quality_rank(&track.quality) >= quality_rank("HI_RES_LOSSLESS")
        {
            return Ok(track.clone());
        }
        let versions = self.search(&format!("{} {}", track.title, track.artist))?;
        Ok(select_best_version(track, versions))
    }
    pub fn stream(&mut self, id: u64, download: bool) -> Result<StreamPlan> {
        ensure!(
            self.session.compatibility,
            "Native playback is not exposed by the public REST API. Use desktop compatibility, or a TIDAL-approved native playback integration."
        );
        let quality = self.settings.quality.clone();
        let v = self.get(
            &format!("tracks/{id}/playbackinfopostpaywall"),
            &[
                ("audioquality", quality),
                (
                    "playbackmode",
                    if download { "OFFLINE" } else { "STREAM" }.into(),
                ),
                ("assetpresentation", "FULL".into()),
                ("prefetchlevel", "NONE".into()),
            ],
        )?;
        let actual = v["audioQuality"].as_str().unwrap_or("UNKNOWN");
        if self.settings.strict_lossless {
            if self.session.client_id == auth::DEVICE_CLIENT_ID
                && !["LOSSLESS", "HI_RES_LOSSLESS"].contains(&actual)
            {
                bail!(
                    "This website-code session returned standard-quality audio. Choose Settings > Account > Lossless web sign-in to enable lossless playback, or explicitly turn off Lossless only."
                );
            }
            ensure!(
                ["LOSSLESS", "HI_RES_LOSSLESS"].contains(&actual),
                "TIDAL returned {actual}; lossless-only mode refused a lossy fallback"
            );
        }
        parse_stream(&v)
    }
    pub fn tidal_lyrics(&mut self, id: u64) -> Result<(String, String)> {
        if self.session.compatibility {
            let v = self.get(&format!("tracks/{id}/lyrics"), &[])?;
            Ok((
                v["subtitles"].as_str().unwrap_or("").into(),
                v["lyrics"].as_str().unwrap_or("").into(),
            ))
        } else {
            bail!("TIDAL lyrics currently require internal access; LRCLIB fallback is available")
        }
    }
}

fn parse_credits(value: &Value) -> Vec<CreditGroup> {
    let Some(items) = value.as_array().or_else(|| value["items"].as_array()) else {
        return Vec::new();
    };
    let mut groups: Vec<CreditGroup> = Vec::new();
    for item in items {
        let raw = item["type"]
            .as_str()
            .or_else(|| item["role"].as_str())
            .unwrap_or("");
        if raw.is_empty() {
            continue;
        }
        let role = match raw.to_lowercase().replace(['_', ' '], "").as_str() {
            "producer" => "Producer".into(),
            "engineer" => "Engineer".into(),
            "mixer" => "Mixer".into(),
            "musicpublisher" | "publisher" => "Music Publisher".into(),
            "composer" => "Composer".into(),
            "lyricist" => "Lyricist".into(),
            "performer" => "Performer".into(),
            _ => raw.replace('_', " "),
        };
        let names = if let Some(contributors) = item["contributors"].as_array() {
            contributors
                .iter()
                .filter_map(|c| c["name"].as_str())
                .filter(|n| !n.is_empty())
                .collect::<Vec<_>>()
        } else {
            item["name"]
                .as_str()
                .filter(|n| !n.is_empty())
                .into_iter()
                .collect()
        }
        .join(", ");
        if names.is_empty() {
            continue;
        }
        if let Some(group) = groups.iter_mut().find(|g| g.role == role) {
            group.names.push_str(", ");
            group.names.push_str(&names);
        } else {
            groups.push(CreditGroup { role, names });
        }
    }
    groups
}

fn valid_id(id: &str) -> Result<()> {
    ensure!(
        !id.is_empty()
            && id.len() <= 128
            && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
        "Invalid TIDAL identifier"
    );
    Ok(())
}
fn included(v: &Value) -> impl Iterator<Item = &Value> {
    v["included"].as_array().into_iter().flatten()
}
fn related<'a>(v: &Value, doc: &'a Value, key: &str) -> Option<&'a Value> {
    let d = v.pointer(&format!("/relationships/{key}/data"))?;
    let id = if d.is_array() { d.get(0)? } else { d };
    included(doc).find(|v| v["id"] == id["id"] && v["type"] == id["type"])
}
pub fn track_v2(v: &Value, doc: &Value) -> Track {
    let a = &v["attributes"];
    let album = related(v, doc, "albums");
    let cover = album.and_then(|a| related(a, doc, "coverArt"));
    Track {
        id: v["id"].as_str().unwrap_or("0").parse().unwrap_or(0),
        artist_id: related(v, doc, "artists")
            .and_then(|v| v["id"].as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
        title: a["title"].as_str().unwrap_or("").into(),
        artist: related(v, doc, "artists")
            .and_then(|v| v["attributes"]["name"].as_str())
            .unwrap_or("")
            .into(),
        album: album
            .and_then(|v| v["attributes"]["title"].as_str())
            .unwrap_or("")
            .into(),
        album_id: album
            .and_then(|v| v["id"].as_str())
            .unwrap_or("0")
            .parse()
            .unwrap_or(0),
        cover: cover
            .and_then(|v| v.pointer("/attributes/files/0/href"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .into(),
        duration: iso_duration(a["duration"].as_str().unwrap_or("")),
        quality: a["mediaTags"]
            .as_array()
            .map(|x| {
                x.iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(" · ")
            })
            .unwrap_or_default(),
        isrc: a["isrc"].as_str().unwrap_or("").into(),
        explicit: a["explicit"].as_bool().unwrap_or(false),
        ..Default::default()
    }
}
pub fn iso_duration(s: &str) -> u64 {
    let mut total = 0.0;
    let mut n = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() || c == '.' {
            n.push(c);
        } else if let Ok(v) = n.parse::<f64>() {
            total += v * match c {
                'H' => 3600.0,
                'M' => 60.0,
                'S' => 1.0,
                _ => 0.0,
            };
            n.clear();
        }
    }
    total as u64
}
fn quality_rank(s: &str) -> u32 {
    if s.contains("HI_RES") || s.contains("HIRES") {
        3
    } else if s.contains("LOSSLESS") {
        2
    } else {
        1
    }
}
pub fn select_best_version(original: &Track, candidates: Vec<Track>) -> Track {
    candidates
        .into_iter()
        .filter(|t| {
            !original.isrc.is_empty()
                && t.isrc == original.isrc
                && t.explicit == original.explicit
                && t.duration.abs_diff(original.duration) <= 2
        })
        .max_by_key(|t| quality_rank(&t.quality))
        .filter(|t| quality_rank(&t.quality) > quality_rank(&original.quality))
        .unwrap_or_else(|| original.clone())
}

pub fn parse_stream(v: &Value) -> Result<StreamPlan> {
    let bytes = STANDARD.decode(
        v["manifest"]
            .as_str()
            .context("Missing playback manifest")?,
    )?;
    ensure!(bytes.len() < 2 * 1024 * 1024, "Playback manifest too large");
    let mime = v["manifestMimeType"].as_str().unwrap_or("");
    let mut plan = StreamPlan {
        urls: Vec::new(),
        encryption: None,
        quality: v["audioQuality"].as_str().unwrap_or("UNKNOWN").into(),
        codec: String::new(),
        bit_depth: v["bitDepth"].as_u64().unwrap_or(0) as u32,
        sample_rate: v["sampleRate"].as_u64().unwrap_or(0) as u32,
    };
    if mime.contains("bts") {
        let m: Value = serde_json::from_slice(&bytes)?;
        plan.codec = m["codecs"].as_str().unwrap_or("").into();
        match m["encryptionType"].as_str().unwrap_or("NONE") {
            "NONE" | "" => {}
            "OLD_AES" => {
                plan.encryption = Some(unwrap_legacy_key(
                    m["keyId"].as_str().context("Missing legacy stream key")?,
                )?)
            }
            other => bail!("Unsupported protected stream type {other}; playback was not attempted"),
        }
        plan.urls.push(
            m["urls"]
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .context("No stream URL")?
                .into(),
        );
    } else if mime.contains("dash") {
        let (urls, codec, rate) = parse_dash(&String::from_utf8(bytes)?)?;
        plan.urls = urls;
        plan.codec = codec;
        if plan.sample_rate == 0 {
            plan.sample_rate = rate;
        }
    } else {
        bail!("Unsupported manifest {mime}");
    }
    for u in &plan.urls {
        let u = url::Url::parse(u)?;
        ensure!(
            u.scheme() == "https" && u.host_str().is_some_and(|h| h.ends_with(".tidal.com")),
            "Untrusted media URL"
        );
    }
    Ok(plan)
}

// Compatible with the documented legacy stream format used by Lumitide (MIT).
fn unwrap_legacy_key(s: &str) -> Result<([u8; 16], [u8; 8])> {
    use cbc::cipher::{BlockDecryptMut, KeyIvInit, block_padding::NoPadding};
    use zeroize::Zeroize;
    let mut token = STANDARD.decode(s)?;
    ensure!(
        token.len() >= 48 && (token.len() - 16) % 16 == 0,
        "Malformed encrypted key token"
    );
    let master = STANDARD.decode("UIlTTEMmmLfGowo/UC60x2H45W6MdGgTRfo/umg4754=")?;
    let (iv, data) = token.split_at_mut(16);
    let clear = cbc::Decryptor::<aes::Aes256>::new_from_slices(&master, iv)
        .map_err(|_| anyhow::anyhow!("Invalid legacy key length"))?
        .decrypt_padded_mut::<NoPadding>(data)
        .map_err(|_| anyhow::anyhow!("Invalid legacy key padding"))?;
    ensure!(clear.len() >= 24, "Truncated stream key");
    let mut key = [0; 16];
    let mut nonce = [0; 8];
    key.copy_from_slice(&clear[..16]);
    nonce.copy_from_slice(&clear[16..24]);
    token.zeroize();
    Ok((key, nonce))
}

pub fn parse_dash(xml: &str) -> Result<(Vec<String>, String, u32)> {
    use quick_xml::{Reader, events::Event};
    ensure!(
        !xml.contains("ContentProtection"),
        "DASH content protection requires an approved DRM playback module"
    );
    let mut r = Reader::from_str(xml);
    r.config_mut().trim_text(true);
    let mut base = String::new();
    let mut in_base = false;
    let mut init = String::new();
    let mut media = String::new();
    let mut number = 1u64;
    let mut durations = Vec::new();
    let mut codec = String::new();
    let mut rate = 0;
    let mut rep = String::new();
    loop {
        match r.read_event()? {
            Event::Start(e) | Event::Empty(e) => {
                let name = e.local_name();
                let n = name.as_ref();
                if n == b"BaseURL" {
                    in_base = true;
                }
                let attrs: HashMap<String, String> = e
                    .attributes()
                    .filter_map(|a| a.ok())
                    .map(|a| {
                        (
                            String::from_utf8_lossy(a.key.as_ref()).to_string(),
                            a.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .unwrap_or_default()
                                .into_owned(),
                        )
                    })
                    .collect();
                if n == b"Representation" {
                    ensure!(
                        rep.is_empty(),
                        "Multiple DASH representations need explicit selection"
                    );
                    rep = attrs.get("id").cloned().unwrap_or_default();
                    codec = attrs.get("codecs").cloned().unwrap_or_default();
                    rate = attrs
                        .get("audioSamplingRate")
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                }
                if n == b"SegmentTemplate" {
                    init = attrs.get("initialization").cloned().unwrap_or_default();
                    media = attrs.get("media").cloned().unwrap_or_default();
                    number = attrs
                        .get("startNumber")
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(1);
                }
                if n == b"S" {
                    let repeat: i64 = attrs.get("r").and_then(|s| s.parse().ok()).unwrap_or(0);
                    ensure!(
                        (0..=10000).contains(&repeat),
                        "Unbounded DASH timeline is unsupported"
                    );
                    let start = attrs.get("t").and_then(|s| s.parse::<u64>().ok());
                    let dur = attrs
                        .get("d")
                        .and_then(|s| s.parse::<u64>().ok())
                        .context("DASH omitted segment duration")?;
                    durations.push((start, dur, repeat as u64));
                }
            }
            Event::Text(e) if in_base => {
                base = quick_xml::escape::unescape(&e.decode()?)?.into_owned()
            }
            Event::End(e) if e.local_name().as_ref() == b"BaseURL" => in_base = false,
            Event::Eof => break,
            _ => {}
        }
    }
    if media.is_empty() {
        ensure!(
            !base.is_empty(),
            "DASH manifest has no progressive URL or segment template"
        );
        return Ok((vec![base], codec, rate));
    }
    ensure!(
        !init.is_empty() && !durations.is_empty(),
        "DASH segment template lacks initialization or timeline"
    );
    let expand = |s: &str, n: u64, t: u64| -> Result<String> {
        let s = s
            .replace("$RepresentationID$", &rep)
            .replace("$Number$", &n.to_string())
            .replace("$Time$", &t.to_string())
            .replace("&amp;", "&");
        ensure!(!s.contains('$'), "Unsupported DASH URL formatting");
        if s.starts_with("https://") {
            Ok(s)
        } else {
            Ok(url::Url::parse(&base)?.join(&s)?.to_string())
        }
    };
    let mut urls = vec![expand(&init, number, 0)?];
    let mut time = 0;
    for (start, dur, repeats) in durations {
        if let Some(s) = start {
            time = s;
        }
        for _ in 0..=repeats {
            ensure!(urls.len() < 15000, "DASH timeline too large");
            urls.push(expand(&media, number, time)?);
            number += 1;
            time += dur;
        }
    }
    Ok((urls, codec, rate))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_artist_photo_uses_exact_search_id_and_shared_hints_avoid_extra_queries() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            for call in 0..3 {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut byte = [0];
                while !bytes.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    bytes.push(byte[0]);
                }
                let request = String::from_utf8(bytes).unwrap();
                let body=if call==1 {
                    assert!(request.starts_with("GET /search?"));
                    json!({"artists":{"items":[{"id":2,"name":"Same Name","picture":"wrong-portrait"},{"id":42,"name":"Same Name","picture":"right-portrait","numberOfFans":987}]}})
                } else {
                    assert!(request.starts_with("GET /artists/42?"));
                    json!({"id":42,"name":"Same Name","picture":null})
                }.to_string();
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });
        let mut tidal = Tidal::new(
            Session {
                access_token: "test-only".into(),
                expires_at: u64::MAX,
                compatibility: true,
                refresh_token: String::new(),
                user_id: 1,
                country: "CA".into(),
                client_id: "test-only".into(),
            },
            Settings::default(),
        )
        .unwrap();
        tidal.test_base = Some(base);
        let artist = tidal.artist_header(42).unwrap();
        assert_eq!(
            artist.picture,
            "https://resources.tidal.com/images/right/portrait/750x750.jpg"
        );
        assert_eq!(artist.fans, Some(987));
        let mut read = tidal.fork_read(std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
            false,
        )));
        assert_eq!(read.artist_header(42).unwrap().picture, artist.picture);
        server.join().unwrap();
    }
    #[test]
    fn playlist_draft_returns_id_enforces_unicode_limits_and_uses_visibility_routes() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/v1/", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            for call in 0..5 {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut byte = [0];
                while !bytes.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    bytes.push(byte[0]);
                }
                let headers = String::from_utf8(bytes).unwrap();
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|s| s.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                let mut body = vec![0; length];
                stream.read_exact(&mut body).unwrap();
                let form = url::form_urlencoded::parse(&body)
                    .into_owned()
                    .collect::<std::collections::HashMap<_, _>>();
                match call {
                    0 => {
                        assert!(headers.starts_with("POST /v1/users/1/playlists?"));
                        assert_eq!(form["title"], "Night & day");
                        assert_eq!(form["description"].chars().count(), 500);
                    }
                    1 | 4 => assert!(headers.starts_with("PUT /v2/playlists/pl1/set-private?")),
                    2 => assert!(headers.starts_with("PUT /v2/playlists/pl1/set-public?")),
                    3 => {
                        assert!(headers.starts_with("POST /v1/playlists/pl1?"));
                        assert_eq!(form["title"], "Edited");
                    }
                    _ => unreachable!(),
                }
                let status = if call == 4 { "403 Forbidden" } else { "200 OK" };
                let body = if call == 0 {
                    "{\"uuid\":\"pl1\"}"
                } else {
                    "{}"
                };
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });
        let mut t = Tidal::new(
            Session {
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
        assert_eq!(
            t.create_playlist("Night & day", &"猫".repeat(500)).unwrap(),
            "pl1"
        );
        t.playlist_visibility("pl1", false).unwrap();
        t.playlist_visibility("pl1", true).unwrap();
        t.edit_playlist("pl1", "Edited", "").unwrap();
        assert!(
            t.playlist_visibility("pl1", false)
                .unwrap_err()
                .to_string()
                .contains("403")
        );
        assert!(t.create_playlist(" ", "").is_err());
        assert!(t.create_playlist("Valid", &"猫".repeat(501)).is_err());
        assert!(t.edit_playlist("pl1", &"a".repeat(251), "").is_err());
        assert!(t.playlist_visibility("../escape", true).is_err());
        server.join().unwrap();
    }
    #[test]
    fn paginated_playlist_keeps_video_indices_and_revision_on_delete() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            for call in 0..4 {
                let (mut s, _) = listener.accept().unwrap();
                s.set_read_timeout(Some(std::time::Duration::from_secs(3)))
                    .unwrap();
                let mut data = Vec::new();
                let mut byte = [0; 1];
                while !data.ends_with(b"\r\n\r\n") {
                    s.read_exact(&mut byte).unwrap();
                    data.push(byte[0]);
                }
                let request = String::from_utf8(data).unwrap();
                let (status, body, extra) = match call {
                    0 => {
                        assert!(request.starts_with("GET /playlists/pl1?"));
                        ("200 OK", "{}".into(), "ETag: \"revision-1\"\r\n")
                    }
                    1 | 2 => {
                        let offset = if call == 1 { 0 } else { 100 };
                        assert!(request.contains(&format!("offset={offset}")));
                        let end = if call == 1 { 100 } else { 102 };
                        let items=(offset..end).map(|i|json!({"type":if i==0{"video"}else{"track"},"item":{"id":i,"title":format!("Track {i}")}})).collect::<Vec<_>>();
                        (
                            "200 OK",
                            json!({"items":items,"totalNumberOfItems":102}).to_string(),
                            "",
                        )
                    }
                    _ => {
                        assert!(request.starts_with("DELETE /playlists/pl1/items/1?"));
                        assert!(
                            request
                                .to_lowercase()
                                .contains("if-none-match: \"revision-1\"")
                        );
                        ("412 Precondition Failed", "{}".into(), "")
                    }
                };
                write!(s,"HTTP/1.1 {status}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            }
        });
        let session = Session {
            access_token: "test-only".into(),
            refresh_token: String::new(),
            expires_at: u64::MAX,
            user_id: 1,
            country: "CA".into(),
            compatibility: true,
            client_id: "test-only".into(),
        };
        let mut t = Tidal::new(session, Settings::default()).unwrap();
        t.test_base = Some(base);
        let tracks = t.playlist_tracks("pl1").unwrap();
        assert_eq!(tracks.len(), 101);
        assert_eq!(tracks[0].playlist_index, Some(1));
        assert_eq!(tracks[100].playlist_index, Some(101));
        let error = t.remove_track("pl1", 0, &tracks[0]).unwrap_err();
        assert!(error.to_string().contains("changed elsewhere"));
        server.join().unwrap();
    }
    #[test]
    fn realmax_never_replaces_different_recording() {
        let t = Track {
            id: 1,
            isrc: "same".into(),
            quality: "LOSSLESS".into(),
            duration: 100,
            ..Default::default()
        };
        let mut other = t.clone();
        other.id = 2;
        other.quality = "HI_RES_LOSSLESS".into();
        other.isrc = "different".into();
        assert_eq!(select_best_version(&t, vec![other]).id, 1);
        let mut hi = t.clone();
        hi.id = 3;
        hi.quality = "HI_RES_LOSSLESS".into();
        assert_eq!(select_best_version(&t, vec![hi]).id, 3);
    }
    #[test]
    fn credits_keep_provider_roles_and_people_without_fabrication() {
        let groups = parse_credits(&json!([
            {"type":"Producer","contributors":[{"name":"A"},{"name":"B"}]},
            {"type":"Producer","contributors":[{"name":"C"}]},
            {"type":"MUSIC_PUBLISHER","contributors":[{"name":"Publisher"}]},
            {"type":"Engineer","contributors":[]}
        ]));
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].role, "Producer");
        assert_eq!(groups[0].names, "A, B, C");
        assert_eq!(groups[1].role, "Music Publisher");
        assert!(parse_credits(&json!({})).is_empty());
    }
    #[test]
    fn collection_and_bulk_playlist_actions_use_real_routes_and_new_revisions() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            for call in 0..8 {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                    .unwrap();
                let mut header = Vec::new();
                let mut byte = [0];
                while !header.ends_with(b"\r\n\r\n") {
                    socket.read_exact(&mut byte).unwrap();
                    header.push(byte[0]);
                }
                let header = String::from_utf8(header).unwrap();
                let len = header
                    .lines()
                    .find_map(|l| {
                        l.to_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|s| s.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                let mut body = vec![0; len];
                socket.read_exact(&mut body).unwrap();
                let body = String::from_utf8(body).unwrap();
                let (status, response, extra) = match call {
                    0 => {
                        assert!(header.starts_with("GET /users/1/favorites/albums?"));
                        (
                            "200 OK",
                            r#"{"items":[{"item":{"id":99}}],"totalNumberOfItems":1}"#,
                            "",
                        )
                    }
                    1 => {
                        assert!(header.starts_with("POST /users/1/favorites/albums?"));
                        assert_eq!(body, "albumId=99");
                        ("200 OK", "{}", "")
                    }
                    2 => {
                        assert!(header.starts_with("DELETE /users/1/favorites/albums/99?"));
                        ("204 No Content", "", "")
                    }
                    3 | 4 => {
                        assert!(header.starts_with("POST /playlists/pl1/items?"));
                        assert!(header.to_lowercase().contains(if call == 3 {
                            "if-none-match: old"
                        } else {
                            "if-none-match: new"
                        }));
                        let fields =
                            url::form_urlencoded::parse(body.as_bytes()).collect::<HashMap<_, _>>();
                        assert_eq!(
                            fields["trackIds"].split(',').count(),
                            if call == 3 { 50 } else { 1 }
                        );
                        ("200 OK", "{}", "ETag: new\r\n")
                    }
                    6 => {
                        assert!(header.starts_with("POST /users/1/favorites/artists?"));
                        assert_eq!(body, "artistId=42");
                        ("200 OK", "{}", "")
                    }
                    7 => {
                        assert!(header.starts_with("DELETE /users/1/favorites/artists/42?"));
                        ("204 No Content", "", "")
                    }
                    _ => {
                        assert!(header.starts_with("POST /users/1/favorites/playlists?"));
                        assert_eq!(body, "uuid=denied");
                        ("403 Forbidden", "{}", "")
                    }
                };
                write!(socket,"HTTP/1.1 {status}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{response}",response.len()).unwrap();
            }
        });
        let session = Session {
            access_token: "test-only".into(),
            refresh_token: String::new(),
            expires_at: u64::MAX,
            user_id: 1,
            country: "CA".into(),
            compatibility: true,
            client_id: "test-only".into(),
        };
        let mut tidal = Tidal::new(session, Settings::default()).unwrap();
        tidal.test_base = Some(base);
        assert_eq!(tidal.collection_state().unwrap(), vec!["album:99"]);
        tidal.collection_set("album:99", true).unwrap();
        tidal.collection_set("album:99", false).unwrap();
        tidal.etags.insert("pl1".into(), "old".into());
        tidal
            .add_tracks("pl1", &(1..=51).collect::<Vec<_>>())
            .unwrap();
        assert!(tidal.collection_set("denied", true).is_err());
        tidal.collection_set("artist:42", true).unwrap();
        tidal.collection_set("artist:42", false).unwrap();
        server.join().unwrap();
    }
    #[test]
    fn dash_segments_are_not_mistaken_for_full_audio() {
        let xml = r#"<MPD><Period><AdaptationSet><Representation id="a" codecs="flac" audioSamplingRate="96000"><SegmentTemplate initialization="https://x.tidal.com/init" media="https://x.tidal.com/$Number$.mp4" startNumber="1"><SegmentTimeline><S d="42" r="2"/></SegmentTimeline></SegmentTemplate></Representation></AdaptationSet></Period></MPD>"#;
        let (u, c, r) = parse_dash(xml).unwrap();
        assert_eq!(u.len(), 4);
        assert_eq!(u[3], "https://x.tidal.com/3.mp4");
        assert_eq!(c, "flac");
        assert_eq!(r, 96000);
    }
    #[test]
    fn encrypted_dash_is_rejected() {
        assert!(parse_dash("<MPD><ContentProtection/></MPD>").is_err());
    }
    #[test]
    fn malformed_key_is_rejected() {
        assert!(unwrap_legacy_key("AA==").is_err());
    }
    #[test]
    fn iso_times() {
        assert_eq!(iso_duration("PT1H2M3.2S"), 3723);
    }
}

fn biography_text(raw: &str) -> String {
    let bounded: String = raw.chars().take(32000).collect();
    let mut rest = bounded.as_str();
    let mut out = String::new();
    while !rest.is_empty() {
        if (rest.starts_with("[wimpLink ")
            || rest.starts_with("[wimpLink]")
            || rest.starts_with("[/wimpLink]"))
            && let Some(end) = rest.find(']')
        {
            rest = &rest[end + 1..];
            continue;
        }
        if rest.starts_with('<')
            && let Some(end) = rest.find('>')
        {
            let tag = rest[1..end].trim().to_lowercase();
            if matches!(tag.as_str(), "p" | "/p" | "br" | "br/" | "br /") {
                out.push('\n');
            }
            rest = &rest[end + 1..];
            continue;
        }
        let c = rest.chars().next().unwrap();
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .trim()
        .to_owned()
}
#[cfg(test)]
mod artist_bio_tests {
    #[test]
    fn service_markup_becomes_readable_text_without_losing_unicode_or_bracketed_prose() {
        assert_eq!(
            super::biography_text(
                "<p>[wimpLink artistId=\"7\"]bôa[/wimpLink] &amp; music [live]</p><p>Next.</p>"
            ),
            "bôa & music [live]\n\nNext."
        );
        assert_eq!(
            super::biography_text("Text [wimpLink unfinished"),
            "Text [wimpLink unfinished"
        );
    }
}
