use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Meta {
    pub url: String,
    pub total: u64,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub received: u64,
}

#[derive(Clone, Debug)]
pub enum EngineState {
    Queued,
    Running,
    Paused,
    Done,
    Failed(String),
}

pub fn parts_dir(folder: &str, id: &str) -> PathBuf {
    Path::new(folder).join(".lotus-parts").join(id)
}

pub fn part_path(folder: &str, id: &str) -> PathBuf {
    parts_dir(folder, id).join("part0")
}

pub fn meta_path(folder: &str, id: &str) -> PathBuf {
    parts_dir(folder, id).join("meta.json")
}

pub fn load_meta(path: &Path) -> Option<Meta> {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
}

pub fn write_meta(path: &Path, meta: &Meta) -> std::io::Result<()> {
    let json = serde_json::to_vec(meta).map_err(std::io::Error::other)?;
    std::fs::write(path, json)
}

pub fn part_len(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// True when the resume validators differ from the stored ones (or there is
/// nothing stored to compare against).
pub fn validators_changed(
    meta: &Option<Meta>,
    etag: &Option<String>,
    last_modified: &Option<String>,
) -> bool {
    let Some(meta) = meta else {
        return true;
    };
    if let (Some(stored), Some(current)) = (&meta.etag, etag) {
        if stored != current {
            return true;
        }
    }
    if let (Some(stored), Some(current)) = (&meta.last_modified, last_modified) {
        if stored != current {
            return true;
        }
    }
    false
}

/// Total size from Content-Range (resume) or Content-Length (full body),
/// falling back to the stored meta value.
pub fn header_total(resp: &ureq::Response, meta: &Option<Meta>) -> u64 {
    if let Some(range) = resp.header("content-range") {
        if let Some((_, total)) = range.rsplit_once('/') {
            if let Ok(total) = total.trim().parse::<u64>() {
                return total;
            }
        }
    }
    if resp.status() == 200 {
        if let Some(len) = resp
            .header("content-length")
            .and_then(|v| v.parse::<u64>().ok())
        {
            return len;
        }
    }
    meta.as_ref().map(|m| m.total).unwrap_or(0)
}

/// Final path in the download folder, avoiding clobbering an existing file
/// by appending " (n)" before the extension.
pub fn unique_target(folder: &str, filename: &str) -> PathBuf {
    let base = Path::new(folder).join(filename);
    if !base.exists() {
        return base;
    }
    let name = Path::new(filename);
    let stem = name.file_stem().unwrap_or(name.as_os_str());
    let ext = name.extension();
    for i in 1..1000u32 {
        let mut candidate = stem.to_string_lossy().into_owned();
        candidate.push_str(&format!(" ({i})"));
        if let Some(ext) = ext {
            candidate.push('.');
            candidate.push_str(&ext.to_string_lossy());
        }
        let path = Path::new(folder).join(candidate);
        if !path.exists() {
            return path;
        }
    }
    base
}

/// Sleeps reads so the average rate stays at limit_kbps (0 = unlimited).
pub struct RateLimiter {
    bytes_per_sec: u64,
    started: Instant,
    total: u64,
}

impl RateLimiter {
    pub fn new(limit_kbps: u64) -> Self {
        Self {
            bytes_per_sec: limit_kbps.saturating_mul(1024),
            started: Instant::now(),
            total: 0,
        }
    }

    pub fn throttle(&mut self, n: u64) {
        if self.bytes_per_sec == 0 {
            return;
        }
        self.total = self.total.saturating_add(n);
        let expected = self.total as f64 / self.bytes_per_sec as f64;
        let target = self.started + Duration::from_secs_f64(expected);
        let now = Instant::now();
        if target > now {
            thread::sleep(target - now);
        }
    }
}
