//! System tray integration surface.
//!
//! The platform code lives in `tray_win` (Windows only). The rest of the app
//! only talks to this module: actions come out of `poll()` once per frame and
//! notifications go in through `notify()`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayAction {
    Open,
    PauseAll,
    ResumeAll,
    Speed(u64),
    Quit,
}

/// Entries of the "Speed limit" submenu, KB/s (0 = unlimited).
pub const SPEED_PRESETS: [u64; 5] = [0, 256, 1024, 4096, 8192];

static ACTIONS: LazyLock<Mutex<VecDeque<TrayAction>>> =
    LazyLock::new(|| Mutex::new(VecDeque::new()));
static CURRENT_KBPS: AtomicU64 = AtomicU64::new(0);

/// Called from the platform message loop.
pub(crate) fn push(action: TrayAction) {
    if let Ok(mut pending) = ACTIONS.lock() {
        pending.push_back(action);
    }
}

/// Everything the user clicked in the tray menu since the last frame.
pub fn poll() -> Vec<TrayAction> {
    match ACTIONS.lock() {
        Ok(mut pending) => pending.drain(..).collect(),
        Err(_) => Vec::new(),
    }
}

/// Keeps the check mark in the speed submenu on the setting in use.
pub fn set_speed_hint(kbps: u64) {
    CURRENT_KBPS.store(kbps, Ordering::Relaxed);
}

pub(crate) fn speed_hint() -> u64 {
    CURRENT_KBPS.load(Ordering::Relaxed)
}

/// A Windows notification when a download finishes or gives up.
pub fn notify(title: &str, body: &str, is_error: bool) {
    #[cfg(windows)]
    crate::ui::tray_win::notify(title, body, is_error);
    #[cfg(not(windows))]
    {
        let _ = (title, body, is_error);
    }
}

/// Owns the OS tray icon; dropping it removes the icon.
pub struct Tray {
    #[cfg(windows)]
    _inner: crate::ui::tray_win::TrayWin,
}

impl Tray {
    pub fn new(ctx: &eframe::egui::Context) -> Option<Tray> {
        #[cfg(windows)]
        {
            crate::ui::tray_win::TrayWin::new(ctx).map(|inner| Tray { _inner: inner })
        }
        #[cfg(not(windows))]
        {
            let _ = ctx;
            None
        }
    }
}
