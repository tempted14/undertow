use crate::{
    audio::{self, Prepared},
    auth,
    backend::Tidal,
    config::{Settings, data_dir, safe_filename},
    lyrics::{self, Lyrics},
    model::{Album, Artist, CatalogItem, CreditGroup, Playlist, PlaylistEdit, Track},
};
use eframe::egui;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{self, Receiver, SyncSender},
};

pub mod artwork;
mod catalog;

pub enum Command {
    PickAvatar,
    LoadAvatar,
    PickFont,
    PickPlaylistImage(u64),
    Login(Settings),
    SignOut,
    Settings(Settings),
    Sync,
    Playlist(String),
    Favorites,
    Home,
    DiscoverySection(crate::discovery::Section),
    Mix(String),
    Album(u64),
    Artist(u64),
    CatalogSearch(String),
    Credits(Track),
    Similar(u64),
    Collection(String, bool),
    CollectionState,
    AddMany(String, Arc<Vec<Track>>),
    SavePlaylist(u64, PlaylistEdit),
    Delete(String),
    Add(String, u64),
    Remove(String, usize, Track),
    Move(String, usize, usize, Arc<Vec<Track>>),
    Favorite(u64, bool),
    Prepare(u64, Track),
    Lyrics(u64, u64, Track, bool),
    Translate(u64, u64, Lyrics, String, bool),
    Download(Track),
    DownloadMany(Arc<Vec<Track>>),
    SaveCover(Track),
}
pub enum Event {
    CatalogRead(u64, Box<Event>),
    Avatar(Box<egui::ColorImage>),
    PickedFont(String),
    PlaylistImage(u64, Result<Box<PlaylistImage>, String>),
    PlaylistSaved(u64, Result<(String, bool), String>),
    DeviceCode(auth::DevicePrompt),
    ManualReturn(auth::ManualPrompt),
    Notice(String),
    Error(String),
    Connected(String),
    SignedOut,
    Playlists(Vec<Playlist>),
    Tracks(String, Vec<Track>),
    Home(Result<crate::discovery::Feed, String>),
    Mix(String, Result<Vec<Track>, String>),
    DiscoverySection(String, Result<crate::discovery::Section, String>),
    Catalog(String, Vec<CatalogItem>),
    Credits(u64, Result<Vec<CreditGroup>, String>),
    Similar(u64, Result<Vec<Track>, String>),
    AlbumInfo(Box<Album>),
    Artist(u64, Result<Box<Artist>, String>),
    ArtistPreview(u64, Box<Artist>),
    Collection(String, bool),
    CollectionState(Vec<String>),
    CollectionFailed(String, String),
    Prepared(u64, Box<Track>, Box<Prepared>),
    Lyrics(u64, u64, Result<Lyrics, String>),
    DownloadDone(String),
}
impl Event {
    pub(crate) fn for_catalog_epoch(self, current: u64) -> Option<Self> {
        match self {
            Self::CatalogRead(epoch, event) => (epoch == current).then_some(*event),
            event => Some(event),
        }
    }
}
pub struct ArtData {
    pub cover: egui::ColorImage,
    pub blur: egui::ColorImage,
    pub palette: [u8; 3],
}
pub struct PlaylistImage {
    pub preview: egui::ColorImage,
    pub png: Vec<u8>,
}
pub struct Worker {
    pub settings_closing: Arc<AtomicBool>,
    pub tx: SyncSender<Command>,
    pub rx: Receiver<Event>,
    pub art: artwork::Service,
    pub generation: Arc<AtomicU64>,
    pub download_cancel: Arc<AtomicBool>,
    pub catalog_epoch: Arc<AtomicU64>,
}
fn emit(tx: &SyncSender<Event>, ctx: &egui::Context, e: Event) {
    if tx.send(e).is_ok() {
        ctx.request_repaint();
    }
}

