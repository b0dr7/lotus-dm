use std::io::{Read, Write};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::core::queue::DownloadJob;

use super::job::{self, header_total, part_path, parts_dir, validators_changed, RateLimiter};
use super::{
    cancelled_cleanup, emit, emit_state, fail, throttle_global, Control, EngineState, Event,
    SEGMENTS,
};

const CHUNK: usize = 64 * 1024;
const TICK: Duration = Duration::from_millis(250);
const META_WRITE: Duration = Duration::from_secs(1);

pub(super) fn run(job: DownloadJob, control: Arc<Control>, tx: Sender<Event>, restarts: u32) {
    let parts = parts_dir(&job.folder, &job.id);
    if let Err(err) = std::fs::create_dir_all(&parts) {
        fail(
            &tx,
            &job.id,
            format!("cannot create {}: {err}", parts.display()),
        );
        return;
    }
    let part = part_path(&job.folder, &job.id);
    let meta_file = job::meta_path(&job.folder, &job.id);

    let mut meta = job::load_meta(&meta_file);
    if let Some(stored) = &meta {
        if stored.url != job.url {
            let _ = std::fs::remove_dir_all(&parts);
            let _ = std::fs::create_dir_all(&parts);
            meta = None;
        }
    }
    if let Some(stored) = &meta {
        if stored.segments.is_some() {
            super::segmented::resume(job, control, tx, stored.clone(), restarts);
            return;
        }
    }
    let started_without_meta = meta.is_none();

    let mut offset = job::part_len(&part);
    if offset > 0 && meta.is_none() {
        offset = 0;
    }

    emit_state(&tx, &job.id, EngineState::Running);
    let mut restarts = restarts;

    'restart: loop {
        let ranged = offset > 0;
        let mut request = ureq::get(&job.url).set("User-Agent", "LotusDM/0.1");
        if ranged {
            request = request.set("Range", &format!("bytes={offset}-"));
        }

        let resp = match request.call() {
            Ok(resp) => resp,
            Err(ureq::Error::Status(code, _)) => {
                if code == 416 && ranged {
                    let _ = std::fs::remove_file(&part);
                    offset = 0;
                    meta = None;
                    restarts += 1;
                    if restarts > 3 {
                        fail(&tx, &job.id, "remote file changed repeatedly".to_string());
                        return;
                    }
                    continue 'restart;
                }
                fail(&tx, &job.id, format!("HTTP {code}"));
                return;
            }
            Err(ureq::Error::Transport(err)) => {
                fail(&tx, &job.id, err.to_string());
                return;
            }
        };

        let status = resp.status();
        let etag = resp.header("etag").map(str::to_string);
        let last_modified = resp.header("last-modified").map(str::to_string);

        if ranged && status == 206 {
            let changed = match &meta {
                Some(stored) => validators_changed(stored, &etag, &last_modified),
                None => true,
            };
            if changed {
                let _ = std::fs::remove_file(&part);
                offset = 0;
                meta = None;
                restarts += 1;
                if restarts > 3 {
                    fail(&tx, &job.id, "remote file changed repeatedly".to_string());
                    return;
                }
                continue 'restart;
            }
        } else if status == 200 {
            if ranged {
                offset = 0;
                meta = None;
            }
            let total = header_total(&resp, &meta);
            let resumable = resp
                .header("accept-ranges")
                .is_some_and(|v| v.eq_ignore_ascii_case("bytes"));
            if resumable && started_without_meta && total >= SEGMENTS as u64 {
                super::segmented::run_fresh(
                    job.clone(),
                    control.clone(),
                    tx.clone(),
                    total,
                    etag.clone(),
                    last_modified.clone(),
                    restarts,
                );
                return;
            }
            if resumable {
                meta = Some(job::Meta {
                    url: job.url.clone(),
                    total,
                    etag,
                    last_modified,
                    received: 0,
                    segments: None,
                });
            } else {
                meta = None;
                emit(&tx, Event::NoResume { id: job.id.clone() });
            }
        } else if status != 206 {
            fail(&tx, &job.id, format!("HTTP {status}"));
            return;
        }

        let total = header_total(&resp, &meta);
        if let Some(stored) = &mut meta {
            stored.total = total.max(stored.total);
            stored.received = offset;
            let _ = job::write_meta(&meta_file, stored);
        }

        let mut open = std::fs::OpenOptions::new();
        open.write(true).create(true);
        if offset > 0 {
            open.append(true);
        } else {
            open.truncate(true);
        }
        let mut file = match open.open(&part) {
            Ok(file) => file,
            Err(err) => {
                fail(&tx, &job.id, format!("open part: {err}"));
                return;
            }
        };

        let mut reader = resp.into_reader();
        let mut buf = vec![0u8; CHUNK];
        let mut received = offset;
        let job_limiter =
            (job.speed_limit_kbps > 0).then(|| Mutex::new(RateLimiter::new(job.speed_limit_kbps)));
        let mut last_tick = Instant::now();
        let mut last_bytes = received;
        let mut last_meta = Instant::now();

        loop {
            if control.cancelled() {
                drop(file);
                cancelled_cleanup(&tx, &job.id, &parts);
                return;
            }
            if control.paused() {
                drop(file);
                emit_state(&tx, &job.id, EngineState::Paused);
                while control.paused() && !control.cancelled() {
                    thread::sleep(Duration::from_millis(100));
                }
                if control.cancelled() {
                    cancelled_cleanup(&tx, &job.id, &parts);
                    return;
                }
                let expected = received;
                received = job::part_len(&part);
                if received != expected {
                    offset = received;
                    meta = job::load_meta(&meta_file);
                    emit_state(&tx, &job.id, EngineState::Running);
                    continue 'restart;
                }
                file = match std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .append(true)
                    .open(&part)
                {
                    Ok(file) => file,
                    Err(err) => {
                        fail(&tx, &job.id, format!("reopen part: {err}"));
                        return;
                    }
                };
                emit_state(&tx, &job.id, EngineState::Running);
                last_tick = Instant::now();
                last_bytes = received;
                last_meta = Instant::now();
                continue;
            }

            let n = match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(err) => {
                    fail(&tx, &job.id, format!("read: {err}"));
                    return;
                }
            };
            if let Err(err) = file.write_all(&buf[..n]) {
                fail(&tx, &job.id, format!("write: {err}"));
                return;
            }
            received += n as u64;
            match &job_limiter {
                Some(limiter) => limiter
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .throttle(n as u64),
                None => throttle_global(n as u64),
            }

            if last_tick.elapsed() >= TICK {
                let elapsed = last_tick.elapsed().as_secs_f64();
                let speed = ((received - last_bytes) as f64 / elapsed) as u64;
                emit(
                    &tx,
                    Event::Progress {
                        id: job.id.clone(),
                        received,
                        total,
                        speed_bps: speed,
                    },
                );
                last_tick = Instant::now();
                last_bytes = received;
            }
            if last_meta.elapsed() >= META_WRITE {
                if let Some(stored) = &mut meta {
                    stored.received = received;
                    let _ = job::write_meta(&meta_file, stored);
                }
                last_meta = Instant::now();
            }
        }

        let _ = file.flush();
        drop(file);
        if total > 0 && received < total {
            fail(
                &tx,
                &job.id,
                format!("connection ended early ({received}/{total})"),
            );
            return;
        }

        let target = job::unique_target(&job.folder, &job.filename);
        if let Err(err) = std::fs::rename(&part, &target) {
            fail(&tx, &job.id, format!("finalize: {err}"));
            return;
        }
        let _ = std::fs::remove_dir_all(&parts);
        emit(
            &tx,
            Event::Progress {
                id: job.id.clone(),
                received,
                total,
                speed_bps: 0,
            },
        );
        emit_state(&tx, &job.id, EngineState::Done);
        return;
    }
}
