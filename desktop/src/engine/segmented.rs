use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::core::queue::DownloadJob;

use super::job::{self, Meta, RateLimiter, SegmentRange};
use super::seg_thread::{self, SegOutcome, Shared};
use super::{emit, emit_state, fail, Control, EngineState, Event, SEGMENTS};

const TICK: Duration = Duration::from_millis(250);
const META_WRITE: Duration = Duration::from_secs(1);
const POLL: Duration = Duration::from_millis(50);

pub(super) fn run_fresh(
    job: DownloadJob,
    control: Arc<Control>,
    tx: Sender<Event>,
    total: u64,
    etag: Option<String>,
    last_modified: Option<String>,
    restarts: u32,
) {
    let meta = Meta {
        url: job.url.clone(),
        total,
        etag,
        last_modified,
        received: 0,
        segments: Some(split_ranges(total, SEGMENTS)),
    };
    // stale single-stream bytes must not survive into the segmented layout
    for index in 0..SEGMENTS {
        let _ = std::fs::remove_file(job::segment_path(&job.folder, &job.id, index));
    }
    if let Err(err) = job::write_meta(&job::meta_path(&job.folder, &job.id), &meta) {
        fail(&tx, &job.id, format!("meta: {err}"));
        return;
    }
    coordinator(job, control, tx, meta, restarts);
}

pub(super) fn resume(
    job: DownloadJob,
    control: Arc<Control>,
    tx: Sender<Event>,
    meta: Meta,
    restarts: u32,
) {
    coordinator(job, control, tx, meta, restarts);
}

fn split_ranges(total: u64, count: usize) -> Vec<SegmentRange> {
    debug_assert!(total >= count as u64);
    let count = count as u64;
    (0..count)
        .map(|i| SegmentRange {
            start: total * i / count,
            end: total * (i + 1) / count - 1,
        })
        .collect()
}

