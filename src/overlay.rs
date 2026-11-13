#[cfg(windows)]
mod native;
#[cfg(windows)]
pub use native::Handle;

use eframe::egui::{self, Align2, Color32, FontId, Frame};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub title: String,
    pub line: String,
    pub color: [u8; 3],
}
struct Overlay {
    parent: u32,
    snapshot: Snapshot,
}
impl egui_software_backend::App for Overlay {
    fn update(
        &mut self,
        ctx: &egui::Context,
        backend: &mut egui_software_backend::SoftwareBackend,
    ) {
        backend.set_frame_rate_limit(10);
        if !parent_alive(self.parent) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        let path = crate::config::data_dir().join(format!("overlay-{}.json", self.parent));
        if let Ok(s) = crate::config::read_json(&path) {
            self.snapshot = s;
        }
        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(Color32::from_rgb(10, 17, 22))
                    .inner_margin(18),
            )
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(&self.snapshot.title)
                        .size(11.)
                        .color(Color32::from_rgb(145, 160, 168)),
                );
                ui.add_space(12.);
                ui.label(
                    egui::RichText::new(&self.snapshot.line)
                        .size(24.)
                        .strong()
                        .color(Color32::from_rgb(
                            self.snapshot.color[0],
                            self.snapshot.color[1],
                            self.snapshot.color[2],
                        )),
                );
                let r = ui.max_rect();
                if ui
                    .interact(r, egui::Id::new("overlay-drag"), egui::Sense::drag())
                    .drag_started()
                {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                ui.painter().text(
                    r.right_bottom(),
                    Align2::RIGHT_BOTTOM,
                    "UNDERTOW",
                    FontId::proportional(8.),
                    Color32::from_gray(80),
                );
            });
        ctx.request_repaint_after(Duration::from_millis(200));
    }
}
pub fn run(parent: u32) -> anyhow::Result<()> {
    let cfg = egui_software_backend::SoftwareBackendAppConfiguration::new().viewport_builder(
        egui::ViewportBuilder::default()
            .with_title("Undertow Lyrics")
            .with_inner_size([560., 155.])
            .with_min_inner_size([300., 120.])
            .with_window_level(egui::WindowLevel::AlwaysOnTop)
            .with_decorations(true),
    );
    egui_software_backend::run_app_with_software_backend(cfg, move |ctx| {
        crate::ui::load_font(&ctx, "");
        Overlay {
            parent,
            snapshot: Snapshot::default(),
        }
    })
    .map_err(|e| anyhow::anyhow!("Lyrics window failed: {e}"))?;
    Ok(())
}
#[cfg(windows)]
fn parent_alive(pid: u32) -> bool {
    use windows::Win32::{
        Foundation::{CloseHandle, WAIT_TIMEOUT},
        System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
    };
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) else {
            return false;
        };
        let alive = WaitForSingleObject(h, 0) == WAIT_TIMEOUT;
        let _ = CloseHandle(h);
        alive
    }
}
#[cfg(not(windows))]
fn parent_alive(_: u32) -> bool {
    true
}

#[cfg(not(windows))]
pub struct Handle {
    child: std::sync::Mutex<std::process::Child>,
}
#[cfg(not(windows))]
impl Handle {
    pub fn start(snapshot: Snapshot, _visible: bool) -> anyhow::Result<Self> {
        let path = crate::config::data_dir().join(format!("overlay-{}.json", std::process::id()));
        crate::config::write_json(&path, &snapshot)?;
        let child = std::process::Command::new(std::env::current_exe()?)
            .arg("--lyrics-overlay")
            .arg(std::process::id().to_string())
            .spawn()?;
        Ok(Self {
            child: std::sync::Mutex::new(child),
        })
    }
    pub fn update(&self, snapshot: Snapshot) {
        let _ = crate::config::write_json(
            &crate::config::data_dir().join(format!("overlay-{}.json", std::process::id())),
            &snapshot,
        );
    }
    pub fn is_alive(&self) -> bool {
        self.child
            .lock()
            .ok()
            .is_some_and(|mut c| c.try_wait().ok().flatten().is_none())
    }
}
#[cfg(not(windows))]
impl Drop for Handle {
    fn drop(&mut self) {
        if let Ok(mut c) = self.child.lock() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}