impl Worker {
    pub fn start(settings: Settings, ctx: egui::Context) -> Self {
        let (tx, rx) = mpsc::sync_channel(16);
        let (etx, erx) = mpsc::sync_channel(24);
        let art = artwork::Service::start(ctx.clone(), settings.artwork_cache_mb);
        let generation = Arc::new(AtomicU64::new(0));
        let gen2 = generation.clone();
        let download_cancel = Arc::new(AtomicBool::new(false));
        let cancel_worker = download_cancel.clone();
        let settings_closing = Arc::new(AtomicBool::new(false));
        let closing = settings_closing.clone();
        let catalog_epoch = Arc::new(AtomicU64::new(0));
        let read_epoch = catalog_epoch.clone();
        std::thread::Builder::new().name("tidal-api".into()).spawn(move||{
            let mut reads = catalog::Reader::start(etx.clone(), ctx.clone(), read_epoch);
            let mut settings=Settings::load();
            let mut tidal=auth::load_session().ok().and_then(|s|Tidal::new(s,settings.clone()).ok());
            if let Some(t)=tidal.as_mut(){match t.identify(){Ok(s)=>emit(&etx,&ctx,Event::Connected(s)),Err(e)=>emit(&etx,&ctx,Event::Error(e.to_string()))}}
            while let Ok(cmd)=rx.recv(){
                if matches!(&cmd,Command::SavePlaylist(..)|Command::Collection(..)|Command::AddMany(..)|Command::Delete(..)|Command::Add(..)|Command::Remove(..)|Command::Move(..)|Command::Favorite(..)) {reads.invalidate_library();}
                let cmd=if let Command::Download(t)=cmd{Command::DownloadMany(Arc::new(vec![t]))}else{cmd};let result=(||->anyhow::Result<()>{match cmd {
                Command::PickPlaylistImage(request) => {
                    let events=etx.clone(); let ctx=ctx.clone();
                    std::thread::Builder::new().name("playlist-image".into()).spawn(move || {
                        let result=(||->anyhow::Result<Option<PlaylistImage>> {
                            let Some(path)=pick_file(false)? else { return Ok(None); };
                            playlist_image(&path).map(Some)
                        })();
                        match result {
                            Ok(Some(image)) => emit(&events,&ctx,Event::PlaylistImage(request,Ok(Box::new(image)))),
                            Ok(None) => emit(&events,&ctx,Event::PlaylistImage(request,Err(String::new()))),
                            Err(error) => emit(&events,&ctx,Event::PlaylistImage(request,Err(error.to_string()))),
                        }
                    })?;
                },
                Command::SavePlaylist(request, edit) => {
                    let result=(||->anyhow::Result<(String,bool)> {
                        let t=tidal.as_mut().ok_or_else(||anyhow::anyhow!("Connect your TIDAL account first"))?;
                        let created=edit.id.is_empty();
                        let id=if created {t.create_playlist(&edit.title,&edit.description)?} else {t.edit_playlist(&edit.id,&edit.title,&edit.description)?;edit.id};
                        let mut problems=Vec::new();
                        if let Some(public)=edit.public && let Err(error)=t.playlist_visibility(&id,public) {
                            problems.push(format!("Visibility could not be changed: {error}"));
                        }
                        if let Some(png)=edit.image && let Err(error)=save_playlist_image(&id,&png) {
                            problems.push(format!("Image could not be saved: {error}"));
                        }
                        let warning=!problems.is_empty();
                        let mut message=if created {"Playlist created".to_owned()} else {"Playlist saved".to_owned()};
                        if warning {message.push_str(". ");message.push_str(&problems.join(". "));}
                        Ok((message,warning))
                    })();
                    let saved=result.is_ok();
                    emit(&etx,&ctx,Event::PlaylistSaved(request,result.map_err(|e|e.to_string())));
                    if saved && let Some(t)=tidal.as_mut() {
                        let lists=playlist_covers(t.playlists()?);
                        crate::config::write_json(&data_dir().join("playlists.json"),&lists)?;
                        emit(&etx,&ctx,Event::Playlists(lists));
                    }
                },
                Command::PickAvatar | Command::LoadAvatar | Command::PickFont => {
                    let events=etx.clone();let ctx=ctx.clone();
                    std::thread::Builder::new().name("profile-and-font-files".into()).spawn(move||{
                        let result=(||->anyhow::Result<()>{
                            if matches!(cmd, Command::PickFont) {
                                if let Some(path)=pick_file(true)? { emit(&events,&ctx,Event::PickedFont(path.to_string_lossy().into_owned())); }
                            } else {
                                let picked=matches!(cmd,Command::PickAvatar);
                                let path=if picked { let Some(path)=pick_file(false)? else {return Ok(())}; path } else {data_dir().join("profile.png")};
                                let image=profile_image(&path)?;
                                if picked {
                                    std::fs::create_dir_all(data_dir())?;
                                    let rgba=image.pixels.iter().flat_map(|p|p.to_srgba_unmultiplied()).collect::<Vec<_>>();
                                    let temp=data_dir().join("profile-next.png");
                                    image::save_buffer(&temp,&rgba,image.width() as u32,image.height() as u32,image::ColorType::Rgba8)?;
                                    std::fs::rename(temp,data_dir().join("profile.png"))?;
                                }
                                emit(&events,&ctx,Event::Avatar(Box::new(image)));
                            } Ok(())
                        })();
                        if let Err(error)=result { emit(&events,&ctx,Event::Notice(format!("Profile/font: {error}"))); }
                    })?;
                },
                Command::Settings(s)=>{s.save_if_open(&closing)?;settings=s.clone();if let Some(t)=tidal.as_mut(){t.settings=s;}},
                Command::Login(s)=>{reads.reset();cancel_worker.store(true,Ordering::Relaxed);settings=s.clone();let session=auth::login(&s,|msg|emit(&etx,&ctx,Event::Notice(msg.into())),|prompt|emit(&etx,&ctx,Event::DeviceCode(prompt)),|prompt|emit(&etx,&ctx,Event::ManualReturn(prompt)))?;let mut t=Tidal::new(session,s)?;let status=t.identify()?;tidal=Some(t);emit(&etx,&ctx,Event::Connected(status));},
                Command::SignOut=>{reads.reset();cancel_worker.store(true,Ordering::Relaxed);auth::sign_out()?;tidal=None;emit(&etx,&ctx,Event::SignedOut);},
                read @ (Command::Sync | Command::Favorites | Command::CollectionState | Command::Home | Command::DiscoverySection(_) | Command::Mix(_) | Command::Artist(_) | Command::Album(_) | Command::CatalogSearch(_) | Command::Credits(_) | Command::Similar(_)) => {
                    let t=tidal.as_ref().ok_or_else(||anyhow::anyhow!("Connect your TIDAL account first"))?;
                    reads.submit(t,read);
                },
                Command::Prepare(gen_id,track)=>{
                    if gen_id!=gen2.load(Ordering::Acquire){return Ok(());}
                    let (chosen,plan)=if !track.local_path.is_empty(){(track.clone(),None)}else{let t=tidal.as_mut().ok_or_else(||anyhow::anyhow!("Connect your TIDAL account first"))?;let chosen=if settings.real_max{t.best_version(&track).unwrap_or(track)}else{track};let plan=t.stream(chosen.id,false)?;(chosen,Some(plan))};
                    let events=etx.clone();let ctx=ctx.clone();let latest=gen2.clone();let retry=settings.no_buffer;let strict=settings.strict_lossless;
                    std::thread::Builder::new().name("prepare-audio".into()).spawn(move||{let result=if let Some(plan)=plan{Prepared::remote(plan,retry)}else{Prepared::local(std::path::Path::new(&chosen.local_path))};match result{
                        Ok(mut p)=>{if p.format.duration<=0.0{p.format.duration=chosen.duration as f64;}
if latest.load(Ordering::Acquire)!=gen_id{p.stats.cancel.store(true,Ordering::Relaxed);return;}
if strict&&!p.format.lossless&&chosen.local_path.is_empty(){p.stats.cancel.store(true,Ordering::Relaxed);emit(&events,&ctx,Event::Error("Decoded audio is not verified FLAC; lossless-only mode refused playback".into()));return;}emit(&events,&ctx,Event::Prepared(gen_id,Box::new(chosen),Box::new(p)));},
                        Err(e)=>emit(&events,&ctx,Event::Error(e.to_string())),
                    }})?;
                },
                Command::Lyrics(gen_id,request,track,refresh)=>{
                    if !refresh && let Some(cached)=lyrics::cached(&track) {
                        emit(&etx,&ctx,Event::Lyrics(gen_id,request,Ok(cached)));
                        return Ok(());
                    }
                    let from_tidal=if track.local_path.is_empty(){tidal.as_mut().and_then(|t|t.tidal_lyrics(track.id).ok())}else{None};let radiant=settings.radiant_provider;let events=etx.clone();let ctx=ctx.clone();
                    std::thread::Builder::new().name("lyrics-fetch".into()).spawn(move||{
                        let result=if let Some((lrc,plain))=from_tidal.filter(|(a,b)|!a.is_empty()||!b.is_empty()){
                            let mut l=if lrc.is_empty(){Lyrics{lines:plain.lines().map(|s|lyrics::Line{text:s.into(),..Default::default()}).collect(),..Default::default()}}else{lyrics::parse_lrc(&lrc)};l.source="TIDAL".into();let _=lyrics::save(&track,&l);Ok(l)
                        }else{lyrics::fetch_fresh(&track,None,radiant)};
                        emit(&events,&ctx,Event::Lyrics(gen_id,request,result.map_err(|e|e.to_string())));
                    })?;
                },
                Command::Translate(gen_id,request,l,lang,romanize)=>{let events=etx.clone();let ctx=ctx.clone();std::thread::Builder::new().name("lyrics-translate".into()).spawn(move||emit(&events,&ctx,Event::Lyrics(gen_id,request,lyrics::translate(&l,&lang,romanize).map_err(|e|e.to_string()))))?;},
                Command::DownloadMany(tracks)=>{let t=tidal.as_ref().ok_or_else(||anyhow::anyhow!("Connect TIDAL first"))?;cancel_worker.store(false,Ordering::Relaxed);start_downloads(tracks,t.session.clone(),settings.clone(),etx.clone(),ctx.clone(),cancel_worker.clone())?;},
                Command::SaveCover(track)=>{let url=track.cover_url(1280).ok_or_else(||anyhow::anyhow!("No artwork for this track"))?;let path=settings.download_dir().join(format!("{} - {} cover.jpg",safe_filename(&track.artist),safe_filename(&track.album)));let c=auth::http()?;let r=c.get(url).send()?.error_for_status()?;std::fs::create_dir_all(settings.download_dir())?;let bytes=r.bytes()?;anyhow::ensure!(bytes.len()<=8*1024*1024,"Artwork exceeds 8 MiB");std::fs::write(&path,bytes)?;emit(&etx,&ctx,Event::Notice(format!("Cover saved to {}",path.display())));},
                other=>{let t=tidal.as_mut().ok_or_else(||anyhow::anyhow!("Connect your TIDAL account first"))?;match other {
                    Command::Playlist(id)=>{let tracks=t.playlist_tracks(&id)?;emit(&etx,&ctx,Event::Tracks(id,tracks));},
                    Command::Collection(key,save)=>{ match t.collection_set(&key,save) {
                        Ok(())=>{ emit(&etx,&ctx,Event::Collection(key.clone(),save)); if !key.starts_with("album:") && !key.starts_with("artist:"){emit(&etx,&ctx,Event::Playlists(playlist_covers(t.playlists()?)));} },
                        Err(e)=>emit(&etx,&ctx,Event::CollectionFailed(key,e.to_string())),
                    } },
                    Command::AddMany(id,tracks)=>{ let ids=tracks.iter().filter(|t|t.id>0).map(|t|t.id).collect::<Vec<_>>(); t.add_tracks(&id,&ids)?; emit(&etx,&ctx,Event::Tracks(id.clone(),t.playlist_tracks(&id)?));emit(&etx,&ctx,Event::Playlists(playlist_covers(t.playlists()?)));emit(&etx,&ctx,Event::Notice("Added to playlist".into())); },
                    Command::Delete(id)=>{t.delete_playlist(&id)?;if let Ok(path)=crate::config::playlist_art_path(&id){let _=std::fs::remove_file(path);}emit(&etx,&ctx,Event::Playlists(playlist_covers(t.playlists()?)));emit(&etx,&ctx,Event::Notice("Playlist deleted on TIDAL".into()));},
                    Command::Add(id,track)=>{t.add_track(&id,track)?;emit(&etx,&ctx,Event::Tracks(id.clone(),t.playlist_tracks(&id)?));emit(&etx,&ctx,Event::Playlists(playlist_covers(t.playlists()?)));},
                    Command::Remove(id,index,track)=>{t.remove_track(&id,index,&track)?;emit(&etx,&ctx,Event::Tracks(id.clone(),t.playlist_tracks(&id)?));emit(&etx,&ctx,Event::Playlists(playlist_covers(t.playlists()?)));},
                    Command::Move(id,index,to,tracks)=>{t.move_track(&id,index,to,&tracks)?;emit(&etx,&ctx,Event::Tracks(id.clone(),t.playlist_tracks(&id)?));},
                    Command::Favorite(id,save)=>{t.favorite(id,save)?;emit(&etx,&ctx,Event::Tracks("favorites".into(),t.favorites()?));emit(&etx,&ctx,Event::Notice(if save{"Saved to My tracks"}else{"Removed from My tracks"}.into()));},
                    _=>{}
                }}
            }Ok(())})();if let Err(e)=result{emit(&etx,&ctx,Event::Error(e.to_string()));}}
        }).expect("API worker");
        Self {
            settings_closing,
            tx,
            rx: erx,
            art,
            generation,
            download_cancel,
            catalog_epoch,
        }
    }
}