fn coordinator(
    job: DownloadJob,
    control: Arc<Control>,
    tx: Sender<Event>,
    mut meta: Meta,
    restarts: u32,
) {
    let parts = job::parts_dir(&job.folder, &job.id);
    let meta_file = job::meta_path(&job.folder, &job.id);
    let ranges = meta.segments.clone().unwrap_or_default();
    let bad_table = ranges.len() != SEGMENTS
        || meta.total == 0
        || ranges
            .iter()
            .any(|r| r.end < r.start || r.end >= meta.total);
    if bad_table {
        if restarts >= 3 {
            fail(&tx, &job.id, "bad segment table".to_string());
            return;
        }
        let _ = std::fs::remove_dir_all(&parts);
        super::worker::run(job, control, tx, restarts + 1);
        return;
    }

    emit_state(&tx, &job.id, EngineState::Running);

    let mut initial = 0u64;
    for (index, range) in ranges.iter().enumerate() {
        let on_disk = job::part_len(&job::segment_path(&job.folder, &job.id, index));
        initial += on_disk.min(range.len());
    }

    let received = Arc::new(AtomicU64::new(initial));
    let abort = Arc::new(AtomicBool::new(false));
    let finished = Arc::new(AtomicUsize::new(0));
    let outcomes: Arc<Mutex<Vec<SegOutcome>>> = Arc::new(Mutex::new(Vec::with_capacity(SEGMENTS)));
    let job_limiter = (job.speed_limit_kbps > 0)
        .then(|| Arc::new(Mutex::new(RateLimiter::new(job.speed_limit_kbps))));

    let mut handles = Vec::with_capacity(SEGMENTS);
    for (index, range) in ranges.into_iter().enumerate() {
        let shared = Shared {
            control: control.clone(),
            received: received.clone(),
            abort: abort.clone(),
            job_limiter: job_limiter.clone(),
            meta: meta.clone(),
        };
        let finished = finished.clone();
        let outcomes = outcomes.clone();
        let job = job.clone();
        handles.push(thread::spawn(move || {
            // A panicking segment must still count itself out or the
            // coordinator would wait forever.
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                seg_thread::segment_loop(index, range, job, shared)
            }))
            .unwrap_or_else(|_| SegOutcome::Failed("segment thread crashed".to_string()));
            outcomes
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .push(outcome);
            finished.fetch_add(1, Ordering::Release);
        }));
    }

    let mut paused_emitted = false;
    let mut last_tick = Instant::now();
    let mut last_bytes = initial;
    let mut last_meta = Instant::now();

    loop {
        let decisive = outcomes
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .iter()
            .any(|o| matches!(o, SegOutcome::Failed(_) | SegOutcome::Changed));
        if control.cancelled() || decisive {
            abort.store(true, Ordering::Release);
        }
        if finished.load(Ordering::Acquire) == SEGMENTS {
            break;
        }
        if control.paused() && !paused_emitted {
            emit_state(&tx, &job.id, EngineState::Paused);
            paused_emitted = true;
        }
        if !control.paused() && paused_emitted {
            emit_state(&tx, &job.id, EngineState::Running);
            paused_emitted = false;
        }
        if last_tick.elapsed() >= TICK {
            let now = received.load(Ordering::Acquire);
            let elapsed = last_tick.elapsed().as_secs_f64();
            let speed = ((now.saturating_sub(last_bytes)) as f64 / elapsed) as u64;
            emit(
                &tx,
                Event::Progress {
                    id: job.id.clone(),
                    received: now,
                    total: meta.total,
                    speed_bps: speed,
                },
            );
            last_tick = Instant::now();
            last_bytes = now;
        }
        if last_meta.elapsed() >= META_WRITE {
            let _ = std::fs::create_dir_all(&parts);
            meta.received = received.load(Ordering::Acquire);
            let _ = job::write_meta(&meta_file, &meta);
            last_meta = Instant::now();
        }
        thread::sleep(POLL);
    }

    for handle in handles {
        let _ = handle.join();
    }
    let results = {
        let mut guard = outcomes.lock().unwrap_or_else(|err| err.into_inner());
        std::mem::take(&mut *guard)
    };

    let all_done =
        results.len() == SEGMENTS && results.iter().all(|o| matches!(o, SegOutcome::Done));
    if all_done {
        match merge(&job, meta.total) {
            Ok(()) => {
                let _ = std::fs::remove_dir_all(&parts);
                emit(
                    &tx,
                    Event::Progress {
                        id: job.id.clone(),
                        received: meta.total,
                        total: meta.total,
                        speed_bps: 0,
                    },
                );
                emit_state(&tx, &job.id, EngineState::Done);
            }
            Err(err) => fail(&tx, &job.id, err),
        }
        return;
    }
    if control.cancelled() {
        let _ = std::fs::remove_dir_all(&parts);
        emit_state(&tx, &job.id, EngineState::Queued);
        return;
    }
    if results.iter().any(|o| matches!(o, SegOutcome::Changed)) {
        if restarts >= 3 {
            fail(&tx, &job.id, "remote file changed repeatedly".to_string());
            return;
        }
        let _ = std::fs::remove_dir_all(&parts);
        super::worker::run(job, control, tx, restarts + 1);
        return;
    }
    if let Some(SegOutcome::Failed(message)) =
        results.iter().find(|o| matches!(o, SegOutcome::Failed(_)))
    {
        fail(&tx, &job.id, message.clone());
    }
}

fn merge(job: &DownloadJob, total: u64) -> Result<(), String> {
    let target = job::unique_target(&job.folder, &job.filename);
    let result = copy_segments(job, &target);
    if result.is_err() {
        let _ = std::fs::remove_file(&target);
    }
    result.and_then(|()| {
        let size = std::fs::metadata(&target)
            .map(|data| data.len())
            .map_err(|err| format!("verify target: {err}"))?;
        if size != total {
            return Err(format!("merged size {size} != expected {total}"));
        }
        Ok(())
    })
}

fn copy_segments(job: &DownloadJob, target: &std::path::Path) -> Result<(), String> {
    let mut out = std::fs::File::create(target).map_err(|err| format!("create target: {err}"))?;
    for index in 0..SEGMENTS {
        let mut input = std::fs::File::open(job::segment_path(&job.folder, &job.id, index))
            .map_err(|err| format!("open segment {index}: {err}"))?;
        std::io::copy(&mut input, &mut out)
            .map_err(|err| format!("merge segment {index}: {err}"))?;
    }
    let _ = out.flush();
    Ok(())
}
