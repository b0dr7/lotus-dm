use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::core::settings::settings_dir;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    Queued,
    Running,
    Paused,
    Done,
    Failed,
}

impl JobStatus {
    pub fn label(self) -> &'static str {
        match self {
            JobStatus::Queued => "Queued",
            JobStatus::Running => "Running",
            JobStatus::Paused => "Paused",
            JobStatus::Done => "Done",
            JobStatus::Failed => "Failed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DownloadJob {
    pub id: String,
    pub url: String,
    pub filename: String,
    pub folder: String,
    pub status: JobStatus,
    pub received_bytes: u64,
    pub total_bytes: u64,
    pub error: Option<String>,
    #[serde(default)]
    pub no_resume: bool,
    #[serde(default)]
    pub speed_limit_kbps: u64,
    #[serde(skip)]
    pub speed_bps: u64,
}

impl DownloadJob {
    pub fn new_queued(url: String, filename: String, folder: String) -> Self {
        Self {
            id: next_id(),
            url,
            filename,
            folder,
            status: JobStatus::Queued,
            received_bytes: 0,
            total_bytes: 0,
            error: None,
            no_resume: false,
            speed_limit_kbps: 0,
            speed_bps: 0,
        }
    }
}

static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or(0);
    let counter = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{millis}-{counter}")
}

pub fn queue_path() -> Option<PathBuf> {
    settings_dir().map(|dir| dir.join("queue.json"))
}

pub fn load() -> Vec<DownloadJob> {
    let Some(path) = queue_path() else {
        return Vec::new();
    };
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

pub fn save(jobs: &[DownloadJob]) -> Result<(), String> {
    let dir = settings_dir().ok_or_else(|| "no config directory".to_string())?;
    std::fs::create_dir_all(&dir).map_err(|err| format!("create {}: {err}", dir.display()))?;
    let json = serde_json::to_string_pretty(jobs).map_err(|err| err.to_string())?;
    std::fs::write(dir.join("queue.json"), json).map_err(|err| err.to_string())
}

pub fn derive_filename(url: &str) -> String {
    // A magnet link has no path to name the row with.
    if url.trim_start().to_ascii_lowercase().starts_with("magnet:") {
        return "torrent".to_string();
    }
    let path = url.split(['?', '#']).next().unwrap_or("");
    let last = path.rsplit('/').next().unwrap_or("").trim();
    if last.is_empty() {
        "download".to_string()
    } else {
        last.to_string()
    }
}

pub fn host_of(url: &str) -> String {
    let after_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let authority = after_scheme.split(['/', '?', '#']).next().unwrap_or("");
    let host_port = authority.rsplit('@').next().unwrap_or(authority);
    let host = if host_port.starts_with('[') {
        host_port.split(']').next().unwrap_or(host_port)
    } else {
        host_port.split(':').next().unwrap_or(host_port)
    };
    if host.is_empty() {
        url.to_string()
    } else {
        host.to_string()
    }
}
