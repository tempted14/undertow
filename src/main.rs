#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
mod audio;
mod auth;
mod backend;
mod cache;
mod collection;
mod config;
mod discovery;
mod font_data;
mod lyric_fonts;
mod lyrics;
mod metrics;
mod model;
mod overlay;
mod presence;
mod tags;
mod ui;
mod validation;
mod worker;

/// The GPU (glow) renderer is the default: it keeps scrolling and animation on the
/// display's refresh rate. `--software` selects the CPU rasterizer, which hidden
/// background diagnostics also use because they have no visible surface to sync to.
pub(crate) fn software_mode() -> bool {
    static SOFTWARE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *SOFTWARE.get_or_init(|| {
        std::env::args().any(|a| a == "--software")
            || (std::env::args().any(|a| a == "--background-benchmark")
                && !std::env::args().any(|a| a == "--gpu-benchmark"))
    })
}

#[cfg(windows)]
fn set_taskbar_icon(cc: &eframe::CreationContext<'_>) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::{
        Win32::{
            Foundation::{HINSTANCE, HWND, LPARAM, WPARAM},
            System::LibraryLoader::GetModuleHandleW,
            UI::WindowsAndMessaging::{ICON_BIG, LoadIconW, SendMessageW, WM_SETICON},
        },
        core::PCWSTR,
    };
    let Ok(handle) = cc.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(window) = handle.as_raw() else {
        return;
    };
    // egui sets the small window icon. Windows uses the big icon on the taskbar.
    unsafe {
        let resource = PCWSTR(std::ptr::without_provenance(1));
        if let Ok(module) = GetModuleHandleW(PCWSTR::null())
            && let Ok(icon) = LoadIconW(Some(HINSTANCE(module.0)), resource)
        {
            SendMessageW(
                HWND(window.hwnd.get() as *mut _),
                WM_SETICON,
                Some(WPARAM(ICON_BIG as usize)),
                Some(LPARAM(icon.0 as isize)),
            );
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--validate-discovery") {
        if let Some(path) = args.get(2)
            && validation::discovery(std::path::Path::new(path)).is_err()
        {
            std::process::exit(1);
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--validate-artist-artwork") {
        if let Some(path) = args.get(2) {
            let queries = &args[3..];
            if validation::artist_artwork(std::path::Path::new(path), queries).is_err() {
                std::process::exit(1);
            }
        }
        return;
    }
    // Windows/browser launchers can supply a bare URI or prepend their own switches.
    // Recognize returns before the single-instance guard. Never print the URI.
    if let Some(uri) = args
        .iter()
        .skip(1)
        .find(|a| a.starts_with("tidal://login/auth"))
    {
        let callback =
            if args.get(1).map(String::as_str) == Some("--auth-callback") && args.len() == 5 {
                args.clone()
            } else {
                vec![args[0].clone(), "--auth-callback".into(), uri.clone()]
            };
        if auth::relay_callback(&callback).is_err() {
            std::process::exit(1);
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--protocol-selftest-return") {
        if auth::protocol_selftest_return(&args).is_err() {
            std::process::exit(1);
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--protocol-selftest") {
        if let Some(path) = args.get(2)
            && let Err(error) = auth::protocol_selftest(
                std::path::Path::new(path),
                args.iter().any(|a| a == "--browser"),
            )
        {
            let _ = config::write_json(
                std::path::Path::new(path),
                &serde_json::json!({"diagnostic_error":error.to_string(),"credentials_used":false,"tidal_requests_made":false}),
            );
            std::process::exit(1);
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--validate-live") {
        if let Some(path) = args.get(2)
            && validation::run(
                std::path::Path::new(path),
                args.iter().any(|a| a == "--playlist-roundtrip"),
            )
            .is_err()
        {
            std::process::exit(1);
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--validate-artists") {
        if let Some(path) = args.get(2)
            && validation::artists(std::path::Path::new(path)).is_err()
        {
            std::process::exit(1);
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--validate-now-playing") {
        if let Some(path) = args.get(2)
            && validation::now_playing(std::path::Path::new(path)).is_err()
        {
            std::process::exit(1);
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--validate-catalog") {
        if let Some(path) = args.get(2)
            && validation::catalog(std::path::Path::new(path)).is_err()
        {
            std::process::exit(1);
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--validate-refresh") {
        if let Some(path) = args.get(2)
            && validation::refresh(std::path::Path::new(path)).is_err()
        {
            std::process::exit(1);
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--lyrics-overlay") {
        if let Some(pid) = args.get(2).and_then(|p| p.parse().ok()) {
            let _ = overlay::run(pid);
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--auth-callback") {
        if auth::relay_callback(&args).is_err() {
            std::process::exit(1);
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--auth-probe") {
        if let Some(path) = args.get(2) {
            let path = std::path::Path::new(path);
            let settings = config::Settings::load();
            let result = (|| -> anyhow::Result<()> {
                let session = auth::login_with_opener(
                    &settings,
                    |_| {},
                    |u| {
                        config::write_json(
                            path,
                            &serde_json::json!({"authorize_url":u,"status":"waiting"}),
                        )
                    },
                )?;
                let mut tidal = backend::Tidal::new(session, settings)?;
                let status = tidal.identify()?;
                let playlists = tidal.playlists()?;
                config::write_json(
                    path,
                    &serde_json::json!({"status":"authenticated","account_status":status,"playlists":playlists.len()}),
                )?;
                Ok(())
            })();
            if let Err(e) = result {
                let _ = config::write_json(
                    path,
                    &serde_json::json!({"status":"error","error":e.to_string()}),
                );
            }
        }
        return;
    }
    if args.iter().any(|a| a == "--help") {
        println!(
            "Undertow — native TIDAL player\n\n--open PATH       Play a local audio file\n--software        Render on the CPU instead of the GPU\n--lyrics          Open the lyrics view\n--benchmark N     Write a measured report after N seconds and exit\n--report PATH     Benchmark JSON output\n--screenshot PATH Capture the actual application window\n--mute            Mute output (decoder and audio device remain active)\n\nUNDERTOW_DATA_DIR overrides the private data directory. No client secret is accepted or stored by this executable."
        );
        return;
    }
    let background_benchmark = args.iter().any(|a| a == "--background-benchmark");
    if background_benchmark
        && (!args.iter().any(|a| a == "--benchmark") || !args.iter().any(|a| a == "--mute"))
    {
        eprintln!("Background diagnostics require --benchmark and --mute");
        return;
    }
    #[cfg(windows)]
    let _instance = if background_benchmark {
        None
    } else {
        match auth::Instance::acquire() {
            Ok(Some(guard)) => Some(guard),
            Ok(None) => return,
            Err(error) => {
                eprintln!("Cannot acquire application lock: {error}");
                return;
            }
        }
    };
    #[cfg(windows)]
    if !background_benchmark {
        let _ = auth::ProtocolBridge::recover();
    }
    let viewport = eframe::egui::ViewportBuilder::default()
        .with_title("Undertow · TIDAL")
        .with_icon(
            eframe::icon_data::from_png_bytes(include_bytes!("../assets/undertow.png"))
                .expect("Bundled Undertow icon"),
        )
        .with_inner_size(
            args.iter()
                .position(|a| a == "--window-size")
                .and_then(|i| args.get(i + 1))
                .and_then(|s| s.split_once('x'))
                .and_then(|(w, h)| {
                    Some([
                        w.parse::<f32>().ok()?.clamp(640., 3840.),
                        h.parse::<f32>().ok()?.clamp(480., 2160.),
                    ])
                })
                .unwrap_or([1600., 1000.]),
        )
        .with_min_inner_size([640.0, 480.0])
        .with_resizable(true)
        .with_visible(!background_benchmark)
        .with_active(!background_benchmark)
        .with_decorations(false);
    if software_mode() {
        let config = egui_software_backend::SoftwareBackendAppConfiguration::new()
            .viewport_builder(viewport);
        if let Err(e) = egui_software_backend::run_app_with_software_backend(config, move |ctx| {
            ui::App::new(&ctx, &args)
        }) {
            eprintln!("Undertow failed to start: {e}");
            std::process::exit(1);
        }
        return;
    }
    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        vsync: true,
        multisampling: 0,
        depth_buffer: 0,
        stencil_buffer: 0,
        ..Default::default()
    };
    if let Err(e) = eframe::run_native(
        "Undertow",
        options,
        Box::new(move |cc| {
            #[cfg(windows)]
            set_taskbar_icon(cc);
            Ok(Box::new(ui::App::new(&cc.egui_ctx, &args)))
        }),
    ) {
        eprintln!("Undertow failed to start: {e}");
        std::process::exit(1);
    }
}
