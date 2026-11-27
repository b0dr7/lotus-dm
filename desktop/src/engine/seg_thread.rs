use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::core::queue::DownloadJob;

use super::job::{self, Meta, RateLimiter, SegmentRange};
use super::{throttle_global, Control};

const CHUNK: usize = 64 * 1024;
const PARK: Duration = Duration::from_millis(100);
const MAX_EARLY: u32 = 3;

pub(super) enum SegOutcome {
    Done,
    Failed(String),
    Changed,
    Stopped,
}

pub(super) struct Shared {
    pub control: Arc<Control>,
    pub received: Arc<AtomicU64>,
    pub abort: Arc<AtomicBool>,
    pub job_limiter: Option<Arc<Mutex<RateLimiter>>>,
    pub meta: Meta,
}

/// Bytes on disk for a segment, truncating anything past its range so a
/// misbehaving server cannot corrupt the merge.
fn disk_len(path: &Path, cap: u64) -> u64 {
    let len = job::part_len(path);
    if len > cap {
        if let Ok(file) = std::fs::OpenOptions::new().write(true).open(path) {
            let _ = file.set_len(cap);
        }
        return cap;
    }
    len
}

fn open_append(path: &Path) -> std::io::Result<std::fs::File> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .append(true)
        .open(path)
}

pub(super) fn segment_loop(
    index: usize,
    range: SegmentRange,
    job: DownloadJob,
    shared: Shared,
) -> SegOutcome {
    let path = job::segment_path(&job.folder, &job.id, index);
    let seg_len = range.len();
    let mut offset = disk_len(&path, seg_len);

    'segment: loop {
        if shared.control.cancelled() || shared.abort.load(Ordering::Acquire) {
            return SegOutcome::Stopped;
        }
        if offset >= seg_len {
            return SegOutcome::Done;
        }

        let req = ureq::get(&job.url).set("User-Agent", "LotusDM/0.1").set(
            "Range",
            &format!("bytes={}-{}", range.start + offset, range.end),
        );
        let resp = match req.call() {
            Ok(resp) => resp,
            Err(ureq::Error::Status(416, _)) => {
                if disk_len(&path, seg_len) >= seg_len {
                    return SegOutcome::Done;
                }
                return SegOutcome::Changed;
            }
            Err(ureq::Error::Status(code, _)) => return SegOutcome::Failed(format!("HTTP {code}")),
            Err(ureq::Error::Transport(err)) => return SegOutcome::Failed(err.to_string()),
        };

        if resp.status() != 206 {
            return SegOutcome::Changed;
        }
        let etag = resp.header("etag").map(str::to_string);
        let last_modified = resp.header("last-modified").map(str::to_string);
        if job::validators_changed(&shared.meta, &etag, &last_modified) {
            return SegOutcome::Changed;
        }
        if let Some(header) = resp.header("content-range") {
            let expected = range.start + offset;
            let confirmed = header
                .strip_prefix("bytes ")
                .and_then(|rest| rest.split_once('/'))
                .is_some_and(|(span, total)| {
                    let start_ok = span
                        .split_once('-')
                        .is_some_and(|(s, _)| s.parse::<u64>() == Ok(expected));
                    let total_ok =
                        total.trim() == "*" || total.trim().parse::<u64>() == Ok(shared.meta.total);
                    start_ok && total_ok
                });
            if !confirmed {
                return SegOutcome::Changed;
            }
        }

        let mut file = match open_append(&path) {
            Ok(file) => file,
            Err(err) => return SegOutcome::Failed(format!("open part: {err}")),
        };
        let mut reader = resp.into_reader();
        let mut buf = vec![0u8; CHUNK];
        let mut early = 0u32;

        loop {
            if shared.control.cancelled() || shared.abort.load(Ordering::Acquire) {
                return SegOutcome::Stopped;
            }
            if shared.control.paused() {
                drop(file);
                while shared.control.paused()
                    && !shared.control.cancelled()
                    && !shared.abort.load(Ordering::Acquire)
                {
                    thread::sleep(PARK);
                }
                if shared.control.cancelled() || shared.abort.load(Ordering::Acquire) {
                    return SegOutcome::Stopped;
                }
                // Parts may have been deleted while parked; resync from disk.
                let on_disk = disk_len(&path, seg_len);
                if on_disk != offset {
                    offset = on_disk;
                    continue 'segment;
                }
                file = match open_append(&path) {
                    Ok(file) => file,
                    Err(err) => return SegOutcome::Failed(format!("reopen part: {err}")),
                };
                continue;
            }

            let n = match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(err) => return SegOutcome::Failed(format!("read: {err}")),
            };
            if let Err(err) = file.write_all(&buf[..n]) {
                return SegOutcome::Failed(format!("write: {err}"));
            }
            offset += n as u64;
            shared.received.fetch_add(n as u64, Ordering::Acquire);
            match &shared.job_limiter {
                Some(limiter) => limiter
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .throttle(n as u64),
                None => throttle_global(n as u64),
            }
        }

        let _ = file.flush();
        drop(file);

        let on_disk = disk_len(&path, seg_len);
        if on_disk >= seg_len {
            return SegOutcome::Done;
        }
        early += 1;
        if early > MAX_EARLY {
            return SegOutcome::Failed(format!("connection ended early ({on_disk}/{seg_len})"));
        }
        offset = on_disk;
    }
}
