//! BitTorrent engine for the desktop app, built on `librqbit`.
//!
//! It shares the HTTP engine's contract: a queue job
//! (`crate::core::queue::DownloadJob`) goes in, `engine::Event`s come out,
//! and `engine::Control` carries pause/cancel. Nothing here knows about egui
//! or any other UI type.
//!
//! Intake accepts magnet links, http(s) links to a `.torrent` file and local
//! `.torrent` files; `engine::spawn_download` routes every such job here.

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

/// librqbit implements no MSE/PE handshake in any release so far: there is no
/// RC4/Diffie-Hellman layer, and the upstream work is still an open pull
/// request (ikatson/rqbit#633). Encrypted peer traffic is not possible here.
///
/// The modes are still honoured as far as they can be, so nothing is silently
/// downgraded: `Disabled` runs plaintext, `Enabled` allows the plaintext
/// fallback it already promises, and `Forced` refuses the torrent.
pub const MSE_SUPPORTED: bool = false;

static ENCRYPTION: Mutex<EncryptionMode> = Mutex::new(EncryptionMode::Disabled);

pub fn encryption() -> EncryptionMode {
    *ENCRYPTION.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Set the MSE policy. Applies to torrents started after the call; running
/// torrents keep the mode they were admitted with.
pub fn set_encryption(mode: EncryptionMode) {
    *ENCRYPTION.lock().unwrap_or_else(PoisonError::into_inner) = mode;
}

/// Gate the worker runs before touching the network.
pub(crate) fn check_encryption() -> Result<(), String> {
    if encryption() == EncryptionMode::Forced && !MSE_SUPPORTED {
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
        set_encryption(EncryptionMode::Forced);
        assert!(check_encryption().is_err());
        set_encryption(EncryptionMode::Enabled);
        assert!(check_encryption().is_ok());
        set_encryption(EncryptionMode::Disabled);
        assert!(check_encryption().is_ok());
    }
}
