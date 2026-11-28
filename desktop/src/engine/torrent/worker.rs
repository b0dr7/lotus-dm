
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use super::rqbitapi::ManagedTorrent;

use crate::core::queue::DownloadJob;
use crate::engine::{emit, emit_state, fail, Control, EngineState, Event};

use super::{intake, session, Policy};

const TICK: Duration = Duration::from_millis(500);
const PAUSE_POLL: Duration = Duration::from_millis(100);
const SEED_POLL: Duration = Duration::from_secs(5);

pub fn run(job: DownloadJob, control: Arc<Control>, tx: Sender<Event>) {
    let Some(source) = intake::parse(&job.url) else {
        fail(
            &tx,
            &job.id,
            "not a magnet link, a .torrent link or a .torrent file".to_string(),
        );
        return;
    };

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

    if let Some(name) = torrent.name().filter(|name| !name.trim().is_empty()) {
        emit(&tx, Event::Name { id: job.id.clone(), name });
    }

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
            emit_state(&tx, &job.id, EngineState::Done);
            seed(&torrent, &control);
            let _ = session::forget(&torrent);
            return;
        }

        thread::sleep(TICK);
    }
}

fn seed(torrent: &Arc<ManagedTorrent>, control: &Control) {
    let policy = super::policy();
    let started = Instant::now();
    loop {
        let stats = torrent.stats();
        let ratio = if stats.total_bytes == 0 {
            0.0
        } else {
            stats.uploaded_bytes as f64 / stats.total_bytes as f64
        };
        if stats.error.is_some()
            || control.cancelled()
            || seeding_done(ratio, started.elapsed(), &policy)
        {
            return;
        }
        thread::sleep(SEED_POLL);
    }
}

fn seeding_done(ratio: f64, elapsed: Duration, policy: &Policy) -> bool {
    if policy.seed_ratio <= 0.0 && policy.seed_minutes == 0 {
        return true;
    }
    (policy.seed_ratio > 0.0 && ratio >= policy.seed_ratio)
        || (policy.seed_minutes > 0 && elapsed.as_secs() >= policy.seed_minutes * 60)
}

fn requeue(tx: &Sender<Event>, job: &DownloadJob, torrent: &Arc<ManagedTorrent>) {
    let _ = session::forget(torrent);
    emit_state(tx, &job.id, EngineState::Queued);
}

fn abort(tx: &Sender<Event>, job: &DownloadJob, torrent: &Arc<ManagedTorrent>, err: String) {
    fail(tx, &job.id, err);
    let _ = session::forget(torrent);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeding_stops_on_completion_ratio_or_time() {
        let off = Policy::default();
        assert!(seeding_done(0.0, Duration::ZERO, &off));

        let by_ratio = Policy {
            seed_ratio: 2.0,
            ..off
        };
        assert!(!seeding_done(1.5, Duration::from_secs(3600), &by_ratio));
        assert!(seeding_done(2.0, Duration::from_secs(1), &by_ratio));

        let by_time = Policy {
            seed_minutes: 10,
            ..off
        };
        assert!(!seeding_done(5.0, Duration::from_secs(599), &by_time));
        assert!(seeding_done(0.0, Duration::from_secs(600), &by_time));
    }
}
