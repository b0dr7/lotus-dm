use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::core::queue::{DownloadJob, JobStatus};
use crate::engine::{self, Control, Event};

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

    pub fn forget(&mut self, id: &str) {
        self.map.remove(id);
    }

    pub fn will_retry(&self, id: &str) -> bool {
        self.map.get(id).is_some_and(|entry| entry.next.is_some())
    }

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
        if let Some(start) = queue[index].start_at {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            if start > now_ms {
                continue;
            }
        }
        let job = queue[index].clone();
        if spawn_job(&job, controls, tx) {
            queue[index].status = JobStatus::Running;
            queue[index].speed_bps = 0;
            free -= 1;
        }
    }
}

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
            "https:
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
        assert_eq!(free_slots(&queue, 0), usize::MAX);
        assert_eq!(free_slots(&queue, 3), 2);
        assert_eq!(free_slots(&queue, 1), 0);
    }
}
