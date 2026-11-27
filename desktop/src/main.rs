#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;
mod engine;
mod ui;

use eframe::egui;

#[cfg(windows)]
mod single_instance {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateMutexW(
            lpMutexAttributes: *mut c_void,
            bInitialOwner: i32,
            lpName: *const u16,
        ) -> *mut c_void;
        fn GetLastError() -> u32;
    }

    #[link(name = "user32")]
    extern "system" {
        fn FindWindowW(lpClassName: *const u16, lpWindowName: *const u16) -> *mut c_void;
        fn SetForegroundWindow(hWnd: *mut c_void) -> i32;
        fn ShowWindow(hWnd: *mut c_void, nCmdShow: i32) -> i32;
        fn IsIconic(hWnd: *mut c_void) -> i32;
        fn IsWindowVisible(hWnd: *mut c_void) -> i32;
    }

    const ERROR_ALREADY_EXISTS: u32 = 183;
    const SW_SHOW: i32 = 5;
    const SW_RESTORE: i32 = 9;

    pub fn ensure_single_instance() -> bool {
        let name: Vec<u16> = std::ffi::OsStr::new("LotusDM_SingleInstance_Mutex")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let handle = unsafe { CreateMutexW(std::ptr::null_mut(), 1, name.as_ptr()) };
        if handle.is_null() {
            return true;
        }
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            // Another instance is running: bring its window forward.
            let title: Vec<u16> = std::ffi::OsStr::new("Lotus Download Manager")
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
            if !hwnd.is_null() {
                unsafe {
                    // The running instance may be hiding in the tray.
                    if IsWindowVisible(hwnd) == 0 {
                        ShowWindow(hwnd, SW_SHOW);
                    }
                    if IsIconic(hwnd) != 0 {
                        ShowWindow(hwnd, SW_RESTORE);
                    }
                    SetForegroundWindow(hwnd);
                }
            }
            return false;
        }
        true
    }
}

fn main() -> eframe::Result {
    #[cfg(windows)]
    if !single_instance::ensure_single_instance() {
        return Ok(());
    }

    let icon = egui::IconData {
        rgba: include_bytes!("../assets/lotus-64.rgba").to_vec(),
        width: 64,
        height: 64,
    };
    // "Start hidden in the tray" means no window at launch, only the icon.
    let start_hidden = crate::core::settings::load().start_hidden;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Lotus Download Manager")
            .with_icon(std::sync::Arc::new(icon))
            .with_visible(!start_hidden)
            .with_inner_size([1040.0, 680.0])
            .with_min_inner_size([760.0, 520.0]),
        ..Default::default()
    };

    eframe::run_native(
        "lotus-dm",
        options,
        Box::new(|cc| Ok(Box::new(ui::app::LotusApp::new(cc)) as Box<dyn eframe::App>)),
    )
}