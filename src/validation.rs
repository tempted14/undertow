//! Explicit, user-invoked live diagnostics. Never serializes account identity or credentials.
use crate::{auth, backend::Tidal, config, lyrics, model::Track};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{path::Path, time::Duration};

pub fn discovery(path: &Path) -> Result<()> {
    let result = (|| -> Result<Value> {
        let mut tidal = Tidal::new(auth::load_session()?, config::Settings::load())?;
        let feed = tidal.home().context("Home feed")?;
        let section = feed
            .sections
            .iter()
            .find(|s| s.id == "ALBUM_RECOMMENDATIONS")
            .or_else(|| {
                feed.sections
                    .iter()
                    .find(|s| !s.shortcuts && s.more.is_some())
            })
            .context("No expandable recommendation section")?;
        let expanded = tidal
            .discovery_section(section)
            .context("Expanded albums")?;
        ensure!(!expanded.items.is_empty(), "Empty expanded recommendations");
        let mix = feed
            .sections
            .iter()
            .flat_map(|s| s.items.iter())
            .find(|i| i.catalog.kind == "Mix")
            .context("No recommended mixes")?;
        let tracks = tidal.mix_tracks(&mix.catalog.id).context("Mix tracks")?;
        ensure!(
            !tracks.is_empty() && tracks.iter().all(|t| t.id > 0),
            "No playable mix tracks"
        );
        let playlist = feed
            .sections
            .iter()
            .filter(|s| !s.shortcuts)
            .flat_map(|s| s.items.iter())
            .find(|i| i.catalog.kind == "Playlist")
            .context("No suggested playlists")?;
        let playlist_tracks = tidal
            .playlist_tracks(&playlist.catalog.id)
            .context("Suggested playlist")?;
        ensure!(!playlist_tracks.is_empty(), "Empty recommended playlist");
        Ok(
            json!({"personalized":feed.personalized,"sections":feed.sections.iter().map(|s|json!({"id":s.id,"title":s.title,"items":s.items.len()})).collect::<Vec<_>>(),
            "expanded_section":section.id,"expanded_items":expanded.items.len(),"mix_tracks":tracks.len(),"playlist_tracks":playlist_tracks.len(),
            "credentials_serialized":false,"service_writes":false,"windows_opened":false}),
        )
    })();
    config::write_json(
        path,
        &match &result {
            Ok(value) => value.clone(),
            Err(error) => json!({"error":format!("{error:#}")}),
        },
    )?;
    result.map(|_| ())
}

pub fn now_playing(path: &Path) -> Result<()> {
    let result = (|| -> Result<Value> {
        let mut tidal = Tidal::new(auth::load_session()?, config::Settings::load())?;
        let tracks = tidal.search("Duvet")?;
        let track = tracks
            .iter()
            .find(|t| {
                t.title.eq_ignore_ascii_case("Duvet") && t.artist.to_lowercase().contains("bôa")
            })
            .or_else(|| tracks.first())
            .context("No reference track")?;
        let credits = tidal.credits(track)?;
        let related = tidal.similar_tracks(track.id)?;
        ensure!(!credits.is_empty(), "No credits returned");
        ensure!(!related.is_empty(), "No similar tracks returned");
        Ok(
            json!({"track_id":track.id,"title":track.title,"artist":track.artist,"album_id":track.album_id,"credits":credits,"related_count":related.len(),"favorites_with_dates":tidal.favorites()?.iter().filter(|t|!t.date_added.is_empty()).count(),"passed":true}),
        )
    })();
    config::write_json(
        path,
        &result
            .as_ref()
            .cloned()
            .unwrap_or_else(|e| json!({"passed":false,"error":e.to_string()})),
    )?;
    result.map(|_| ())
}

