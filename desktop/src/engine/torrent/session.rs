//! One process-wide librqbit session shared by every torrent job.
//!
//! librqbit is async, so the session lives on its own tokio runtime that is
//! created once and kept for the lifetime of the app. Callers stand on a
//! plain queue thread and block on that runtime; never call into this module
//! from inside it (that would panic on a nested `block_on`).

use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use librqbit::api::TorrentIdOrHash;
use librqbit::{
    AddTorrentOptions, AddTorrentResponse, ManagedTorrent, Session, SessionOptions, TorrentStats,
};

use super::intake::TorrentSource;

/// Adding a magnet waits for metadata; without peers that never arrives, so
/// give up instead of parking a queue thread forever.
const ADD_TIMEOUT: Duration = Duration::from_secs(120);

static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
static SESSION: Mutex<Option<Arc<Session>>> = Mutex::new(None);

/// Handle to one added torrent, kept by the job's worker thread.
pub struct TorrentHandle {
    torrent: Arc<ManagedTorrent>,
    fallback_name: String,
}

impl TorrentHandle {
    pub fn name(&self) -> String {
        self.torrent
            .name()
            .unwrap_or_else(|| self.fallback_name.clone())
    }

    pub fn stats(&self) -> TorrentStats {
        self.torrent.stats()
    }
}

fn runtime() -> Result<&'static tokio::runtime::Runtime, String> {
    if let Some(rt) = RUNTIME.get() {
        return Ok(rt);
    }
    let built = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .thread_name("lotus-torrent")
        .build()
        .map_err(|err| format!("cannot start the torrent runtime: {err}"))?;
    let _ = RUNTIME.set(built);
    RUNTIME
        .get()
        .ok_or_else(|| "torrent runtime unavailable".to_string())
}

fn default_folder() -> PathBuf {
    dirs::download_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join("Downloads")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("LotusDM")
}

/// Start the session on first use. A failed start is not cached, so a later
/// job can retry (for example after a port conflict clears).
fn session() -> Result<(Arc<Session>, &'static tokio::runtime::Runtime), String> {
    let rt = runtime()?;

    let mut guard = SESSION.lock().unwrap_or_else(|err| err.into_inner());
    if let Some(session) = guard.as_ref() {
        return Ok((Arc::clone(session), rt));
    }

    let folder = default_folder();
    std::fs::create_dir_all(&folder)
        .map_err(|err| format!("cannot create {}: {err}", folder.display()))?;

    let opts = SessionOptions {
        // lotus-dm keeps the job list in queue.json, so librqbit's own
        // session dump would only be a second, conflicting source of truth.
        persistence: None,
        // Re-check existing pieces instead of re-downloading them.
        fastresume: true,
        client_name_and_version: Some(format!("LotusDM/{}", env!("CARGO_PKG_VERSION"))),
        ..Default::default()
    };

    let started = rt
        .block_on(Session::new_with_opts(folder, opts))
        .map_err(|err| format!("cannot start the torrent engine: {err:#}"))?;

    *guard = Some(Arc::clone(&started));
    Ok((started, rt))
}

/// Add a torrent into `folder` and wait until its metadata is known.
pub fn add(source: &TorrentSource, folder: &str, paused: bool) -> Result<TorrentHandle, String> {
    let (session, rt) = session()?;
    let payload = source.to_add()?;
    let opts = AddTorrentOptions {
        paused,
        // The queue decides what to download; overwriting is required for
        // resuming or re-adding an already complete torrent.
        overwrite: true,
        output_folder: Some(folder.to_string()),
        ..Default::default()
    };

    let response = rt.block_on(async move {
        match tokio::time::timeout(ADD_TIMEOUT, session.add_torrent(payload, Some(opts))).await {
            Ok(result) => result.map_err(|err| format!("cannot add torrent: {err:#}")),
            Err(_) => Err(
                "gave up waiting for the torrent metadata; no peers answered the magnet link"
                    .to_string(),
            ),
        }
    })?;

    let torrent = match response {
        AddTorrentResponse::Added(_, torrent)
        | AddTorrentResponse::AlreadyManaged(_, torrent) => torrent,
        AddTorrentResponse::ListOnly(_) => {
            return Err("torrent was listed only, nothing to download".to_string())
        }
    };

    Ok(TorrentHandle {
        torrent,
        fallback_name: source.display_name(),
    })
}

pub fn set_paused(handle: &TorrentHandle, paused: bool) -> Result<(), String> {
    let (session, rt) = session()?;
    let result = if paused {
        rt.block_on(session.pause(&handle.torrent))
    } else {
        rt.block_on(session.unpause(&handle.torrent))
    };
    result.map_err(|err| {
        format!(
            "cannot {} torrent: {err:#}",
            if paused { "pause" } else { "resume" }
        )
    })
}

/// Drop the torrent from the session. `delete_files` also removes what was
/// downloaded, which is only safe when the job is being cancelled for good.
pub fn remove(handle: &TorrentHandle, delete_files: bool) -> Result<(), String> {
    let (session, rt) = session()?;
    let id = TorrentIdOrHash::Id(handle.torrent.id());
    rt.block_on(session.delete(id, delete_files))
        .map_err(|err| format!("cannot remove torrent: {err:#}"))
}
