use std::io::{Read, Write};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use suppaftp::types::FileType;
use suppaftp::FtpStream;

use crate::core::queue::DownloadJob;

use super::job::{part_path, parts_dir, unique_target, RateLimiter};
use super::{
    cancelled_cleanup, emit, emit_state, fail, throttle_global, Control, EngineState, Event,
};

const CHUNK: usize = 64 * 1024;
const TICK: Duration = Duration::from_millis(250);
const PAUSE_POLL: Duration = Duration::from_millis(100);
const DEFAULT_PORT: u16 = 21;

pub fn is_ftp(raw: &str) -> bool {
    raw.trim()
        .get(..6)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("ftp://"))
}

#[derive(Debug, PartialEq, Eq)]
struct Target {
    address: String,
    user: String,
    password: String,
    path: String,
}

impl Target {
    fn parse(url: &str) -> Option<Self> {
        let text = url.trim();
        if !is_ftp(text) {
            return None;
        }
        let rest = &text[6..];
        let (authority, path) = match rest.find('/') {
            Some(index) => (&rest[..index], &rest[index..]),
            None => (rest, ""),
        };
        let (userinfo, host) = match authority.rsplit_once('@') {
            Some((userinfo, host)) => (Some(userinfo), host),
            None => (None, authority),
        };
        if host.is_empty() || path.len() < 2 {
            return None;
        }
        let (user, password) = match userinfo {
            Some(info) => match info.split_once(':') {
                Some((user, password)) => (user.to_string(), password.to_string()),
                None => (info.to_string(), String::new()),
            },
            None => ("anonymous".to_string(), "lotus@".to_string()),
        };
        let address = if host.contains(':') {
            host.to_string()
        } else {
            format!("{host}:{DEFAULT_PORT}")
        };
        Some(Self {
            address,
            user,
            password,
            path: path.to_string(),
        })
    }
}

pub(super) fn run(job: DownloadJob, control: Arc<Control>, tx: Sender<Event>) {
    let Some(target) = Target::parse(&job.url) else {
        fail(&tx, &job.id, "not a usable ftp:// url".to_string());
        return;
    };

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

    emit_state(&tx, &job.id, EngineState::Running);
    let mut stream = match FtpStream::connect(target.address.as_str()) {
        Ok(stream) => stream,
        Err(err) => {
            fail(&tx, &job.id, format!("ftp connect: {err}"));
            return;
        }
    };
    if let Err(err) = stream.login(&target.user, &target.password) {
        fail(&tx, &job.id, format!("ftp login: {err}"));
        return;
    }
    let _ = stream.transfer_type(FileType::Binary);
    let total = stream.size(target.path.as_str()).unwrap_or(0) as u64;

    let mut file = match std::fs::File::create(&part) {
        Ok(file) => file,
        Err(err) => {
            fail(&tx, &job.id, format!("open part: {err}"));
            return;
        }
    };
    let mut transfer = match stream.retr_as_stream(target.path.as_str()) {
        Ok(transfer) => transfer,
        Err(err) => {
            fail(&tx, &job.id, format!("ftp retrieve: {err}"));
            return;
        }
    };

    let job_limiter =
        (job.speed_limit_kbps > 0).then(|| Mutex::new(RateLimiter::new(job.speed_limit_kbps)));
    let mut buf = vec![0u8; CHUNK];
    let mut received = 0u64;
    let mut last_tick = Instant::now();
    let mut last_bytes = 0u64;

    loop {
        if control.cancelled() {
            drop(transfer);
            drop(file);
            cancelled_cleanup(&tx, &job.id, &parts);
            return;
        }
        if control.paused() {
            emit_state(&tx, &job.id, EngineState::Paused);
            while control.paused() && !control.cancelled() {
                thread::sleep(PAUSE_POLL);
            }
            if control.cancelled() {
                drop(transfer);
                drop(file);
                cancelled_cleanup(&tx, &job.id, &parts);
                return;
            }
            emit_state(&tx, &job.id, EngineState::Running);
            last_tick = Instant::now();
            last_bytes = received;
            continue;
        }

        let n = match transfer.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(err) => {
                fail(&tx, &job.id, format!("ftp read: {err}"));
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
    }

    let _ = file.flush();
    drop(file);
    if let Err(err) = transfer.finish() {
        fail(&tx, &job.id, format!("ftp: {err}"));
        return;
    }
    let _ = stream.quit();

    if total > 0 && received < total {
        fail(
            &tx,
            &job.id,
            format!("connection ended early ({received}/{total})"),
        );
        return;
    }

    let target_path = unique_target(&job.folder, &job.filename);
    if let Err(err) = std::fs::rename(&part, &target_path) {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_anonymous_and_credential_urls() {
        assert_eq!(
            Target::parse("ftp://ftp.gnu.org/gnu/bash/bash-5.2.tar.gz"),
            Some(Target {
                address: "ftp.gnu.org:21".to_string(),
                user: "anonymous".to_string(),
                password: "lotus@".to_string(),
                path: "/gnu/bash/bash-5.2.tar.gz".to_string(),
            })
        );
        assert_eq!(
            Target::parse("  FTP://bob:secret@example.com:2121/pub/file.iso  "),
            Some(Target {
                address: "example.com:2121".to_string(),
                user: "bob".to_string(),
                password: "secret".to_string(),
                path: "/pub/file.iso".to_string(),
            })
        );
    }

    #[test]
    fn rejects_other_schemes_and_empty_paths() {
        for url in [
            "http://example.com/file.iso",
            "ftps://example.com/file.iso",
            "ftp://example.com/",
            "ftp://example.com",
            "ftp:///file.iso",
        ] {
            assert!(Target::parse(url).is_none(), "{url}");
        }
        assert!(is_ftp("FTP://host/x"));
        assert!(!is_ftp("https://host/x"));
    }
}
