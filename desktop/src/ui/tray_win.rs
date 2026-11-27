//! Windows tray icon, context menu and balloon notifications.
//!
//! Hand-written Win32 on purpose: no extra crates, and the rest of the app
//! already talks to the OS this way (see `core::sys`). The window procedure
//! runs on the main thread because winit pumps the message queue, so the
//! action queue in `ui::tray` is only ever touched from two threads with a
//! lock held.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{LazyLock, Mutex};

use eframe::egui;

use crate::ui::tray::{self, TrayAction};

const WM_APP_CALLBACK: u32 = 0x8001;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_CONTEXTMENU: u32 = 0x007B;
const WM_NULL: u32 = 0x0000;

const NIM_ADD: u32 = 0;
const NIM_MODIFY: u32 = 1;
const NIM_DELETE: u32 = 2;
const NIF_MESSAGE: u32 = 0x1;
const NIF_ICON: u32 = 0x2;
const NIF_TIP: u32 = 0x4;
const NIF_INFO: u32 = 0x10;
const NIIF_INFO: u32 = 1;
const NIIF_ERROR: u32 = 3;

const IMAGE_ICON: u32 = 1;
const LR_LOADFROMFILE: u32 = 0x0010;

const MF_STRING: u32 = 0x0;
const MF_CHECKED: u32 = 0x8;
const MF_SEPARATOR: u32 = 0x800;
const MF_POPUP: u32 = 0x10;

const TPM_RETURNCMD: u32 = 0x0100;
const TPM_RIGHTBUTTON: u32 = 0x0002;

const TRAY_ID: u32 = 1;
const ID_OPEN: usize = 100;
const ID_PAUSE: usize = 101;
const ID_RESUME: usize = 102;
const ID_QUIT: usize = 110;
const ID_SPEED: usize = 200;

#[repr(C)]
struct WndClassW {
    style: u32,
    wnd_proc: Option<unsafe extern "system" fn(isize, u32, usize, isize) -> isize>,
    cls_extra: i32,
    wnd_extra: i32,
    instance: isize,
    icon: isize,
    cursor: isize,
    background: isize,
    menu_name: *const u16,
    class_name: *const u16,
}

#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}

/// NOTIFYICONDATAW as of Vista and later: 976 bytes on x64, 956 on x86.
#[repr(C)]
struct NotifyIconData {
    cb_size: u32,
    hwnd: isize,
    id: u32,
    flags: u32,
    callback_message: u32,
    icon: isize,
    tip: [u16; 128],
    state: u32,
    state_mask: u32,
    info: [u16; 256],
    timeout: u32,
    info_title: [u16; 64],
    info_flags: u32,
    guid: [u8; 16],
    balloon_icon: isize,
}

impl NotifyIconData {
    fn zeroed() -> Self {
        // Plain old data: an all-zero struct is a valid, empty value.
        unsafe { std::mem::zeroed() }
    }
}

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> isize;
}

#[link(name = "user32")]
extern "system" {
    fn RegisterClassW(class: *const WndClassW) -> u16;
    fn CreateWindowExW(
        ex_style: u32,
        class: *const u16,
        name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: isize,
        menu: isize,
        instance: isize,
        param: *mut core::ffi::c_void,
    ) -> isize;
    fn DefWindowProcW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
    fn DestroyWindow(hwnd: isize) -> i32;
    fn DestroyIcon(icon: isize) -> i32;
    fn LoadImageW(
        instance: isize,
        name: *const u16,
        kind: u32,
        cx: i32,
        cy: i32,
        flags: u32,
    ) -> isize;
    fn CreatePopupMenu() -> isize;
    fn AppendMenuW(menu: isize, flags: u32, id: usize, item: *const u16) -> i32;
    fn TrackPopupMenu(
        menu: isize,
        flags: u32,
        x: i32,
        y: i32,
        reserved: i32,
        hwnd: isize,
        rect: *const core::ffi::c_void,
    ) -> i32;
    fn DestroyMenu(menu: isize) -> i32;
    fn GetCursorPos(point: *mut Point) -> i32;
    fn SetForegroundWindow(hwnd: *mut core::ffi::c_void) -> i32;
    fn PostMessageW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> i32;
}

