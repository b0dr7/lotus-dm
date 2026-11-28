
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayAction {
    Open,
    PauseAll,
    ResumeAll,
    Speed(u64),
    Quit,
}

pub const SPEED_PRESETS: [u64; 5] = [0, 256, 1024, 4096, 8192];

static ACTIONS: LazyLock<Mutex<VecDeque<TrayAction>>> =
    LazyLock::new(|| Mutex::new(VecDeque::new()));
static CURRENT_KBPS: AtomicU64 = AtomicU64::new(0);

pub(crate) fn push(action: TrayAction) {
    if let Ok(mut pending) = ACTIONS.lock() {
        pending.push_back(action);
    }
}

pub fn poll() -> Vec<TrayAction> {
    match ACTIONS.lock() {
        Ok(mut pending) => pending.drain(..).collect(),
        Err(_) => Vec::new(),
    }
}

pub fn set_speed_hint(kbps: u64) {
    CURRENT_KBPS.store(kbps, Ordering::Relaxed);
}

pub(crate) fn speed_hint() -> u64 {
    CURRENT_KBPS.load(Ordering::Relaxed)
}

pub fn notify(title: &str, body: &str, is_error: bool) {
    #[cfg(windows)]
    crate::ui::tray_win::notify(title, body, is_error);
    #[cfg(not(windows))]
    {
        let _ = (title, body, is_error);
    }
}

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
