//! Queue worker for torrent jobs.
//!
//! Same shape as the HTTP worker: it runs on its own thread, reports through
//! `engine::Event` and obeys `engine::Control`. A torrent job is a normal
//! `DownloadJob` whose URL the torrent engine accepted.

use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use librqbit::ManagedTorrent;

use crate::core::queue::DownloadJob;
use crate::engine::{emit, emit_state, fail, Control, EngineState, Event};

use super::{intake, session};

/// How often progress is sampled. Torrent stats are cheap to read, and this is
/// also the pause/cancel reaction time.
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
        fail(&tx, &job.id, format!("cannot create {}: {err}", job.folder));
        return;
    }

    emit_state(&tx, &job.id, EngineState::Running);
    let torrent = match session::add(&source, &job.folder) {
        Ok(torrent) => torrent,
        Err(err) => {
            fail(&tx, &job.id, err);
            return;
        }
    };

    loop {
        if control.cancelled() {
            requeue(&tx, &job, &torrent);
            return;
        }

        if control.paused() {
            if let Err(err) = session::pause(&torrent) {
                abort(&tx, &job, &torrent, err);
                return;
            }
            emit_state(&tx, &job.id, EngineState::Paused);
            while control.paused() && !control.cancelled() {
                thread::sleep(PAUSE_POLL);
            }
            if control.cancelled() {
                requeue(&tx, &job, &torrent);
                return;
            }
            if let Err(err) = session::resume(&torrent) {
                abort(&tx, &job, &torrent, err);
                return;
            }
            emit_state(&tx, &job.id, EngineState::Running);
            continue;
        }

        let stats = torrent.stats();
        if let Some(err) = &stats.error {
            abort(&tx, &job, &torrent, format!("torrent failed: {err}"));
            return;
        }

        emit(
            &tx,
            Event::Progress {
                id: job.id.clone(),
                received: stats.progress_bytes,
                total: stats.total_bytes,
                speed_bps: stats
                    .live
                    .as_ref()
                    .map_or(0, |live| live.download_speed.as_bytes()),
            },
        );

        if stats.finished {
            // Seeding is not part of the queue lifecycle and the files stay on
            // disk, so let the session stop talking to peers.
            emit_state(&tx, &job.id, EngineState::Done);
            let _ = session::forget(&torrent);
            return;
        }

        thread::sleep(TICK);
    }
}

/// Hand the job back to the queue, keeping what was already downloaded so a
/// retry can resume it.
fn requeue(tx: &Sender<Event>, job: &DownloadJob, torrent: &Arc<ManagedTorrent>) {
    let _ = session::forget(torrent);
    emit_state(tx, &job.id, EngineState::Queued);
}

/// Report a fatal error and drop the torrent from the session.
fn abort(
    tx: &Sender<Event>,
    job: &DownloadJob,
    torrent: &Arc<ManagedTorrent>,
    err: String,
) {
    fail(tx, &job.id, err);
    let _ = session::forget(torrent);
}
