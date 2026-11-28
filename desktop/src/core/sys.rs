//! Small OS helpers with no crate dependencies: clipboard text and local
//! wall-clock time. Windows gets the real API; other hosts fall back so the
//! crate still checks on a non-Windows development machine.

#[cfg(windows)]
mod win {
    #[repr(C)]
    #[derive(Default)]
    pub struct SystemTime {
        pub year: u16,
        pub month: u16,
        pub day_of_week: u16,
        pub day: u16,
        pub hour: u16,
        pub minute: u16,
        pub second: u16,
        pub milliseconds: u16,
    }

    #[link(name = "user32")]
    extern "system" {
        pub fn OpenClipboard(hwnd: isize) -> i32;
        pub fn CloseClipboard() -> i32;
        pub fn IsClipboardFormatAvailable(format: u32) -> i32;
        pub fn GetClipboardData(format: u32) -> isize;
    }

    #[link(name = "kernel32")]
    extern "system" {
        pub fn GlobalLock(mem: isize) -> *mut u16;
        pub fn GlobalUnlock(mem: isize) -> i32;
        pub fn GetLocalTime(time: *mut SystemTime);
    }
}

#[cfg(windows)]
fn read_clipboard() -> Option<String> {
    const CF_UNICODETEXT: u32 = 13;
    unsafe {
        if win::OpenClipboard(0) == 0 {
            return None;
        }
        let text = if win::IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 {
            None
        } else {
            let handle = win::GetClipboardData(CF_UNICODETEXT);
            if handle == 0 {
                None
            } else {
                let ptr = win::GlobalLock(handle);
                if ptr.is_null() {
                    None
                } else {
                    let mut len = 0usize;
                    while *ptr.add(len) != 0 {
                        len += 1;
                    }
                    let text = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
                    let _ = win::GlobalUnlock(handle);
                    Some(text)
                }
            }
        };
        win::CloseClipboard();
        text
    }
}

#[cfg(not(windows))]
fn read_clipboard() -> Option<String> {
    None
}

/// Current clipboard contents as text, or None when unavailable.
pub fn clipboard_text() -> Option<String> {
    read_clipboard()
}

#[cfg(windows)]
fn system_time() -> (u8, u8) {
    let mut time = win::SystemTime::default();
    unsafe { win::GetLocalTime(&mut time) };
    ((time.hour % 24) as u8, (time.minute % 60) as u8)
}

#[cfg(not(windows))]
fn system_time() -> (u8, u8) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let day = secs % 86_400;
    (((day / 3600) % 24) as u8, ((day / 60) % 60) as u8)
}

/// Local wall-clock time as (hour, minute), 24h.
pub fn local_hm() -> (u8, u8) {
    system_time()
}

/// Add or remove the per-user Run entry so Lotus starts with Windows.
/// Returns true on success. No-op (returns false) on non-Windows hosts.
pub fn set_autostart(enabled: bool) -> bool {
    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "advapi32")]
        extern "system" {
            fn RegOpenKeyExW(
                hKey: isize,
                lpSubKey: *const u16,
                ulOptions: u32,
                samDesired: u32,
                phkResult: *mut isize,
            ) -> i32;
            fn RegSetValueExW(
                hKey: isize,
                lpValueName: *const u16,
                reserved: u32,
                dwType: u32,
                lpData: *const u8,
                cbData: u32,
            ) -> i32;
            fn RegDeleteValueW(hKey: isize, lpValueName: *const u16) -> i32;
            fn RegCloseKey(hKey: isize) -> i32;
        }
        const HKEY_CURRENT_USER: isize = 0x80000001u32 as isize;
        const KEY_SET_VALUE: u32 = 0x0002;
        const REG_SZ: u32 = 1;
        let subkey: Vec<u16> = OsStr::new("Software\\Microsoft\\Windows\\CurrentVersion\\Run")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let value_name: Vec<u16> = OsStr::new("LotusDM")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            let mut hkey: isize = 0;
            if RegOpenKeyExW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                0,
                KEY_SET_VALUE,
                &mut hkey,
            ) != 0
            {
                return false;
            }
            let ok = if enabled {
                let exe = match std::env::current_exe() {
                    Ok(p) => p,
                    Err(_) => {
                        RegCloseKey(hkey);
                        return false;
                    }
                };
                let cmd = format!("\"{}\"", exe.display());
                let bytes: Vec<u8> = cmd.encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
                RegSetValueExW(
                    hkey,
                    value_name.as_ptr(),
                    0,
                    REG_SZ,
                    bytes.as_ptr(),
                    bytes.len() as u32,
                ) == 0
            } else {
                RegDeleteValueW(hkey, value_name.as_ptr()) == 0 || true
            };
            RegCloseKey(hkey);
            ok
        }
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
        false
    }
}

/// Title of the app window, which is how the Win32 helpers find it.
pub const MAIN_WINDOW_TITLE: &str = "Lotus Download Manager";

