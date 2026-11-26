use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;

use crate::core::queue::DownloadJob;

pub mod job;
pub mod worker;

pub use job::EngineState;

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
}

pub fn spawn_download(job: DownloadJob, control: Arc<Control>, tx: Sender<Event>) {
    thread::spawn(move || worker::run(job, control, tx));
}