#[link(name = "shell32")]
extern "system" {
    fn Shell_NotifyIconW(message: u32, data: *const NotifyIconData) -> i32;
}

static REPAINT: LazyLock<Mutex<Option<egui::Context>>> = LazyLock::new(|| Mutex::new(None));
static HWND: AtomicIsize = AtomicIsize::new(0);

fn wide(text: &str) -> Vec<u16> {
    OsStr::new(text).encode_wide().chain(Some(0)).collect()
}

fn wide_os(text: &OsStr) -> Vec<u16> {
    text.encode_wide().chain(Some(0)).collect()
}

fn fill_text(dst: &mut [u16], text: &str) {
    let mut index = 0;
    for unit in text.encode_utf16() {
        if index + 1 >= dst.len() {
            break;
        }
        dst[index] = unit;
        index += 1;
    }
    dst[index] = 0;
}

fn wake() {
    if let Ok(slot) = REPAINT.lock() {
        if let Some(ctx) = slot.as_ref() {
            ctx.request_repaint();
        }
    }
}

unsafe fn add_item(menu: isize, id: usize, label: &str) {
    let text = wide(label);
    AppendMenuW(menu, MF_STRING, id, text.as_ptr());
}

unsafe fn add_separator(menu: isize) {
    AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
}

unsafe fn show_menu(hwnd: isize) {
    let menu = CreatePopupMenu();
    if menu == 0 {
        return;
    }
    add_item(menu, ID_OPEN, "Open Lotus Download Manager");
    add_separator(menu);
    add_item(menu, ID_PAUSE, "Pause all");
    add_item(menu, ID_RESUME, "Resume all");
    add_separator(menu);

    let speed_menu = CreatePopupMenu();
    let current = tray::speed_hint();
    for (index, preset) in tray::SPEED_PRESETS.iter().enumerate() {
        let label = if *preset == 0 {
            "Unlimited".to_string()
        } else {
            format!("{preset} KB/s")
        };
        let mut flags = MF_STRING;
        if current == *preset {
            flags |= MF_CHECKED;
        }
        let text = wide(&label);
        AppendMenuW(speed_menu, flags, ID_SPEED + index, text.as_ptr());
    }
    let header = wide("Speed limit");
    AppendMenuW(menu, MF_POPUP | MF_STRING, speed_menu as usize, header.as_ptr());

    add_separator(menu);
    add_item(menu, ID_QUIT, "Quit");

    let mut point = Point { x: 0, y: 0 };
    GetCursorPos(&mut point);
    // The classic pairing: focus first, then the menu, otherwise it can stay
    // open when the user clicks elsewhere.
    SetForegroundWindow(hwnd as *mut core::ffi::c_void);
    let command = TrackPopupMenu(
        menu,
        TPM_RETURNCMD | TPM_RIGHTBUTTON,
        point.x,
        point.y,
        0,
        hwnd,
        std::ptr::null(),
    );
    PostMessageW(hwnd, WM_NULL, 0, 0);
    DestroyMenu(menu);

    match command as usize {
        ID_OPEN => tray::push(TrayAction::Open),
        ID_PAUSE => tray::push(TrayAction::PauseAll),
        ID_RESUME => tray::push(TrayAction::ResumeAll),
        ID_QUIT => tray::push(TrayAction::Quit),
        other if other >= ID_SPEED && other < ID_SPEED + tray::SPEED_PRESETS.len() => {
            tray::push(TrayAction::Speed(tray::SPEED_PRESETS[other - ID_SPEED]));
        }
        _ => {}
    }
    wake();
}

