use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, LazyLock, Mutex};
use std::thread;

use crate::core::queue::DownloadJob;

pub mod ftp;
pub mod job;
pub mod media;
pub mod seg_thread;
pub mod segmented;
pub mod site_explorer;
pub mod spider;
pub mod torrent;
pub mod worker;
pub mod zip_partial;

use job::RateLimiter;

pub use job::EngineState;

pub const SEGMENTS: usize = 8;

#[derive(Default)]
pub struct Control {
    cancel: AtomicBool,
    pause: AtomicBool,
}

impl Control {
    pub fn request_pause(&self) {
        self.pause.store(true, Ordering::Relaxed);
    }

    pub fn request_resume(&self) {
        self.pause.store(false, Ordering::Relaxed);
    }

    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub(crate) fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    pub(crate) fn paused(&self) -> bool {
        self.pause.load(Ordering::Relaxed)
    }
}

#[derive(Debug)]
pub enum Event {
    Progress {
        id: String,
        received: u64,
        total: u64,
        speed_bps: u64,
    },
    State {
        id: String,
        state: EngineState,
    },
    NoResume {
        id: String,
    },
    Name {
        id: String,
        name: String,
    },
}

pub fn spawn_download(job: DownloadJob, control: Arc<Control>, tx: Sender<Event>) {
    if torrent::is_torrent(&job.url) {
        thread::spawn(move || torrent::worker::run(job, control, tx));
    } else if ftp::is_ftp(&job.url) {
        thread::spawn(move || ftp::run(job, control, tx));
    } else if media::is_media(&job.url) {
        thread::spawn(move || media::run(job, control, tx));
    } else {
        thread::spawn(move || worker::run(job, control, tx, 0));
    }
}

static GLOBAL_LIMITER: LazyLock<Mutex<RateLimiter>> =
    LazyLock::new(|| Mutex::new(RateLimiter::new(0)));

pub fn set_global_limit_kbps(kbps: u64) {
    GLOBAL_LIMITER
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .set_limit(kbps);
}

pub(crate) fn throttle_global(n: u64) {
    GLOBAL_LIMITER
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .throttle(n);
}

pub(crate) fn emit(tx: &Sender<Event>, event: Event) {
    let _ = tx.send(event);
}

pub(crate) fn emit_state(tx: &Sender<Event>, id: &str, state: EngineState) {
    emit(
        tx,
        Event::State {
            id: id.to_string(),
            state,
        },
    );
}

pub(crate) fn fail(tx: &Sender<Event>, id: &str, message: String) {
    emit_state(tx, id, EngineState::Failed(message));
}

pub(crate) fn cancelled_cleanup(tx: &Sender<Event>, id: &str, parts: &Path) {
    let _ = std::fs::remove_dir_all(parts);
    emit_state(tx, id, EngineState::Queued);
}
