use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const PUBLIC_CLIENT_ID: &str = "G2jghrMon6ybvlDv";
static SETTINGS_SAVE: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn playlist_art_path(id: &str) -> Result<PathBuf> {
    anyhow::ensure!(
        !id.is_empty()
            && id.len() <= 128
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "Invalid playlist identifier"
    );
    Ok(data_dir().join("playlist-art").join(format!("{id}.png")))
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub compatibility: bool,
    pub device_login: bool,
    pub paste_login: bool,
    pub client_id: String,
    pub redirect_uri: String,
    pub scopes: String,
    pub token_broker: String,
    pub country: String,
    pub quality: String,
    pub strict_lossless: bool,
    pub volume: f32,
    pub animated_lyrics: bool,
    pub lyric_glow: bool,
    pub lyric_cover_color: bool,
    pub lyric_motion_blur: bool,
    pub lyric_blur_strength: f32,
    pub lyric_color: [u8; 3],
    pub lyric_size: f32,
    pub cover_theme: bool,
    pub frosted_glass: bool,
    pub effect_strength: f32,
    pub show_sidebar: bool,
    pub sidebar_collapsed: bool,
    pub show_quality: bool,
    pub show_covers: bool,
    pub playlist_background: bool,
    pub everywhere: bool,
    pub floating_lyrics: bool,
    pub no_buffer: bool,
    pub real_max: bool,
    pub font_path: String,
    pub discord: bool,
    pub discord_client_id: String,
    pub translation_language: String,
    pub romanize: bool,
    pub radiant_provider: bool,
    pub downloads: String,
    pub artwork_cache_mb: u64,
    pub fps: u32,
    pub display_sync: bool,
    pub motion_speed: f32,
    pub lyric_offset_ms: i32,
    pub font_enabled: bool,
    pub profile_picture: bool,
    pub profile_name: String,
    pub one_click_lyrics: bool,
    pub copy_lyrics: bool,
    pub cover_fetcher: bool,
    pub song_downloader: bool,
    pub total_play_time: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
        compatibility: true, device_login: true, paste_login: false, client_id: PUBLIC_CLIENT_ID.into(), redirect_uri: "http://127.0.0.1:47832/callback".into(),
        scopes: "user.read collection.read collection.write playlists.read playlists.write search.read".into(), token_broker: String::new(), country: "CA".into(),
        quality: "HI_RES_LOSSLESS".into(), strict_lossless: true, volume: 0.65,
        animated_lyrics: true, lyric_glow: true, lyric_cover_color:false, lyric_motion_blur:true, lyric_blur_strength:0.6, lyric_color: [245, 245, 245], lyric_size: 38.0,
        cover_theme: true, frosted_glass: true, effect_strength: 0.6,
        sidebar_collapsed: false, show_sidebar: true, show_quality: true, show_covers: true, playlist_background: false,
        everywhere: true, floating_lyrics: false, no_buffer: true, real_max: true, font_path: String::new(),
        discord: true, discord_client_id: "1564020404258087134".into(), translation_language: "en".into(), romanize: false, radiant_provider: true,
        downloads: String::new(), artwork_cache_mb: 64, fps: 120, display_sync:true,
        motion_speed:1., lyric_offset_ms:0, font_enabled:true, profile_picture:false, profile_name:String::new(),
        one_click_lyrics:true, copy_lyrics:true, cover_fetcher:true, song_downloader:true, total_play_time:true,
    }
    }
}

pub fn data_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("UNDERTOW_DATA_DIR") {
        return PathBuf::from(p);
    }
    directories::ProjectDirs::from("app", "Undertow", "Undertow")
        .map(|p| p.data_local_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("data"))
}

pub fn read_json<T: serde::de::DeserializeOwned>(p: &Path) -> Result<T> {
    anyhow::ensure!(
        std::fs::metadata(p)?.len() < 16 * 1024 * 1024,
        "Saved data exceeds safety limit"
    );
    Ok(serde_json::from_slice(&std::fs::read(p)?)?)
}

pub fn write_json<T: Serialize>(p: &Path, v: &T) -> Result<()> {
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = p.with_extension("tmp");
    std::fs::write(&temp, serde_json::to_vec_pretty(v)?)?;
    std::fs::rename(temp, p)?;
    Ok(())
}

impl Settings {
    pub fn load() -> Self {
        {
            let mut s: Self = read_json(&data_dir().join("settings.json")).unwrap_or_default();
            if s.lyric_color == [35, 225, 203] {
                s.lyric_color = [245, 245, 245];
            }
            if s.discord_client_id.is_empty() {
                s.discord_client_id = Self::default().discord_client_id;
            }
            s.motion_speed = if s.motion_speed.is_finite() {
                s.motion_speed.clamp(0.5, 2.)
            } else {
                1.
            };
            s.lyric_offset_ms = s.lyric_offset_ms.clamp(-3000, 3000);
            s
        }
    }
    pub fn save(&self) -> Result<()> {
        let _guard = SETTINGS_SAVE
            .lock()
            .map_err(|_| anyhow::anyhow!("Settings save is unavailable"))?;
        write_json(&data_dir().join("settings.json"), self)
    }
    pub fn save_if_open(&self, closing: &std::sync::atomic::AtomicBool) -> Result<()> {
        let _guard = SETTINGS_SAVE
            .lock()
            .map_err(|_| anyhow::anyhow!("Settings save is unavailable"))?;
        if !closing.load(std::sync::atomic::Ordering::Acquire) {
            write_json(&data_dir().join("settings.json"), self)?;
        }
        Ok(())
    }
    pub fn download_dir(&self) -> PathBuf {
        if self.downloads.is_empty() {
            data_dir().join("downloads")
        } else {
            self.downloads.clone().into()
        }
    }
    pub fn effective_font(&self) -> &str {
        if self.font_enabled {
            &self.font_path
        } else {
            ""
        }
    }
}

pub fn safe_filename(s: &str) -> String {
    let s: String = s
        .chars()
        .take(130)
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let s = s.trim().trim_end_matches('.');
    let stem = s.split('.').next().unwrap_or("").to_uppercase();
    if s.is_empty()
        || [
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
            "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
        ]
        .contains(&stem.as_str())
    {
        format!("_{s}")
    } else {
        s.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filenames_cannot_escape_download_folder() {
        assert_eq!(safe_filename("../CON: bad/track?"), ".._CON_ bad_track_");
        assert_eq!(safe_filename("CON.flac"), "_CON.flac");
        assert!(!safe_filename("a\nb").contains('\n'));
    }
    #[test]
    fn shipped_config_has_no_secret_field() {
        let j = serde_json::to_string(&Settings::default()).unwrap();
        assert!(!j.contains("client_secret"));
    }
    #[test]
    fn old_settings_migrate_and_plugin_font_toggle_preserves_the_chosen_file() {
        let mut s: Settings =
            serde_json::from_str(r#"{"font_path":"chosen.ttf","volume":0.4}"#).unwrap();
        assert_eq!(s.effective_font(), "chosen.ttf");
        s.font_enabled = false;
        assert_eq!(s.effective_font(), "");
        assert_eq!(s.font_path, "chosen.ttf");
        assert!(s.song_downloader && s.one_click_lyrics && s.copy_lyrics);
        let round: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(s.motion_speed, round.motion_speed);
        assert_eq!(round.volume, 0.4);
    }
}
