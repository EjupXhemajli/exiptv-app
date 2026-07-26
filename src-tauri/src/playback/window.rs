//! Natives Kind-Videofenster, in das mpv rendert.
//!
//! Unter Windows: eigenes Child-Window, dessen HWND als `wid` an mpv geht.
//! Position/Größe meldet das Frontend über IPC (Bounds des Platzhalters),
//! damit Video und UI deckungsgleich sind.
//!
//! Wichtig: In windows-sys 0.59 sind alle Handle-Typen (HWND, HINSTANCE,
//! HBRUSH, …) `*mut c_void`. Deshalb werden Null-Handles als
//! `std::ptr::null_mut()` erzeugt und mit `.is_null()` geprüft – niemals
//! als Integer `0`.

#[cfg(target_os = "windows")]
pub use win::VideoWindow;
#[cfg(not(target_os = "windows"))]
pub use stub::VideoWindow;

#[cfg(target_os = "windows")]
mod win {
    use std::ptr::null_mut;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, RECT};
    use windows_sys::Win32::Graphics::Gdi::{
        BeginPaint, EndPaint, FillRect, CreateSolidBrush, DeleteObject, PAINTSTRUCT,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, RegisterClassW, DestroyWindow, SetWindowPos, ShowWindow,
        MoveWindow, GetClientRect, WNDCLASSW, WS_CHILD, WS_VISIBLE, WS_CLIPSIBLINGS,
        WS_CLIPCHILDREN, SW_SHOW, SW_HIDE, HWND_TOP, SWP_NOACTIVATE, WM_PAINT,
        WM_ERASEBKGND, WM_NCHITTEST, CS_HREDRAW, CS_VREDRAW,
    };

    // HTTRANSPARENT signalisiert Windows, dass dieses Fenster für Maus-Treffer
    // "durchlässig" ist – Klicks werden an das darunterliegende Fenster
    // (das WebView mit der Steuerung) weitergereicht.
    const HTTRANSPARENT: LRESULT = -1;

    fn class_name() -> Vec<u16> { "EXIPTVVideoHost\0".encode_utf16().collect() }
    static REGISTER: std::sync::Once = std::sync::Once::new();

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        match msg {
            // Maus-Treffer durchreichen: die HTML-Steuerung im WebView darunter
            // bekommt dadurch die Klicks (Schließen, Vollbild, Kanalwechsel …).
            WM_NCHITTEST => HTTRANSPARENT,
            WM_ERASEBKGND => 1, // wir malen selbst -> kein Flackern
            WM_PAINT => {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut rc);
                // Tiefes Blauschwarz passend zum Design (#060a18) in BGR.
                let brush = CreateSolidBrush(0x00_18_0a_06);
                FillRect(hdc, &rc, brush);
                DeleteObject(brush as *mut core::ffi::c_void);
                EndPaint(hwnd, &ps);
                0
            }
            _ => DefWindowProcW(hwnd, msg, w, l),
        }
    }

    fn ensure_class() {
        REGISTER.call_once(|| unsafe {
            let name = class_name();
            let wc = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(wndproc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: null_mut(),
                hIcon: null_mut(),
                hCursor: null_mut(),
                hbrBackground: null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: name.as_ptr(),
            };
            RegisterClassW(&wc);
        });
    }

    pub struct VideoWindow {
        hwnd: HWND,
        parent: HWND,
    }

    // HWND ist nur ein Zeiger-Handle; alle Zugriffe erfolgen serialisiert
    // aus dem Playback-Thread.
    unsafe impl Send for VideoWindow {}

    impl VideoWindow {
        pub fn new(parent_hwnd: isize) -> Result<Self, String> {
            ensure_class();
            let parent = parent_hwnd as HWND;
            let name = class_name();
            let title: Vec<u16> = "video\0".encode_utf16().collect();
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    name.as_ptr(),
                    title.as_ptr(),
                    WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
                    0, 0, 16, 16,
                    parent,
                    null_mut(),
                    null_mut(),
                    std::ptr::null(),
                )
            };
            if hwnd.is_null() {
                return Err("Videofenster konnte nicht erstellt werden.".into());
            }
            unsafe { ShowWindow(hwnd, SW_SHOW); }
            Ok(Self { hwnd, parent })
        }

        /// HWND als `wid` für mpv (als i64 übergeben).
        pub fn wid(&self) -> i64 { self.hwnd as isize as i64 }

        pub fn set_bounds(&self, x: i32, y: i32, w: i32, h: i32) {
            unsafe {
                MoveWindow(self.hwnd, x, y, w.max(1), h.max(1), 1);
                SetWindowPos(self.hwnd, HWND_TOP, x, y, w.max(1), h.max(1), SWP_NOACTIVATE);
            }
        }

        pub fn show(&self, visible: bool) {
            unsafe { ShowWindow(self.hwnd, if visible { SW_SHOW } else { SW_HIDE }); }
        }

        #[allow(dead_code)]
        pub fn parent(&self) -> isize { self.parent as isize as isize }
    }

    impl Drop for VideoWindow {
        fn drop(&mut self) {
            unsafe { DestroyWindow(self.hwnd); }
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod stub {
    pub struct VideoWindow;
    impl VideoWindow {
        pub fn new(_parent: isize) -> Result<Self, String> {
            Err("Videofenster ist auf dieser Plattform noch nicht verfügbar.".into())
        }
        pub fn wid(&self) -> i64 { 0 }
        pub fn set_bounds(&self, _x: i32, _y: i32, _w: i32, _h: i32) {}
        pub fn show(&self, _visible: bool) {}
        #[allow(dead_code)]
        pub fn parent(&self) -> isize { 0 }
    }
}