pub fn refresh(path: &Path) -> Result<()> {
    let result = (|| -> Result<()> {
        let settings = config::Settings::load();
        let mut session = auth::load_session()?;
        let prior_client = session.client_id.clone();
        auth::refresh_now(&mut session, &settings)?;
        ensure!(
            session.client_id == prior_client,
            "Refresh changed the authorized client"
        );
        let mut tidal = Tidal::new(session, settings)?;
        tidal.identify()?;
        Ok(())
    })();
    config::write_json(
        path,
        &json!({"refresh_verified":result.is_ok(),"error":result.as_ref().err().map(|e|e.to_string())}),
    )?;
    result
}

pub fn catalog(path: &Path) -> Result<()> {
    let settings = config::Settings::load();
    let mut tidal = Tidal::new(auth::load_session()?, settings)?;
    let mut report = json!({"authenticated":false});
    let result = (|| -> Result<()> {
        tidal.identify()?;
        report["authenticated"] = json!(true);
        let items = tidal.catalog_search("Steve")?;
        for kind in ["Artist", "Album", "Track", "Playlist"] {
            let count = items.iter().filter(|i| i.kind == kind).count();
            ensure!(count > 0, "No {kind} results");
            report[kind] = json!(count);
        }
        let album = items
            .iter()
            .find(|i| i.kind == "Album" && i.title.eq_ignore_ascii_case("Oh yeah?"))
            .context("Reference album not returned")?;
        let id = album.id.parse()?;
        let info = tidal.album_info(id)?;
        let tracks = tidal.album_tracks(id)?;
        ensure!(
            !info.cover.is_empty() && !info.artist.is_empty() && !tracks.is_empty(),
            "Album metadata incomplete"
        );
        report["reference_album"] = json!({"id":id,"title":info.title,"artist":info.artist,"year":info.year,"track_count":tracks.len(),"duration_seconds":tracks.iter().map(|t|t.duration).sum::<u64>(),"cover_present":!info.cover.is_empty(),"artist_picture_present":!info.artist_picture.is_empty()});
        Ok(())
    })();
    report["passed"] = json!(result.is_ok());
    report["error"] = json!(result.as_ref().err().map(|e| e.to_string()));
    config::write_json(path, &report)?;
    result
}

pub fn run(path: &Path, playlist_writes: bool) -> Result<()> {
    let settings = config::Settings::load();
    let mut tidal = Tidal::new(auth::load_session()?, settings)?;
    let mut report = json!({"authenticated":false,"all_16_live_validated":false});
    let result = (|| -> Result<()> {
        tidal.identify()?;
        report["authenticated"] = json!(true);
        let lists = tidal.playlists()?;
        report["playlist_count"] = json!(lists.len());
        config::write_json(&config::data_dir().join("playlists.json"), &lists)?;
        let mut tracks = tidal.favorites()?;
        report["favorite_count"] = json!(tracks.len());
        if tracks.len() < 2 {
            for playlist in lists.iter().take(3) {
                tracks.extend(tidal.playlist_tracks(&playlist.id)?);
                if tracks.len() >= 2 {
                    break;
                }
            }
        }
        tracks.retain(|t| t.id > 0);
        let track = tracks
            .first()
            .context("No saved or playlist tracks available for live playback validation")?
            .clone();
        if playlist_writes {
            report["playlist_roundtrip"] = playlist_roundtrip(&mut tidal, &tracks);
        }
        let plan = tidal.stream(track.id, false)?;
        let prepared = crate::audio::Prepared::remote(plan, true)?;
        ensure!(prepared.format.lossless, "TIDAL returned a non-FLAC codec");
        let audio = crate::audio::Audio::start(prepared, 0.0)?;
        std::thread::sleep(Duration::from_secs(3));
        report["stream"] = json!({"track_id":track.id,"codec":audio.format.codec,"bits":audio.format.bit_depth,"sample_rate":audio.format.sample_rate,"output_rate":audio.format.output_rate,"played_seconds":audio.position(),"underrun_samples":audio.stats.underrun_samples.load(std::sync::atomic::Ordering::Relaxed),"muted":true});
        ensure!(
            audio.position() > 1.0,
            "No decoded TIDAL audio was consumed by the output device"
        );
        audio.sink.pause();
        audio.seek(10.0);
        audio.sink.play();
        std::thread::sleep(Duration::from_secs(1));
        report["seek_verified"] = json!(audio.position() >= 10.5);
        audio.sink.pause();
        let fetched = lyrics::fetch(&track, Some(&mut tidal));
        match fetched {
            Ok(l) => {
                report["lyrics"] =
                    json!({"source":l.source,"lines":l.lines.len(),"synced":l.synced});
                report["translation"] = match lyrics::translate(&l, "es", false) {
                    Ok(translated) => json!({"succeeded":true,"lines":translated.lines.len()}),
                    Err(e) => json!({"succeeded":false,"error":e.to_string()}),
                };
            }
            Err(e) => report["lyrics"] = json!({"error":e.to_string()}),
        }
        report["offline_manifest"] = match tidal.stream(track.id, true) {
            Ok(plan) => {
                json!({"authorized":true,"segments":plan.urls.len(),"note":"Manifest access only; full offline download has not been verified."})
            }
            Err(e) => json!({"authorized":false,"error":e.to_string()}),
        };
        Ok(())
    })();
    if let Err(e) = &result {
        report["error"] = json!(e.to_string());
    }
    config::write_json(path, &report)?;
    result
}

