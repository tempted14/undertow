use crate::{auth::now, model::Track};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    time::{Duration, Instant},
};

pub struct Presence {
    pipe: Option<File>,
    client_id: String,
    last: Instant,
    pub status: String,
}
impl Default for Presence {
    fn default() -> Self {
        Self {
            pipe: None,
            client_id: String::new(),
            last: Instant::now() - Duration::from_secs(30),
            status: "Discord application ID needed".into(),
        }
    }
}
pub fn activity(track: &Track, position: f64, duration: f64, paused: bool) -> Value {
    // Discord otherwise falls back to the registered application's client name.
    // Details also drives the compact friends/member-list listening status.
    let artist = if paused {
        if track.artist.is_empty() {
            "Paused".into()
        } else {
            format!("{} · Paused", track.artist)
        }
    } else {
        track.artist.clone()
    };
    let mut a = json!({"name":"Music","type":2,"status_display_type":2,"details":truncate(&track.title,128),"state":truncate(&artist,128),"assets":{"large_text":truncate(&track.album,128)}});
    if let Some(url) = track.cover_url(640).filter(|u| u.starts_with("https://")) {
        a["assets"]["large_image"] = url.into();
    }
    if !paused && duration > 0.0 {
        let start = now().saturating_sub(position as u64);
        a["timestamps"] = json!({"start":start,"end":start+duration as u64});
    }
    a
}
fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}
impl Presence {
    fn connect(&mut self, id: &str) -> Result<()> {
        ensure!(
            id.len() >= 16 && id.len() <= 22 && id.chars().all(|c| c.is_ascii_digit()),
            "Enter a public Discord application ID in Settings"
        );
        #[cfg(windows)]
        for i in 0..10 {
            if let Ok(pipe) = OpenOptions::new()
                .read(true)
                .write(true)
                .open(format!(r"\\.\pipe\discord-ipc-{i}"))
            {
                self.pipe = Some(pipe);
                self.client_id = id.into();
                self.send(0, &json!({"v":1,"client_id":id}))?;
                let (op, reply) = self.receive()?;
                ensure!(
                    op == 1 && reply["evt"] == "READY",
                    "Discord rejected the IPC handshake"
                );
                self.status = "Discord connected".into();
                return Ok(());
            }
        }
        anyhow::bail!("Discord desktop is not running")
    }
    fn send(&mut self, op: u32, v: &Value) -> Result<()> {
        let bytes = serde_json::to_vec(v)?;
        let p = self.pipe.as_mut().context("Discord not connected")?;
        p.write_all(&op.to_le_bytes())?;
        p.write_all(&(bytes.len() as u32).to_le_bytes())?;
        p.write_all(&bytes)?;
        Ok(())
    }
    fn receive(&mut self) -> Result<(u32, Value)> {
        let p = self.pipe.as_mut().context("Discord not connected")?;
        ready(p, 8)?;
        let mut h = [0; 8];
        p.read_exact(&mut h)?;
        let len = u32::from_le_bytes(h[4..].try_into().unwrap()) as usize;
        ensure!(len <= 64 * 1024, "Discord IPC response too large");
        ready(p, len)?;
        let mut b = vec![0; len];
        p.read_exact(&mut b)?;
        Ok((
            u32::from_le_bytes(h[..4].try_into().unwrap()),
            serde_json::from_slice(&b)?,
        ))
    }
    pub fn update(
        &mut self,
        id: &str,
        track: Option<&Track>,
        position: f64,
        duration: f64,
        paused: bool,
        enabled: bool,
    ) -> Result<()> {
        if self.last.elapsed() < Duration::from_secs(5) {
            return Ok(());
        }
        self.last = Instant::now();
        if !enabled {
            if self.pipe.is_some() {
                let _=self.send(1,&json!({"cmd":"SET_ACTIVITY","args":{"pid":std::process::id(),"activity":null},"nonce":crate::auth::random_urlsafe()}));
                self.pipe = None;
            }
            self.status = "Discord disabled".into();
            return Ok(());
        }
        if id.is_empty() {
            self.status = "Discord application ID needed".into();
            return Ok(());
        }
        let result = (|| -> Result<()> {
            if self.pipe.is_none() || self.client_id != id {
                self.connect(id)?;
            }
            let a = track.map(|t| activity(t, position, duration, paused));
            let nonce = crate::auth::random_urlsafe();
            self.send(1,&json!({"cmd":"SET_ACTIVITY","args":{"pid":std::process::id(),"activity":a},"nonce":nonce}))?;
            for _ in 0..8 {
                let (op, v) = self.receive()?;
                if op == 3 {
                    self.send(4, &v)?;
                    continue;
                }
                ensure!(op == 1, "Discord closed the IPC connection");
                if v["nonce"] == nonce {
                    ensure!(
                        v["evt"] != "ERROR",
                        "Discord rejected this application or activity"
                    );
                    self.status = "Rich Presence active".into();
                    return Ok(());
                }
            }
            anyhow::bail!("Discord did not acknowledge activity")
        })();
        if let Err(e) = &result {
            self.status = e.to_string();
            self.pipe = None;
        }
        result
    }
}
impl Drop for Presence {
    fn drop(&mut self) {
        if self.pipe.is_some() {
            let _=self.send(1,&json!({"cmd":"SET_ACTIVITY","args":{"pid":std::process::id(),"activity":null},"nonce":crate::auth::random_urlsafe()}));
        }
    }
}
#[cfg(windows)]
fn ready(file: &File, n: usize) -> Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::{Foundation::HANDLE, System::Pipes::PeekNamedPipe};
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let mut available = 0;
        unsafe {
            PeekNamedPipe(
                HANDLE(file.as_raw_handle()),
                None,
                0,
                None,
                Some(&mut available),
                None,
            )?;
        }
        if available as usize >= n {
            return Ok(());
        }
        ensure!(Instant::now() < deadline, "Discord IPC timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[cfg(not(windows))]
fn ready(_: &File, _: usize) -> Result<()> {
    anyhow::bail!("Discord IPC is Windows-only in this build")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn listening_activity_uses_neutral_name_and_song_status_without_extra_labels() {
        let t = Track {
            id: 42,
            title: "Song title".into(),
            artist: "Artist name".into(),
            album: "Album title".into(),
            cover: "cover-id".into(),
            ..Default::default()
        };
        let a = activity(&t, 30., 180., false);
        assert_eq!(a["name"], "Music");
        assert_eq!(a["status_display_type"], 2);
        assert_eq!(a["details"], "Song title");
        assert_eq!(a["state"], "Artist name");
        assert_eq!(a["assets"]["large_text"], "Album title");
        assert!(
            a["assets"]["large_image"]
                .as_str()
                .unwrap()
                .starts_with("https://resources.tidal.com/")
        );
        assert!(a.get("buttons").is_none());
        assert_eq!(
            a["timestamps"]["end"].as_u64().unwrap() - a["timestamps"]["start"].as_u64().unwrap(),
            180
        );
        let paused = activity(&t, 30., 180., true);
        assert_eq!(paused["state"], "Artist name · Paused");
        assert_eq!(paused["name"], "Music");
        assert!(paused.get("timestamps").is_none());
    }
    #[test]
    fn pause_removes_misleading_timestamps() {
        let t = Track {
            id: 42,
            title: "t".into(),
            ..Default::default()
        };
        assert!(activity(&t, 5.0, 100.0, true).get("timestamps").is_none());
        assert_eq!(activity(&t, 5.0, 100.0, false)["type"], 2);
    }
    #[test]
    fn local_cover_paths_never_leave_the_machine() {
        let t = Track {
            local_path: "C:\\audio.flac".into(),
            cover: "local:C:\\private.jpg".into(),
            ..Default::default()
        };
        assert!(
            activity(&t, 0., 10., false)["assets"]
                .get("large_image")
                .is_none()
        );
    }
}