unsafe extern "system" fn wnd_proc(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize {
    if msg == WM_APP_CALLBACK {
        let mouse = (lparam as u32) & 0xFFFF;
        match mouse {
            WM_LBUTTONUP => {
                tray::push(TrayAction::Open);
                wake();
            }
            WM_RBUTTONUP | WM_CONTEXTMENU => show_menu(hwnd),
            _ => {}
        }
        return 0;
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

pub struct TrayWin {
    hwnd: isize,
    icon: isize,
}

impl TrayWin {
    pub fn new(ctx: &egui::Context) -> Option<TrayWin> {
        if let Ok(mut slot) = REPAINT.lock() {
            *slot = Some(ctx.clone());
        }
        unsafe {
            let class = wide("LotusDMTrayWindow");
            let instance = GetModuleHandleW(std::ptr::null());
            let wc = WndClassW {
                style: 0,
                wnd_proc: Some(wnd_proc),
                cls_extra: 0,
                wnd_extra: 0,
                instance,
                icon: 0,
                cursor: 0,
                background: 0,
                menu_name: std::ptr::null(),
                class_name: class.as_ptr(),
            };
            // Fails harmlessly when the class already exists.
            RegisterClassW(&wc);

            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                class.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                instance,
                std::ptr::null_mut(),
            );
            if hwnd == 0 {
                return None;
            }

            let icon_path = std::env::temp_dir().join("lotus-dm-tray.ico");
            let icon = if std::fs::write(&icon_path, include_bytes!("../../assets/lotus.ico")).is_ok()
            {
                let path = wide_os(icon_path.as_os_str());
                LoadImageW(0, path.as_ptr(), IMAGE_ICON, 16, 16, LR_LOADFROMFILE)
            } else {
                0
            };

            let mut data = NotifyIconData::zeroed();
            data.cb_size = std::mem::size_of::<NotifyIconData>() as u32;
            data.hwnd = hwnd;
            data.id = TRAY_ID;
            data.flags = NIF_MESSAGE | NIF_TIP | (if icon != 0 { NIF_ICON } else { 0 });
            data.callback_message = WM_APP_CALLBACK;
            data.icon = icon;
            fill_text(&mut data.tip, "Lotus Download Manager");

            if Shell_NotifyIconW(NIM_ADD, &data) == 0 {
                DestroyWindow(hwnd);
                if icon != 0 {
                    DestroyIcon(icon);
                }
                return None;
            }
            HWND.store(hwnd, Ordering::Relaxed);
            Some(TrayWin { hwnd, icon })
        }
    }
}

impl Drop for TrayWin {
    fn drop(&mut self) {
        unsafe {
            let mut data = NotifyIconData::zeroed();
            data.cb_size = std::mem::size_of::<NotifyIconData>() as u32;
            data.hwnd = self.hwnd;
            data.id = TRAY_ID;
            Shell_NotifyIconW(NIM_DELETE, &data);
            DestroyWindow(self.hwnd);
            if self.icon != 0 {
                DestroyIcon(self.icon);
            }
        }
        HWND.store(0, Ordering::Relaxed);
    }
}

/// Balloon/toast in the notification area. Safe to call before the icon
/// exists: it simply does nothing then.
pub fn notify(title: &str, body: &str, is_error: bool) {
    let hwnd = HWND.load(Ordering::Relaxed);
    if hwnd == 0 {
        return;
    }
    unsafe {
        let mut data = NotifyIconData::zeroed();
        data.cb_size = std::mem::size_of::<NotifyIconData>() as u32;
        data.hwnd = hwnd;
        data.id = TRAY_ID;
        data.flags = NIF_INFO;
        fill_text(&mut data.info_title, title);
        fill_text(&mut data.info, body);
        data.info_flags = if is_error { NIIF_ERROR } else { NIIF_INFO };
        data.timeout = 5000;
        Shell_NotifyIconW(NIM_MODIFY, &data);
    }
}
