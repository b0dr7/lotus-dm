//! One process-wide librqbit session shared by every torrent job.
//!
//! librqbit is async, so the session lives on its own tokio runtime that is
//! created once and kept for the lifetime of the app. Callers stand on a
//! plain queue thread and block on that runtime; never call into this module
//! from inside it (that would panic on a nested `block_on`).
//!
//! rqbit reads the encryption mode, the DHT and LSD switches only when it
//! builds a session, so a settings change would need an app restart. Instead
//! the session is rebuilt as soon as nothing is running: at once from the
//! settings panel, or before the next torrent, while a download in flight
//! keeps what it started with.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::Duration;

use librqbit::api::TorrentIdOrHash;
use librqbit::{
    AddTorrentOptions, AddTorrentResponse, ManagedTorrent, Session, SessionOptions,
};

use super::intake::TorrentSource;
use super::{EncryptionMode, Policy};

/// Adding a magnet waits for metadata; without peers that never arrives, so
/// give up instead of parking a queue thread forever.
const ADD_TIMEOUT: Duration = Duration::from_secs(120);

/// The session-wide policy: everything rqbit only reads at session start.
type Key = (EncryptionMode, bool, bool);

fn key(policy: &Policy) -> Key {
    (policy.encryption, policy.dht, policy.lsd)
}

/// The live session, what it was built with, and how many torrents depend on
/// it. A session may only be replaced while `live` is 0.
struct Engine {
    session: Arc<Session>,
    key: Key,
    live: usize,
}

/// The runtime is kept across rebuilds; only the session is recreated.
static RUNTIME: OnceLock<Result<tokio::runtime::Runtime, String>> = OnceLock::new();
static ENGINE: Mutex<Option<Engine>> = Mutex::new(None);

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

fn engine() -> MutexGuard<'static, Option<Engine>> {
    ENGINE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// True when a session built with `running` cannot serve `policy` and nothing
/// is using it, so replacing it would not interrupt a download.
fn replaceable(running: Key, live: usize, policy: &Policy) -> bool {
    running != key(policy) && live == 0
}

/// True when `policy` differs from what the running session was built with.
/// Called on every settings change, so it never waits for a rebuild in flight:
/// the next torrent re-checks the policy anyway, so nothing is missed.
pub fn policy_changed(policy: &Policy) -> bool {
    match ENGINE.try_lock() {
        Ok(slot) => slot
            .as_ref()
            .is_some_and(|running| running.key != key(policy)),
        Err(_) => false,
    }
}

/// Create a session for `policy`. `None` is librqbit's way of disabling the
/// DHT; the other two switches have plain booleans.
fn start(policy: &Policy) -> Result<Arc<Session>, String> {
    // librqbit's own state (piece checks for fastresume), not the downloads:
    // every torrent brings its own output folder.
    let folder = crate::core::settings::settings_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("torrent");
    std::fs::create_dir_all(&folder)
        .map_err(|err| format!("cannot create {}: {err}", folder.display()))?;

    let mut opts = SessionOptions {
        // lotus-dm keeps the job list in queue.json, so librqbit's own session
        // dump would be a second, conflicting source of truth.
        persistence: None,
        // Re-check existing pieces instead of re-downloading them.
        fastresume: true,
        client_name_and_version: Some(format!("LotusDM/{}", env!("CARGO_PKG_VERSION"))),
        ..Default::default()
    };
    if !policy.dht {
        opts.dht = None;
    }
    opts.disable_local_service_discovery = !policy.lsd;
    opts.mse_mode = policy.encryption.into();

    runtime()?
        .block_on(Session::new_with_opts(folder, opts))
        .map_err(|err| format!("cannot start the torrent engine: {err:#}"))
}

/// Stop `old` and start a session for `policy` in its place. Stopping releases
/// the listen socket, so the new session has to bind the same port back; that
/// can lose a race with the closing socket, hence the retries.
fn restart(old: Engine, policy: &Policy) -> Result<Engine, String> {
    runtime()?.block_on(old.session.stop());
    let mut err = String::from("cannot start the torrent engine");
    for attempt in 0..3 {
        std::thread::sleep(Duration::from_millis(if attempt == 0 { 0 } else { 300 }));
        match start(policy) {
            Ok(session) => return Ok(Engine { session, key: key(policy), live: 0 }),
            Err(problem) => err = problem,
        }
    }
    Err(err)
}

/// A session lent to a new torrent and counted as live until that torrent is
/// forgotten, so a rebuild cannot stop it mid-add. Dropping the lease without
/// handing it over releases the count again, which is what a failed add does.
struct Lease {
    session: Arc<Session>,
    /// Set once the queue owns the torrent; `forget` releases the count then.
    handed_over: bool,
}

