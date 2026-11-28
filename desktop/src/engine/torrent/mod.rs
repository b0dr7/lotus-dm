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

/// librqbit implements no MSE/PE handshake in any release so far: there is no
/// RC4/Diffie-Hellman layer, and the upstream work is still an open pull
/// request (ikatson/rqbit#633). Encrypted peer traffic is not possible here.
///
/// The modes are still honoured as far as they can be, so nothing is silently
/// downgraded: `Disabled` runs plaintext, `Enabled` allows the plaintext
/// fallback it already promises, and `Forced` refuses the torrent.
pub const MSE_SUPPORTED: bool = false;

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

/// Set the policy. The DHT/LSD switches are session-wide, so a change applies
/// the next time the app starts; running torrents keep what they started with.
pub fn set_policy(new: Policy) {
    *POLICY.lock().unwrap_or_else(PoisonError::into_inner) = new;
}

/// Gate the worker runs before touching the network.
pub(crate) fn check_encryption() -> Result<(), String> {
    if policy().encryption == EncryptionMode::Forced && !MSE_SUPPORTED {
        return Err(
            "encrypted peer connections (MSE) are unavailable in this build, \
             so forced encryption would still send plaintext; set torrent \
             encryption to Enabled or Disabled to download this torrent"
                .to_string(),
        );
    }
    Ok(())
}

/// True when the queue URL belongs to the torrent engine.
pub fn is_torrent(raw: &str) -> bool {
    intake::parse(raw).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forced_encryption_is_refused_without_mse() {
        set_policy(Policy {
            encryption: EncryptionMode::Forced,
            ..Policy::default()
        });
        assert!(check_encryption().is_err());
        set_policy(Policy::default());
        assert!(check_encryption().is_ok());
    }
}
