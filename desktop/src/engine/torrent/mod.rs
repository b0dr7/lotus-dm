//! BitTorrent engine for the desktop app, built on `librqbit`.
//!
//! It shares the HTTP engine's contract: a queue job
//! (`crate::core::queue::DownloadJob`) goes in, `engine::Event`s come out,
//! and `engine::Control` carries pause/cancel. Nothing here knows about egui
//! or any other UI type.
//!
//! Intake accepts magnet links, http(s) links to a `.torrent` file and local
//! `.torrent` files; `engine::spawn_download` routes every such job here.
//!
//! Two FDM switches have no counterpart in librqbit and are therefore always
//! in one state, with no dead toggle: sequential download (librqbit only ever
//! fetches pieces in file order) and PEX (always enabled, no API switch).

use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};

pub mod intake;
pub mod session;
pub mod worker;

/// MSE/PE (encrypted BitTorrent peer connections) policy, mirroring the three
/// modes FDM exposes.
///
/// The encryption itself comes from the rqbit MSE pull request
/// (ikatson/rqbit#633), which this build depends on as a pinned git revision
/// because no librqbit release contains it yet: `Disabled` is plaintext only,
/// `Enabled` prefers encrypted peers and redials plaintext when a peer cannot
/// complete the handshake, and `Forced` drops peers that do not.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncryptionMode {
    /// Plaintext peer connections only.
    #[default]
    Disabled,
    /// Prefer encrypted peers, fall back to plaintext when a peer cannot.
    Enabled,
    /// Encrypted peers only; never run unencrypted traffic.
    Forced,
}

impl EncryptionMode {
    pub fn label(self) -> &'static str {
        match self {
            EncryptionMode::Disabled => "Disabled",
            EncryptionMode::Enabled => "Enabled",
            EncryptionMode::Forced => "Forced",
        }
    }
}

/// Hand the encryption mode to librqbit. Only the 64-bit build carries MSE:
/// the pinned rqbit pull request's DH-768 code is 64-bit only, so 32-bit
/// builds use the released crate, which has no `mse_mode` at all.
#[cfg(target_pointer_width = "64")]
pub fn apply_mse(opts: &mut librqbit::SessionOptions, mode: EncryptionMode) {
    use librqbit::MseMode;

    opts.mse_mode = match mode {
        EncryptionMode::Disabled => MseMode::Disabled,
        EncryptionMode::Enabled => MseMode::Enabled,
        EncryptionMode::Forced => MseMode::Forced,
    };
}

#[cfg(not(target_pointer_width = "64"))]
pub fn apply_mse(_opts: &mut librqbit::SessionOptions, _mode: EncryptionMode) {}

/// What the queue asks of the torrent engine: the encryption policy, the
/// peer-discovery switches and how long to seed.
#[derive(Clone, Copy, Debug)]
pub struct Policy {
    pub encryption: EncryptionMode,
    /// Find peers through the DHT. Off means magnets without a tracker cannot
    /// resolve, and `.torrent` jobs only use trackers and their peer list.
    pub dht: bool,
    /// Local Service Discovery (BEP-14) on the LAN.
    pub lsd: bool,
    /// Stop seeding at this share ratio (uploaded / downloaded). 0 together
    /// with `seed_minutes` 0 means "stop as soon as the files are complete".
    pub seed_ratio: f64,
    /// Stop seeding after this many minutes; 0 for no time limit.
    pub seed_minutes: u64,
}

const DEFAULT_POLICY: Policy = Policy {
    encryption: EncryptionMode::Disabled,
    dht: true,
    lsd: true,
    seed_ratio: 0.0,
    seed_minutes: 0,
};

impl Default for Policy {
    fn default() -> Self {
        DEFAULT_POLICY
    }
}

static POLICY: Mutex<Policy> = Mutex::new(DEFAULT_POLICY);

pub fn policy() -> Policy {
    *POLICY.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Set the policy. rqbit takes the encryption mode and the DHT/LSD switches
/// per session, so when they change an idle session is rebuilt in the
/// background and the next torrent uses the new settings. Torrents already
/// running keep the session they started with, which is the only way to avoid
/// interrupting them.
pub fn set_policy(new: Policy) {
    *POLICY.lock().unwrap_or_else(PoisonError::into_inner) = new;
    // Stopping a librqbit session sleeps for a second, so the rebuild happens
    // off the UI thread. Seed limits apply live and never reach this path.
    if session::policy_changed(&new) {
        std::thread::spawn(session::refresh_policy);
    }
}

/// True when the queue URL belongs to the torrent engine.
pub fn is_torrent(raw: &str) -> bool {
    intake::parse(raw).is_some()
}

#[cfg(all(test, target_pointer_width = "64"))]
mod tests {
    use super::*;

    #[test]
    fn encryption_modes_map_onto_rqbits_mse_modes() {
        let mut opts = librqbit::SessionOptions::default();
        apply_mse(&mut opts, EncryptionMode::Forced);
        assert_eq!(opts.mse_mode, librqbit::MseMode::Forced);
        apply_mse(&mut opts, EncryptionMode::Disabled);
        assert_eq!(opts.mse_mode, librqbit::MseMode::Disabled);
    }
}