#[cfg(windows)]
fn pick_file(font: bool) -> anyhow::Result<Option<std::path::PathBuf>> {
    use windows::{
        Win32::UI::Controls::Dialogs::*,
        core::{PCWSTR, PWSTR},
    };
    let filter = if font {
        "Font files (*.ttf;*.otf)\0*.ttf;*.otf\0\0"
    } else {
        "Pictures (*.png;*.jpg;*.jpeg)\0*.png;*.jpg;*.jpeg\0\0"
    };
    let filter = filter.encode_utf16().collect::<Vec<_>>();
    let title = if font {
        "Choose your Undertow font\0"
    } else {
        "Choose your Undertow profile picture\0"
    }
    .encode_utf16()
    .collect::<Vec<_>>();
    let mut path = vec![0u16; 32768];
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrTitle: PCWSTR(title.as_ptr()),
        lpstrFile: PWSTR(path.as_mut_ptr()),
        nMaxFile: path.len() as u32,
        Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR | OFN_EXPLORER,
        ..Default::default()
    };
    if !unsafe { GetOpenFileNameW(&mut dialog) }.as_bool() {
        anyhow::ensure!(
            unsafe { CommDlgExtendedError() } == COMMON_DLG_ERRORS(0),
            "Could not open the file picker"
        );
        return Ok(None);
    }
    use std::os::windows::ffi::OsStringExt;
    let end = path.iter().position(|c| *c == 0).unwrap_or(path.len());
    Ok(Some(std::ffi::OsString::from_wide(&path[..end]).into()))
}
#[cfg(not(windows))]
fn pick_file(_font: bool) -> anyhow::Result<Option<std::path::PathBuf>> {
    anyhow::bail!("The native file chooser is currently supported on Windows")
}

