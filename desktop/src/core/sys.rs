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