impl Drop for Lease {
    fn drop(&mut self) {
        if self.handed_over {
            return;
        }
        let mut slot = engine();
        if let Some(running) = slot.as_mut() {
            running.live = running.live.saturating_sub(1);
        }
    }
}

/// The session to use for a new torrent, rebuilt first when the policy says so
/// and counted as live before the lock is released.
///
/// The lock is held across the rebuild so two queue threads cannot start
/// competing sessions; the other callers are torrent threads that only take it
/// for a moment.
fn acquire(policy: &Policy) -> Result<Lease, String> {
    let mut slot = engine();
    let stale = match slot.as_ref() {
        Some(running) => replaceable(running.key, running.live, policy),
        None => true,
    };
    if stale {
        let replacement = match slot.take() {
            Some(old) => restart(old, policy)?,
            None => Engine {
                session: start(policy)?,
                key: key(policy),
                live: 0,
            },
        };
        *slot = Some(replacement);
    }
    let running = slot
        .as_mut()
        .ok_or_else(|| "the torrent engine is not running".to_string())?;
    running.live += 1;
    Ok(Lease { session: running.session.clone(), handed_over: false })
}

/// The session holding the torrents that are already running.
fn current() -> Result<Arc<Session>, String> {
    engine()
        .as_ref()
        .map(|running| running.session.clone())
        .ok_or_else(|| "the torrent engine is not running".to_string())
}

/// Rebuild an idle session after a policy change, so the next torrent and the
/// next incoming connection use the new mode without an app restart. No
/// session yet stays lazy: creating one here would open the listen socket and
/// the DHT on a settings click. A failed rebuild is left for the next torrent
/// to retry and report.
pub fn refresh_policy() {
    let policy = super::policy();
    let mut slot = engine();
    let stale = slot
        .as_ref()
        .is_some_and(|e| replaceable(e.key, e.live, &policy));
    if !stale {
        return;
    }
    let old = slot.take().expect("a running session was inspected above");
    *slot = restart(old, &policy).ok();
}

/// Add a torrent into `folder` and wait until its metadata is known.
pub fn add(source: &TorrentSource, folder: &str) -> Result<Arc<ManagedTorrent>, String> {
    let mut lease = acquire(&super::policy())?;
    let opts = AddTorrentOptions {
        // The queue decides what to download, and resuming or re-adding an
        // already complete torrent needs overwriting.
        overwrite: true,
        output_folder: Some(folder.to_string()),
        ..Default::default()
    };
    let payload = source.to_add()?;

    let response = runtime()?.block_on(async {
        match tokio::time::timeout(ADD_TIMEOUT, lease.session.add_torrent(payload, Some(opts)))
            .await
        {
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
            return Err("torrent was listed only, nothing to download".into());
        }
    };
    // From here until `forget` this torrent must not have its engine swapped
    // out from under it, so the lease stays counted instead of released.
    lease.handed_over = true;
    Ok(torrent)
}

pub fn pause(torrent: &Arc<ManagedTorrent>) -> Result<(), String> {
    runtime()?
        .block_on(current()?.pause(torrent))
        .map_err(|err| format!("cannot pause torrent: {err:#}"))
}

pub fn resume(torrent: &Arc<ManagedTorrent>) -> Result<(), String> {
    runtime()?
        .block_on(current()?.unpause(torrent))
        .map_err(|err| format!("cannot resume torrent: {err:#}"))
}

/// Drop the torrent from the session but keep what was downloaded, so the
/// queue can resume or retry it later. This also releases the session for a
/// rebuild, which is why the count is decremented even if the delete failed.
pub fn forget(torrent: &Arc<ManagedTorrent>) -> Result<(), String> {
    let id = TorrentIdOrHash::Id(torrent.id());
    let result = runtime()?
        .block_on(current()?.delete(id, false))
        .map_err(|err| format!("cannot remove torrent: {err:#}"));
    let mut slot = engine();
    if let Some(running) = slot.as_mut() {
        running.live = running.live.saturating_sub(1);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebuilds_only_when_the_key_changed_and_nothing_is_running() {
        let wanted = Policy {
            encryption: EncryptionMode::Forced,
            dht: true,
            lsd: true,
            ..Policy::default()
        };
        let same = key(&wanted);
        assert!(!replaceable(same, 0, &wanted));

        let plaintext = (EncryptionMode::Disabled, true, true);
        assert!(replaceable(plaintext, 0, &wanted));
        // A changed mode does not justify pulling the session out from under a
        // download that is already using it.
        assert!(!replaceable(plaintext, 2, &wanted));

        // The DHT and LSD switches are as session-wide as the encryption mode.
        assert!(replaceable(same, 0, &Policy { dht: false, ..wanted }));
        assert!(replaceable(same, 0, &Policy { lsd: false, ..wanted }));
    }
}