fn profile_image(path: &std::path::Path) -> anyhow::Result<egui::ColorImage> {
    anyhow::ensure!(
        std::fs::metadata(path)?.len() <= 8 * 1024 * 1024,
        "Choose an image smaller than 8 MiB"
    );
    let mut reader = image::ImageReader::open(path)?.with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let cropped = reader
        .decode()?
        .resize_to_fill(256, 256, image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [256, 256],
        cropped.as_raw(),
    ))
}

fn playlist_image(path: &std::path::Path) -> anyhow::Result<PlaylistImage> {
    let preview = profile_image(path)?;
    let rgba = preview
        .pixels
        .iter()
        .flat_map(|p| p.to_srgba_unmultiplied())
        .collect();
    let image = image::RgbaImage::from_raw(256, 256, rgba)
        .ok_or_else(|| anyhow::anyhow!("Invalid image dimensions"))?;
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image).write_to(&mut png, image::ImageFormat::Png)?;
    Ok(PlaylistImage {
        preview,
        png: png.into_inner(),
    })
}
fn save_playlist_image(id: &str, png: &[u8]) -> anyhow::Result<()> {
    anyhow::ensure!(png.len() <= 1024 * 1024, "Playlist image exceeds 1 MiB");
    let path = crate::config::playlist_art_path(id)?;
    std::fs::create_dir_all(path.parent().unwrap())?;
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, png)?;
    std::fs::rename(temp, path)?;
    Ok(())
}
fn playlist_covers(mut playlists: Vec<Playlist>) -> Vec<Playlist> {
    for playlist in &mut playlists {
        if crate::config::playlist_art_path(&playlist.id).is_ok_and(|p| p.is_file()) {
            playlist.image = format!("local-playlist:{}", playlist.id);
        }
    }
    playlists
}

