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