#[cfg(windows)]
mod park {
    //! Off-screen parking for the tray window; see [`hide_main_window`].

    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    /// Extended-style index (winuser.h `GWL_EXSTYLE`).
    pub const GWL_EXSTYLE: i32 = -20;
    /// Keeps the window out of the taskbar and Alt+Tab.
    pub const WS_EX_TOOLWINDOW: isize = 0x0000_0080;
    pub const SWP_NOSIZE: u32 = 0x0001;
    pub const SWP_NOZORDER: u32 = 0x0004;
    pub const SWP_NOACTIVATE: u32 = 0x0010;
    pub const SWP_FRAMECHANGED: u32 = 0x0020;
    pub const SW_SHOW: i32 = 5;
    pub const SW_RESTORE: i32 = 9;
    /// Far enough away that no monitor shows it, yet still a visible window.
    pub const PARK_X: i32 = -32_000;
    pub const PARK_Y: i32 = -32_000;

    /// `GetWindowRect` fills all four edges; only the top-left corner is used.
    #[repr(C)]
    #[derive(Default)]
    pub struct Rect {
        pub left: i32,
        pub top: i32,
        #[allow(dead_code)]
        pub right: i32,
        #[allow(dead_code)]
        pub bottom: i32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn FindWindowW(class: *const u16, name: *const u16) -> *mut core::ffi::c_void;
        pub fn GetWindowRect(hwnd: *mut core::ffi::c_void, rect: *mut Rect) -> i32;
        pub fn SetWindowPos(
            hwnd: *mut core::ffi::c_void,
            insert_after: *mut core::ffi::c_void,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            flags: u32,
        ) -> i32;
        pub fn ShowWindow(hwnd: *mut core::ffi::c_void, cmd: i32) -> i32;
        pub fn SetForegroundWindow(hwnd: *mut core::ffi::c_void) -> i32;
    }

    // GetWindowLongPtrW is a real export only on 64-bit Windows; the 32-bit
    // headers alias it to GetWindowLongW, which this build has to call by that
    // name. Both are pointer-sized, so `isize` fits either.
    #[cfg(target_pointer_width = "64")]
    #[link(name = "user32")]
    extern "system" {
        pub fn GetWindowLongPtrW(hwnd: *mut core::ffi::c_void, index: i32) -> isize;
        pub fn SetWindowLongPtrW(hwnd: *mut core::ffi::c_void, index: i32, value: isize) -> isize;
    }
    #[cfg(target_pointer_width = "32")]
    #[link(name = "user32")]
    extern "system" {
        #[link_name = "GetWindowLongW"]
        pub fn GetWindowLongPtrW(hwnd: *mut core::ffi::c_void, index: i32) -> isize;
        #[link_name = "SetWindowLongW"]
        pub fn SetWindowLongPtrW(hwnd: *mut core::ffi::c_void, index: i32, value: isize) -> isize;
    }

    /// The app window, found by title so any thread can ask for it.
    pub fn window() -> Option<*mut core::ffi::c_void> {
        let title: Vec<u16> = OsStr::new(super::MAIN_WINDOW_TITLE)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
        (!hwnd.is_null()).then_some(hwnd)
    }
}

#[cfg(windows)]
static PARKED_FROM: std::sync::Mutex<Option<(i32, i32)>> = std::sync::Mutex::new(None);

/// Move the window out of the way for the tray.
///
/// It is parked, not hidden. Windows sends no `WM_PAINT`, and therefore no
/// `RedrawRequested`, to a window that is invisible or minimised, and eframe
/// runs `App::update` only from redraws (emilk/egui#5229; fixed upstream after
/// the 0.31 release this app uses). A visible window that is merely off-screen
/// keeps painting, so the queue, the retry timers and the finish/fail
/// notifications keep running while the window sits in the tray.
/// `WS_EX_TOOLWINDOW` keeps it out of the taskbar and Alt+Tab so it still
/// looks hidden to the user.
pub fn hide_main_window() {
    #[cfg(windows)]
    unsafe {
        use park::*;

        let Some(hwnd) = park::window() else {
            return;
        };
        let mut rect = Rect::default();
        if GetWindowRect(hwnd, &mut rect) != 0 && rect.left > PARK_X + 1 {
            if let Ok(mut from) = PARKED_FROM.lock() {
                *from = Some((rect.left, rect.top));
            }
        }
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_TOOLWINDOW);
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            PARK_X,
            PARK_Y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}

/// Put the window back where it was and bring it to the front.
pub fn show_main_window() {
    #[cfg(windows)]
    unsafe {
        use park::*;

        let Some(hwnd) = park::window() else {
            return;
        };
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style & !WS_EX_TOOLWINDOW);
        let (x, y) = PARKED_FROM
            .lock()
            .ok()
            .and_then(|from| *from)
            .unwrap_or((120, 80));
        ShowWindow(hwnd, SW_SHOW);
        ShowWindow(hwnd, SW_RESTORE);
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
        );
        SetForegroundWindow(hwnd);
    }
}