fn decode_art(bytes: Vec<u8>, target_side: u32) -> anyhow::Result<ArtData> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let img = reader
        .decode()?
        .thumbnail(target_side, target_side)
        .to_rgba8();
    let small = image::imageops::resize(&img, 48, 48, image::imageops::FilterType::Triangle);
    let mut blurred = image::imageops::blur(&small, 7.0);
    // Color the tiny cached backdrop once; no per-frame full-resolution blur.
    for pixel in blurred.pixels_mut() {
        let gray = (pixel[0] as f32 + pixel[1] as f32 + pixel[2] as f32) / 3.;
        for channel in 0..3 {
            pixel[channel] =
                ((gray + (pixel[channel] as f32 - gray) * 1.55) * 1.7).clamp(0., 255.) as u8;
        }
    }
    let mut sums = [0u64; 3];
    let mut weight = 0u64;
    for pixel in small.pixels().step_by(4) {
        let max = *pixel.0[..3].iter().max().unwrap();
        let min = *pixel.0[..3].iter().min().unwrap();
        let w = (max - min) as u64 + 1;
        for i in 0..3 {
            sums[i] += pixel[i] as u64 * w;
        }
        weight += w;
    }
    let palette = sums.map(|s| (s / weight.max(1)).min(255) as u8);
    Ok(ArtData {
        cover: egui::ColorImage::from_rgba_unmultiplied(
            [img.width() as usize, img.height() as usize],
            img.as_raw(),
        ),
        blur: egui::ColorImage::from_rgba_unmultiplied([48, 48], blurred.as_raw()),
        palette,
    })
}

