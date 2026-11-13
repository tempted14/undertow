//! A small native floating window. It shares the player process and needs no
//! second egui context, font atlas, HTTP runtime, or application executable.
use super::Snapshot;
use anyhow::{Context, Result};
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use windows::{
    Win32::{
        Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{HiDpi::GetDpiForWindow, WindowsAndMessaging::*},
    },
    core::w,
};

const UPDATE: u32 = WM_APP + 41;

pub struct Handle {
    hwnd: usize,
    snapshot: Arc<Mutex<Snapshot>>,
    alive: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Handle {
    pub fn start(snapshot: Snapshot, visible: bool) -> Result<Self> {
        let shared = Arc::new(Mutex::new(snapshot));
        let snapshot = shared.clone();
        let alive = Arc::new(AtomicBool::new(true));
        let running = alive.clone();
        let (ready, receive) = mpsc::sync_channel(1);
        let thread = std::thread::Builder::new()
            .name("floating-lyrics".into())
            .stack_size(256 * 1024)
            .spawn(move || {
                let result = unsafe { window_thread(snapshot, running.clone(), visible, &ready) };
                if let Err(error) = result {
                    let _ = ready.send(Err(error.to_string()));
                }
                running.store(false, Ordering::Release);
            })?;
        let hwnd = receive
            .recv()
            .context("Lyrics window did not initialize")?
            .map_err(anyhow::Error::msg)?;
        Ok(Self {
            hwnd,
            snapshot: shared,
            alive,
            thread: Some(thread),
        })
    }
    pub fn update(&self, snapshot: Snapshot) {
        if !self.is_alive() {
            return;
        }
        let Ok(mut current) = self.snapshot.try_lock() else {
            return;
        };
        if *current == snapshot {
            return;
        }
        *current = snapshot;
        drop(current);
        unsafe {
            let _ = PostMessageW(
                Some(HWND(self.hwnd as *mut _)),
                UPDATE,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        if self.is_alive() {
            unsafe {
                let _ = PostMessageW(
                    Some(HWND(self.hwnd as *mut _)),
                    WM_CLOSE,
                    WPARAM(0),
                    LPARAM(0),
                );
            }
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct WindowState {
    snapshot: Arc<Mutex<Snapshot>>,
    alive: Arc<AtomicBool>,
    fonts: [HFONT; 3],
    brush: HBRUSH,
    dpi: u32,
    lyric_fonts: OnceLock<Option<crate::lyric_fonts::native::Engine>>,
}
impl WindowState {
    unsafe fn scale_fonts(&mut self, dpi: u32) {
        for font in self.fonts {
            if !font.is_invalid() {
                unsafe {
                    let _ = DeleteObject(font.into());
                }
            }
        }
        self.dpi = dpi.max(96);
        for (slot, (size, weight)) in self.fonts.iter_mut().zip([(11, 400), (24, 600), (8, 400)]) {
            *slot = unsafe {
                CreateFontW(
                    -((size * self.dpi as i32 + 48) / 96),
                    0,
                    0,
                    0,
                    weight,
                    0,
                    0,
                    0,
                    DEFAULT_CHARSET,
                    OUT_DEFAULT_PRECIS,
                    CLIP_DEFAULT_PRECIS,
                    CLEARTYPE_QUALITY,
                    DEFAULT_PITCH.0 as u32,
                    w!("Segoe UI"),
                )
            };
        }
    }
}
impl Drop for WindowState {
    fn drop(&mut self) {
        unsafe {
            for font in self.fonts {
                if !font.is_invalid() {
                    let _ = DeleteObject(font.into());
                }
            }
            let _ = DeleteObject(self.brush.into());
        }
    }
}

unsafe fn window_thread(
    snapshot: Arc<Mutex<Snapshot>>,
    alive: Arc<AtomicBool>,
    visible: bool,
    ready: &mpsc::SyncSender<std::result::Result<usize, String>>,
) -> Result<()> {
    static REGISTER: OnceLock<bool> = OnceLock::new();
    let instance = unsafe { GetModuleHandleW(None)? };
    let registered = *REGISTER.get_or_init(|| unsafe {
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            lpszClassName: w!("UndertowNativeLyrics"),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        }) != 0
    });
    anyhow::ensure!(registered, "Could not register the lyrics window");
    let mut state = Box::new(WindowState {
        snapshot,
        alive,
        fonts: [HFONT::default(); 3],
        brush: unsafe { CreateSolidBrush(rgb([10, 17, 22])) },
        dpi: 96,
        lyric_fonts: OnceLock::new(),
    });
    unsafe {
        state.scale_fonts(96);
    }
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            w!("UndertowNativeLyrics"),
            w!("Undertow Lyrics"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            580,
            195,
            None,
            None,
            Some(instance.into()),
            Some((&mut *state as *mut WindowState).cast()),
        )?
    };
    unsafe {
        state.scale_fonts(GetDpiForWindow(hwnd));
    }
    if visible {
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
    }
    let _ = ready.send(Ok(hwnd.0 as usize));
    let mut message = MSG::default();
    loop {
        let result = unsafe { GetMessageW(&mut message, None, 0, 0) }.0;
        if result <= 0 {
            break;
        }
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    // State stays alive until WM_NCDESTROY has finished; it is never owned by a callback.
    if unsafe { IsWindow(Some(hwnd)) }.as_bool() {
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
    }
    Ok(())
}

fn rgb(color: [u8; 3]) -> COLORREF {
    COLORREF(color[0] as u32 | (color[1] as u32) << 8 | (color[2] as u32) << 16)
}

unsafe fn paint(hwnd: HWND, dc: HDC, state: &WindowState) {
    let mut bounds = RECT::default();
    if unsafe { GetClientRect(hwnd, &mut bounds) }.is_err() {
        return;
    }
    unsafe {
        FillRect(dc, &bounds, state.brush);
        SetBkMode(dc, TRANSPARENT);
    }
    let scale = state.dpi as f32 / 96.;
    let pad = (18. * scale) as i32;
    let snapshot = match state.snapshot.lock() {
        Ok(s) => s.clone(),
        Err(_) => return,
    };
    let mut title = snapshot.title.encode_utf16().collect::<Vec<_>>();
    let mut line = snapshot.line.encode_utf16().collect::<Vec<_>>();
    let mut brand = "UNDERTOW".encode_utf16().collect::<Vec<_>>();
    let old = unsafe { SelectObject(dc, state.fonts[0].into()) };
    let mut rect = RECT {
        left: pad,
        top: pad,
        right: bounds.right - pad,
        bottom: pad + (19. * scale) as i32,
    };
    unsafe {
        SetTextColor(dc, rgb([145, 160, 168]));
        DrawTextW(
            dc,
            &mut title,
            &mut rect,
            DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
    }
    rect.top += (30. * scale) as i32;
    rect.bottom = bounds.bottom - (27. * scale) as i32;
    unsafe {
        SelectObject(dc, state.fonts[1].into());
        SetTextColor(dc, rgb(snapshot.color));
        let painted = state
            .lyric_fonts
            .get_or_init(|| crate::lyric_fonts::native::Engine::new().ok())
            .as_ref()
            .is_some_and(|fonts| {
                fonts
                    .paint_gdi(dc, &snapshot.line, rect, scale, snapshot.color)
                    .is_ok()
            });
        if !painted {
            DrawTextW(dc, &mut line, &mut rect, DT_WORDBREAK | DT_NOPREFIX);
        }
    }
    rect.top = bounds.bottom - (23. * scale) as i32;
    rect.bottom = bounds.bottom - (7. * scale) as i32;
    unsafe {
        SelectObject(dc, state.fonts[2].into());
        SetTextColor(dc, rgb([80; 3]));
        DrawTextW(
            dc,
            &mut brand,
            &mut rect,
            DT_RIGHT | DT_SINGLELINE | DT_NOPREFIX,
        );
        SelectObject(dc, old);
    }
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        let create = unsafe { &*(lp.0 as *const CREATESTRUCTW) };
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
    }
    let state = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut WindowState;
    if !state.is_null() {
        let state = unsafe { &mut *state };
        match msg {
            UPDATE | WM_SIZE => {
                unsafe {
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
                return LRESULT(0);
            }
            WM_ERASEBKGND => return LRESULT(1),
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let dc = unsafe { BeginPaint(hwnd, &mut ps) };
                unsafe {
                    paint(hwnd, dc, state);
                    let _ = EndPaint(hwnd, &ps);
                }
                return LRESULT(0);
            }
            WM_PRINTCLIENT => {
                unsafe {
                    paint(hwnd, HDC(wp.0 as *mut _), state);
                }
                return LRESULT(0);
            }
            WM_GETMINMAXINFO => {
                let info = unsafe { &mut *(lp.0 as *mut MINMAXINFO) };
                info.ptMinTrackSize.x = (300 * state.dpi / 96) as i32;
                info.ptMinTrackSize.y = (120 * state.dpi / 96) as i32;
                return LRESULT(0);
            }
            WM_DPICHANGED => {
                unsafe {
                    state.scale_fonts((wp.0 & 0xffff) as u32);
                }
                let r = unsafe { &*(lp.0 as *const RECT) };
                unsafe {
                    let _ = SetWindowPos(
                        hwnd,
                        None,
                        r.left,
                        r.top,
                        r.right - r.left,
                        r.bottom - r.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
                return LRESULT(0);
            }
            WM_NCHITTEST => {
                let hit = unsafe { DefWindowProcW(hwnd, msg, wp, lp) };
                return if hit.0 == HTCLIENT as isize {
                    LRESULT(HTCAPTION as isize)
                } else {
                    hit
                };
            }
            WM_DESTROY => {
                state.alive.store(false, Ordering::Release);
                unsafe {
                    PostQuitMessage(0);
                }
                return LRESULT(0);
            }
            WM_NCDESTROY => unsafe {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            },
            _ => {}
        }
    }
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hidden_native_lyrics_paints_updates_and_releases_its_window() {
        // This never shows or activates a window, including under the test runner.
        let handle = Handle::start(
            Snapshot {
                title: "Artist · Track".into(),
                line: "日本語 — lyrics & a second line".into(),
                color: [35, 225, 203],
            },
            false,
        )
        .unwrap();
        let hwnd = HWND(handle.hwnd as *mut _);
        assert!(!unsafe { IsWindowVisible(hwnd) }.as_bool());
        let mut pid = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
        }
        assert_eq!(
            pid,
            std::process::id(),
            "Floating lyrics spawned another process"
        );
        handle.update(Snapshot {
            title: "Updated".into(),
            line: "New line".into(),
            color: [255; 3],
        });
        let mut info = BITMAPINFO::default();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = 560;
        info.bmiHeader.biHeight = -155;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        let mut pixels = std::ptr::null_mut();
        let dc = unsafe { CreateCompatibleDC(None) };
        let bitmap =
            unsafe { CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut pixels, None, 0) }
                .unwrap();
        let old = unsafe { SelectObject(dc, bitmap.into()) };
        unsafe {
            SendMessageW(
                hwnd,
                WM_PRINTCLIENT,
                Some(WPARAM(dc.0 as usize)),
                Some(LPARAM(0)),
            );
            let _ = GdiFlush();
        }
        let data = unsafe { std::slice::from_raw_parts(pixels as *const [u8; 4], 560 * 155) };
        let background = [22, 17, 10];
        let background_ok = data[0][..3] == background;
        let ink = data.iter().filter(|p| p[..3] != background).count();
        unsafe {
            SelectObject(dc, old);
            let _ = DeleteObject(bitmap.into());
            let _ = DeleteDC(dc);
        }
        assert!(background_ok);
        assert!(ink > 100, "Floating lyrics did not paint text");
        let alive = handle.alive.clone();
        drop(handle);
        assert!(!alive.load(Ordering::Acquire));
        assert!(!unsafe { IsWindow(Some(hwnd)) }.as_bool());
    }
}
