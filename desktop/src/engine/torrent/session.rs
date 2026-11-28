//! One process-wide librqbit session shared by every torrent job.
//!
//! librqbit is async, so the session lives on its own tokio runtime that is
//! created once and kept for the lifetime of the app. Callers stand on a
//! plain queue thread and block on that runtime; never call into this module
//! from inside it (that would panic on a nested `block_on`).

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use librqbit::api::TorrentIdOrHash;
use librqbit::{
    AddTorrentOptions, AddTorrentResponse, ManagedTorrent, Session, SessionOptions,
};

use super::intake::TorrentSource;

/// Adding a magnet waits for metadata; without peers that never arrives, so
/// give up instead of parking a queue thread forever.
const ADD_TIMEOUT: Duration = Duration::from_secs(120);

/// Both starts are one-shot: if they fail, every torrent job reports the same
/// error until the app is restarted.
static RUNTIME: OnceLock<Result<tokio::runtime::Runtime, String>> = OnceLock::new();
static SESSION: OnceLock<Result<Arc<Session>, String>> = OnceLock::new();

fn runtime() -> Result<&'static tokio::runtime::Runtime, String> {
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .thread_name("lotus-torrent")
                .build()
                .map_err(|err| format!("cannot start the torrent runtime: {err}"))
        })
        .as_ref()
        .map_err(|err| err.clone())
}

fn session() -> Result<&'static Arc<Session>, String> {
    SESSION
        .get_or_init(|| {
            // librqbit's own state (piece checks for fastresume), not the
            // downloads: every torrent brings its own output folder.
            let folder = crate::core::settings::settings_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("torrent");
            std::fs::create_dir_all(&folder)
                .map_err(|err| format!("cannot create {}: {err}", folder.display()))?;

            let policy = super::policy();
            let mut opts = SessionOptions {
                // lotus-dm keeps the job list in queue.json, so librqbit's own
                // session dump would be a second, conflicting source of truth.
                persistence: None,
                // Re-check existing pieces instead of re-downloading them.
                fastresume: true,
                client_name_and_version: Some(format!("LotusDM/{}", env!("CARGO_PKG_VERSION"))),
                ..Default::default()
            };
            // Peer discovery is session-wide, so it is fixed here rather than
            // per torrent; `None` is librqbit's way of disabling the DHT.
            if !policy.dht {
                opts.dht = None;
            }
            opts.disable_local_service_discovery = !policy.lsd;

            runtime()?
                .block_on(Session::new_with_opts(folder, opts))
                .map_err(|err| format!("cannot start the torrent engine: {err:#}"))
        })
        .as_ref()
        .map_err(|err| err.clone())
}

/// Add a torrent into `folder` and wait until its metadata is known.
pub fn add(source: &TorrentSource, folder: &str) -> Result<Arc<ManagedTorrent>, String> {
    let session = session()?;
    let opts = AddTorrentOptions {
        // The queue decides what to download, and resuming or re-adding an
        // already complete torrent needs overwriting.
        overwrite: true,
        output_folder: Some(folder.to_string()),
        ..Default::default()
    };
    let payload = source.to_add()?;

    let response = runtime()?.block_on(async {
        match tokio::time::timeout(ADD_TIMEOUT, session.add_torrent(payload, Some(opts))).await {
            Ok(result) => result.map_err(|err| format!("cannot add torrent: {err:#}")),
            Err(_) => Err(
                "gave up waiting for the torrent metadata; no peers answered the magnet link"
                    .to_string(),
            ),
        }
    })?;

    match response {
        AddTorrentResponse::Added(_, torrent)
        | AddTorrentResponse::AlreadyManaged(_, torrent) => Ok(torrent),
        AddTorrentResponse::ListOnly(_) => Err("torrent was listed only, nothing to download".into()),
    }
}

pub fn pause(torrent: &Arc<ManagedTorrent>) -> Result<(), String> {
    runtime()?
        .block_on(session()?.pause(torrent))
        .map_err(|err| format!("cannot pause torrent: {err:#}"))
}

pub fn resume(torrent: &Arc<ManagedTorrent>) -> Result<(), String> {
    runtime()?
        .block_on(session()?.unpause(torrent))
        .map_err(|err| format!("cannot resume torrent: {err:#}"))
}

/// Drop the torrent from the session but keep what was downloaded, so the
/// queue can resume or retry it later.
pub fn forget(torrent: &Arc<ManagedTorrent>) -> Result<(), String> {
    let id = TorrentIdOrHash::Id(torrent.id());
    runtime()?
        .block_on(session()?.delete(id, false))
        .map_err(|err| format!("cannot remove torrent: {err:#}"))
}
