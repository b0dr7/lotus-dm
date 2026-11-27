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

/// Fire every retry whose backoff has elapsed.
pub fn pump_retries(
    queue: &mut Vec<DownloadJob>,
    controls: &mut HashMap<String, Arc<Control>>,
    tx: &Sender<Event>,
    tracker: &mut RetryTracker,
) {
    for id in tracker.take_due() {
        let Some(slot) = queue
            .iter_mut()
            .find(|job| job.id == id && job.status == JobStatus::Failed)
        else {
            continue;
        };
        slot.status = JobStatus::Queued;
        let job = slot.clone();
        spawn_job(&job, controls, tx);
    }
}

/// Scheduler trigger: start every queued job at once.
pub fn start_all_queued(
    queue: &mut Vec<DownloadJob>,
    controls: &mut HashMap<String, Arc<Control>>,
    tx: &Sender<Event>,
) {
    for index in 0..queue.len() {
        if queue[index].status == JobStatus::Queued {
            let job = queue[index].clone();
            spawn_job(&job, controls, tx);
        }
    }
}
