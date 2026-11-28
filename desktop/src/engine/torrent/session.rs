use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::Duration;

use super::rqbitapi::api::TorrentIdOrHash;
use super::rqbitapi::{
    AddTorrentOptions, AddTorrentResponse, ManagedTorrent, Session, SessionOptions,
};

use super::intake::TorrentSource;
use super::{EncryptionMode, Policy};

const ADD_TIMEOUT: Duration = Duration::from_secs(120);

type Key = (EncryptionMode, bool, bool);

fn key(policy: &Policy) -> Key {
    (policy.encryption, policy.dht, policy.lsd)
}

struct Engine {
    session: Arc<Session>,
    key: Key,
    live: usize,
}

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

fn replaceable(running: Key, live: usize, policy: &Policy) -> bool {
    running != key(policy) && live == 0
}

pub fn policy_changed(policy: &Policy) -> bool {
    match ENGINE.try_lock() {
        Ok(slot) => slot
            .as_ref()
            .is_some_and(|running| running.key != key(policy)),
        Err(_) => false,
    }
}

fn start(policy: &Policy) -> Result<Arc<Session>, String> {
    let folder = crate::core::settings::settings_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("torrent");
    std::fs::create_dir_all(&folder)
        .map_err(|err| format!("cannot create {}: {err}", folder.display()))?;

    let mut opts = SessionOptions {
        persistence: None,
        fastresume: true,
        client_name_and_version: Some(format!("LotusDM/{}", env!("CARGO_PKG_VERSION"))),
        ..Default::default()
    };
    if !policy.dht {
        opts.dht = None;
    }
    opts.disable_local_service_discovery = !policy.lsd;
    super::apply_mse(&mut opts, policy.encryption);

    runtime()?
        .block_on(Session::new_with_opts(folder, opts))
        .map_err(|err| format!("cannot start the torrent engine: {err:#}"))
}

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

struct Lease {
    session: Arc<Session>,
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

fn current() -> Result<Arc<Session>, String> {
    engine()
        .as_ref()
        .map(|running| running.session.clone())
        .ok_or_else(|| "the torrent engine is not running".to_string())
}

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

pub fn add(source: &TorrentSource, folder: &str) -> Result<Arc<ManagedTorrent>, String> {
    let mut lease = acquire(&super::policy())?;
    let opts = AddTorrentOptions {
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
        assert!(!replaceable(plaintext, 2, &wanted));

        assert!(replaceable(same, 0, &Policy { dht: false, ..wanted }));
        assert!(replaceable(same, 0, &Policy { lsd: false, ..wanted }));
    }
}
