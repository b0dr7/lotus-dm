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

    const ERROR_ALREADY_EXISTS: u32 = 183;

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
            crate::core::sys::show_main_window();
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
    let start_hidden = crate::core::settings::load().start_hidden;
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(crate::core::sys::MAIN_WINDOW_TITLE)
        .with_icon(std::sync::Arc::new(icon))
        .with_inner_size([1040.0, 680.0])
        .with_min_inner_size([760.0, 520.0]);
    if start_hidden {
        viewport = viewport.with_position([-32_000.0, -32_000.0]);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "lotus-dm",
        options,
        Box::new(|cc| Ok(Box::new(ui::app::LotusApp::new(cc)) as Box<dyn eframe::App>)),
    )
}