fn playlist_roundtrip(tidal: &mut Tidal, tracks: &[Track]) -> Value {
    let title = format!("Undertow validation {:016x}", rand::random::<u64>());
    let description = "Temporary playlist created by Undertow's explicit live validation command.";
    let mut report = json!({"temporary_title":title,"created":false,"deleted":false});
    if let Err(e) = tidal.create_playlist(&title, description) {
        report["error"] = json!(e.to_string());
        return report;
    }
    report["created"] = json!(true);
    let found = (0..5).find_map(|_| {
        let found = tidal
            .playlists()
            .ok()?
            .into_iter()
            .find(|p| p.title == title);
        if found.is_none() {
            std::thread::sleep(Duration::from_millis(500));
        }
        found
    });
    let Some(playlist) = found else {
        report["error"] = json!(
            "Created playlist is not yet visible; remove only the exact temporary title if it appears later."
        );
        return report;
    };
    let id = playlist.id;
    let result = (|| -> Result<()> {
        let edited_title = format!("{title} edited");
        tidal.edit_playlist(&id, &edited_title, "Updated validation description")?;
        let mut independent = Tidal::new(tidal.session.clone(), tidal.settings.clone())?;
        let edited = independent
            .playlists()?
            .into_iter()
            .find(|p| p.id == id)
            .context("Edited playlist is not visible on an independent request")?;
        ensure!(
            edited.title == edited_title && edited.description == "Updated validation description",
            "Playlist metadata did not round trip"
        );
        report["metadata_roundtrip"] = json!(true);
        let first = tracks.first().context("No track for playlist validation")?;
        tidal.playlist_tracks(&id)?;
        tidal.add_track(&id, first.id)?;
        let rows = tidal.playlist_tracks(&id)?;
        ensure!(
            rows.len() == 1 && rows[0].id == first.id,
            "Added track did not round trip"
        );
        report["add_roundtrip"] = json!(true);
        if let Some(second) = tracks.iter().find(|t| t.id != first.id) {
            tidal.add_track(&id, second.id)?;
            let rows = tidal.playlist_tracks(&id)?;
            ensure!(rows.len() == 2, "Second track did not round trip");
            tidal.move_track(&id, 1, 0, &rows)?;
            let rows = tidal.playlist_tracks(&id)?;
            ensure!(
                rows[0].id == second.id,
                "Reordered tracks did not round trip"
            );
            report["reorder_roundtrip"] = json!(true);
        }
        let rows = tidal.playlist_tracks(&id)?;
        tidal.remove_track(&id, 0, &rows[0])?;
        let remaining = tidal.playlist_tracks(&id)?;
        ensure!(
            remaining.len() + 1 == rows.len(),
            "Removed track did not round trip"
        );
        report["remove_roundtrip"] = json!(true);
        Ok(())
    })();
    if let Err(e) = result {
        report["error"] = json!(e.to_string());
    }
    match tidal.delete_playlist(&id) {
        Ok(()) => {
            report["deleted"] = json!(true);
            match tidal.playlists() {
                Ok(lists) => {
                    report["deletion_roundtrip"] = json!(!lists.iter().any(|p| p.id == id))
                }
                Err(e) => report["cleanup_error"] = json!(e.to_string()),
            }
        }
        Err(e) => report["cleanup_error"] = json!(e.to_string()),
    }
    report
}

