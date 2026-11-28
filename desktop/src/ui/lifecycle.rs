use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::core::queue::{DownloadJob, JobStatus};
use crate::engine::{self, Control, Event};

/// Backoff schedule: retry 1 at 30s, retry 2 at 2m, retry 3 at 10m.
const BACKOFF: [Duration; 3] = [
    Duration::from_secs(30),
    Duration::from_secs(120),
    Duration::from_secs(600),
];

struct Retry {
    attempts: u32,
    next: Option<Instant>,
}

#[derive(Default)]
pub struct RetryTracker {
    map: HashMap<String, Retry>,
}

impl RetryTracker {
    /// Schedule the next automatic retry for a failed job, unless the three
    /// allowed attempts are already spent.
    pub fn on_failure(&mut self, id: &str) {
        let entry = self.map.entry(id.to_string()).or_insert(Retry {
            attempts: 0,
            next: None,
        });
        if (entry.attempts as usize) < BACKOFF.len() {
            entry.next = Some(Instant::now() + BACKOFF[entry.attempts as usize]);
            entry.attempts += 1;
        }
    }

    /// A manual retry starts a fresh backoff episode.
    pub fn forget(&mut self, id: &str) {
        self.map.remove(id);
    }

    /// True while another automatic attempt is queued, so the UI can hold
    /// back the "failed" notification until the last one is spent.
    pub fn will_retry(&self, id: &str) -> bool {
        self.map.get(id).is_some_and(|entry| entry.next.is_some())
    }

    /// Ids whose backoff elapsed; the attempt count survives for the next
    /// failure so the chain still stops after three.
    pub fn take_due(&mut self) -> Vec<String> {
        let now = Instant::now();
        let mut due = Vec::new();
        for (id, entry) in self.map.iter_mut() {
            if entry.next.is_some_and(|next| next <= now) {
                entry.next = None;
                due.push(id.clone());
            }
        }
        due
    }
}

/// Register a control and launch the engine thread for a job.
pub fn spawn_job(
    job: &DownloadJob,
    controls: &mut HashMap<String, Arc<Control>>,
    tx: &Sender<Event>,
) -> bool {
    if controls.contains_key(&job.id) {
        return false;
    }
    let control = Arc::new(Control::default());
    controls.insert(job.id.clone(), control.clone());
    engine::spawn_download(job.clone(), control, tx.clone());
    true
}

/// How many more downloads may start: the limit minus what is already running.
/// 0 means "no limit".
pub fn free_slots(queue: &[DownloadJob], limit: u8) -> usize {
    if limit == 0 {
        return usize::MAX;
    }
    let running = queue
        .iter()
        .filter(|job| job.status == JobStatus::Running)
        .count();
    (limit as usize).saturating_sub(running)
}

/// Start queued downloads while the concurrency limit has room, oldest first.
/// The slot is taken immediately (the job is marked running before the engine
/// thread reports in), so one frame cannot start more than the limit allows.
pub fn start_pending(
    queue: &mut Vec<DownloadJob>,
    controls: &mut HashMap<String, Arc<Control>>,
    tx: &Sender<Event>,
    limit: u8,
) {
    let mut free = free_slots(queue, limit);
    for index in 0..queue.len() {
        if free == 0 {
            break;
        }
        if queue[index].status != JobStatus::Queued {
            continue;
        }
        let job = queue[index].clone();
        if spawn_job(&job, controls, tx) {
            queue[index].status = JobStatus::Running;
            queue[index].speed_bps = 0;
            free -= 1;
        }
    }
}

/// Hand every retry whose backoff has elapsed back to the queue; `start_pending`
/// decides when there is room for it.
pub fn pump_retries(queue: &mut Vec<DownloadJob>, tracker: &mut RetryTracker) {
    for id in tracker.take_due() {
        if let Some(slot) = queue
            .iter_mut()
            .find(|job| job.id == id && job.status == JobStatus::Failed)
        {
            slot.status = JobStatus::Queued;
        }
    }
}

/// Scheduler trigger: let the queue drain, still respecting the limit.
pub fn start_all_queued(
    queue: &mut Vec<DownloadJob>,
    controls: &mut HashMap<String, Arc<Control>>,
    tx: &Sender<Event>,
    limit: u8,
) {
    start_pending(queue, controls, tx, limit);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(status: JobStatus) -> DownloadJob {
        let mut job = DownloadJob::new_queued(
            "https://example.com/a.bin".to_string(),
            "a.bin".to_string(),
            "C:/Downloads".to_string(),
        );
        job.status = status;
        job
    }

    #[test]
    fn the_limit_counts_only_what_is_running() {
        let queue = vec![
            job(JobStatus::Running),
            job(JobStatus::Paused),
            job(JobStatus::Queued),
            job(JobStatus::Done),
        ];
        // 0 is unlimited, so nothing is ever held back.
        assert_eq!(free_slots(&queue, 0), usize::MAX);
        assert_eq!(free_slots(&queue, 3), 2);
        assert_eq!(free_slots(&queue, 1), 0);
    }
}
