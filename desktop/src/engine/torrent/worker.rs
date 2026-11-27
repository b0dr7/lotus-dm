//! Queue worker for torrent jobs.
//!
//! Same shape as the HTTP worker: it runs on its own thread, reports through
//! `engine::Event` and obeys `engine::Control`. A torrent job is a normal
//! `DownloadJob` whose URL the torrent engine accepted.

use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::core::queue::DownloadJob;
use crate::engine::{emit, emit_state, fail, Control, EngineState, Event};

use super::intake;
use super::session::{self, TorrentHandle};

/// How often progress is sampled. Torrent stats are cheap, but this is also
/// the pause/cancel reaction time.
const TICK: Duration = Duration::from_millis(500);
const PAUSE_POLL: Duration = Duration::from_millis(100);

pub fn run(job: DownloadJob, control: Arc<Control>, tx: Sender<Event>) {
    let Some(source) = intake::parse(&job.url) else {
        fail(
            &tx,
            &job.id,
            "not a magnet link, a .torrent link or a .torrent file".to_string(),
        );
        return;
    };

    if let Err(err) = super::check_encryption() {
        fail(&tx, &job.id, err);
        return;
    }

    if let Err(err) = std::fs::create_dir_all(&job.folder) {
        fail(
            &tx,
            &job.id,
            format!("cannot create {}: {err}", job.folder),
        );
        return;
    }

    emit_state(&tx, &job.id, EngineState::Running);
    let handle = match session::add(&source, &job.folder, false) {
        Ok(handle) => handle,
        Err(err) => {
            fail(&tx, &job.id, err);
            return;
        }
    };

    eprintln!(
        "lotusdm: torrent \"{}\" added to {}",
        handle.name(),
        job.folder
    );

    let mut speed = SpeedMeter::default();
    loop {
        if control.cancelled() {
            // The queue may still retry this job, so keep what was written.
            let _ = session::remove(&handle, false);
            emit_state(&tx, &job.id, EngineState::Queued);
            return;
        }

        if control.paused() {
            if let Err(err) = session::set_paused(&handle, true) {
                fail(&tx, &job.id, err);
                let _ = session::remove(&handle, false);
                return;
            }
            emit_state(&tx, &job.id, EngineState::Paused);
            while control.paused() && !control.cancelled() {
                thread::sleep(PAUSE_POLL);
            }
            if control.cancelled() {
                let _ = session::remove(&handle, false);
                emit_state(&tx, &job.id, EngineState::Queued);
                return;
            }
            if let Err(err) = session::set_paused(&handle, false) {
                fail(&tx, &job.id, err);
                let _ = session::remove(&handle, false);
                return;
            }
            speed.reset();
            emit_state(&tx, &job.id, EngineState::Running);
            continue;
        }

        match tick(&job, &handle, &tx, &mut speed) {
            Ok(true) => return,
            Ok(false) => {}
            Err(err) => {
                fail(&tx, &job.id, err);
                let _ = session::remove(&handle, false);
                return;
            }
        }

        thread::sleep(TICK);
    }
}

/// One progress sample. `Ok(true)` means the torrent completed and the stop
/// was already reported; `Err` means the torrent failed for good.
fn tick(
    job: &DownloadJob,
    handle: &TorrentHandle,
    tx: &Sender<Event>,
    speed: &mut SpeedMeter,
) -> Result<bool, String> {
    let stats = handle.stats();

    if let Some(err) = stats.error {
        return Err(format!("torrent failed: {err}"));
    }

    emit(
        tx,
        Event::Progress {
            id: job.id.clone(),
            received: stats.progress_bytes,
            total: stats.total_bytes,
            speed_bps: speed.sample(stats.progress_bytes),
        },
    );

    if !stats.finished {
        return Ok(false);
    }

    emit_state(tx, &job.id, EngineState::Done);
    // Seeding is not part of the queue lifecycle and the files stay on disk,
    // so forget the torrent and let the session stop talking to peers.
    let _ = session::remove(handle, false);
    Ok(true)
}

/// Byte counter to bytes-per-second, sampled at most once per interval.
#[derive(Default)]
struct SpeedMeter {
    last_bytes: u64,
    last_at: Option<Instant>,
    bps: u64,
}

impl SpeedMeter {
    fn reset(&mut self) {
        self.last_bytes = 0;
        self.last_at = None;
        self.bps = 0;
    }

    fn sample(&mut self, bytes: u64) -> u64 {
        let now = Instant::now();
        if let Some(last_at) = self.last_at {
            let elapsed = now.duration_since(last_at).as_secs_f64();
            if elapsed >= 0.5 {
                let delta = bytes.saturating_sub(self.last_bytes);
                self.bps = (delta as f64 / elapsed) as u64;
                self.last_bytes = bytes;
                self.last_at = Some(now);
            }
        } else {
            self.last_bytes = bytes;
            self.last_at = Some(now);
        }
        self.bps
    }
}