fn start_downloads(
    tracks: Arc<Vec<Track>>,
    session: auth::Session,
    settings: Settings,
    events: SyncSender<Event>,
    ctx: egui::Context,
    cancel: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    std::thread::Builder::new()
        .name("download-queue".into())
        .spawn(move || {
            let result = (|| -> anyhow::Result<String> {
                let mut tidal = Tidal::new(session, settings.clone())?;
                let mut saved = 0;
                for (i, track) in tracks.iter().enumerate() {
                    if cancel.load(Ordering::Relaxed) {
                        return Ok(format!("Download queue stopped · {saved} saved"));
                    }
                    if !track.local_path.is_empty() {
                        continue;
                    }
                    emit(
                        &events,
                        &ctx,
                        Event::Notice(format!(
                            "Downloading {} of {} · {}",
                            i + 1,
                            tracks.len(),
                            track.title
                        )),
                    );
                    let track = if settings.real_max {
                        tidal.best_version(track).unwrap_or_else(|_| track.clone())
                    } else {
                        track.clone()
                    };
                    let plan = tidal.stream(track.id, true)?;
                    let ext = if plan.codec.to_lowercase().contains("flac") && plan.urls.len() == 1
                    {
                        "flac"
                    } else {
                        "m4a"
                    };
                    let path = settings.download_dir().join(format!(
                        "{} - {} [{}].{ext}",
                        safe_filename(&track.artist),
                        safe_filename(&track.title),
                        track.id
                    ));
                    if path.exists() {
                        continue;
                    }
                    audio::save_download(plan, &path)?;
                    if ext == "flac" {
                        crate::tags::write_flac(&path, &track, None)?;
                    }
                    saved += 1;
                }
                Ok(format!(
                    "Download queue complete · {saved} saved to {}",
                    settings.download_dir().display()
                ))
            })();
            match result {
                Ok(s) => emit(&events, &ctx, Event::DownloadDone(s)),
                Err(e) => emit(&events, &ctx, Event::Error(format!("Download queue: {e}"))),
            }
        })?;
    Ok(())
}

#[cfg(test)]
mod artwork_memory_tests {
    #[test]
    fn avatar_is_center_cropped_and_bounded_instead_of_retaining_the_source() {
        let path =
            std::env::temp_dir().join(format!("undertow-avatar-{}.png", rand::random::<u64>()));
        let mut source = image::RgbaImage::from_pixel(1024, 256, image::Rgba([255, 0, 0, 255]));
        for y in 0..256 {
            for x in 384..640 {
                source.put_pixel(x, y, image::Rgba([0, 255, 0, 255]));
            }
        }
        source.save(&path).unwrap();
        let image = super::profile_image(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(image.size, [256, 256]);
        assert_eq!(image.pixels[128 * 256 + 128], eframe::egui::Color32::GREEN);
        assert!(image.pixels.iter().filter(|p| p.g() > 200).count() > 64_000);
    }
    #[test]
    fn large_source_decodes_to_requested_thumbnail_without_losing_full_source() {
        let source = image::RgbaImage::from_pixel(750, 750, image::Rgba([20, 80, 160, 255]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(source)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let bytes = bytes.into_inner();
        let small = super::decode_art(bytes.clone(), 80).unwrap();
        let large = super::decode_art(bytes, 640).unwrap();
        assert_eq!(small.cover.size, [80, 80]);
        assert_eq!(large.cover.size, [640, 640]);
        assert_eq!(small.blur.size, [48, 48]);
        assert!(small.cover.pixels.len() * 50 < large.cover.pixels.len());
    }
}