pub fn artist_artwork(path: &Path, queries: &[String]) -> Result<()> {
    use std::io::Read;
    let result = (|| -> Result<Value> {
        let mut tidal = Tidal::new(auth::load_session()?, config::Settings::load())?;
        let client = auth::http()?;
        let mut reports = Vec::new();
        for query in queries {
            let found = tidal
                .catalog_search(query)?
                .into_iter()
                .find(|i| i.kind == "Artist" && i.title.eq_ignore_ascii_case(query))
                .context("Exact artist search result missing")?;
            let from_hint = tidal.artist_header(found.id.parse()?)?;
            let mut direct = Tidal::new(tidal.session.clone(), tidal.settings.clone())?;
            let artist = direct.artist_header(found.id.parse()?)?;
            ensure!(!artist.picture.is_empty(), "Artist portrait missing");
            let mut variants = Vec::new();
            for url in [
                artist.picture.clone(),
                artist.picture.replace("750x750.jpg", "640x640.jpg"),
            ] {
                if url.is_empty() {
                    continue;
                }
                let response = client
                    .get(&url)
                    .timeout(std::time::Duration::from_secs(12))
                    .send()?;
                let status = response.status().as_u16();
                let mut bytes = Vec::new();
                response.take(8 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
                let dimensions = image::ImageReader::new(std::io::Cursor::new(&bytes))
                    .with_guessed_format()?
                    .into_dimensions()
                    .ok();
                variants.push(
                    json!({"url":url,"status":status,"bytes":bytes.len(),"dimensions":dimensions}),
                );
            }
            reports.push(json!({"id":artist.id,"name":artist.name,"search_picture":found.cover,"header_picture":artist.picture,"hint_matches_direct":from_hint.picture==artist.picture,"fans":artist.fans,"variants":variants}));
        }
        Ok(
            json!({"passed":true,"artists":reports,"scope":"Read-only metadata and public CDN image requests. No playback or library writes."}),
        )
    })();
    config::write_json(
        path,
        &result
            .as_ref()
            .cloned()
            .unwrap_or_else(|e| json!({"passed":false,"error":e.to_string()})),
    )?;
    result.map(|_| ())
}

pub fn artists(path: &Path) -> Result<()> {
    let result = (|| -> Result<Value> {
        let mut tidal = Tidal::new(auth::load_session()?, config::Settings::load())?;
        let mut reports = Vec::new();
        for query in ["Steve Lacy", "bôa", "Yeat"] {
            let found = tidal
                .catalog_search(query)?
                .into_iter()
                .find(|i| i.kind == "Artist" && i.title.eq_ignore_ascii_case(query))
                .context("Exact artist search result missing")?;
            let artist = tidal.artist(found.id.parse()?)?;
            ensure!(!artist.tracks.is_empty(), "Artist top tracks missing");
            ensure!(!artist.albums.is_empty(), "Artist albums missing");
            ensure!(
                artist.tracks.iter().all(|t| t.artist_id > 0),
                "Artist identifiers missing from tracks"
            );
            reports.push(json!({"id":artist.id,"name":artist.name,"top_tracks":artist.tracks.len(),"albums":artist.albums.len(),"singles":artist.singles.len(),"biography_available":!artist.biography.is_empty(),"notices":artist.notices}));
        }
        Ok(json!({"passed":true,"artists":reports}))
    })();
    config::write_json(
        path,
        &result
            .as_ref()
            .cloned()
            .unwrap_or_else(|e| json!({"passed":false,"error":e.to_string()})),
    )?;
    result.map(|_| ())
}
