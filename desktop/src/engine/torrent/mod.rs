//! BitTorrent engine for the desktop app, built on `librqbit`.
//!
//! It shares the HTTP engine's contract: a queue job
//! (`crate::core::queue::DownloadJob`) goes in, `engine::Event`s come out,
//! and `engine::Control` carries pause/cancel. Nothing in this module knows
//! about egui or any other UI type, so the queue and the UI stay decoupled
//! from the torrent stack.
//!
//! Intake accepts magnet links, http(s) links to a `.torrent` file and local
//! `.torrent` files; `engine::spawn_download` routes every such job here.

use std::sync::atomic::{AtomicU8, Ordering};

use serde::{Deserialize, Serialize};

pub mod intake;
pub mod session;
pub mod worker;

/// MSE/PE (encrypted BitTorrent peer connections) policy, mirroring the
/// three modes FDM exposes.
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
    fn index(self) -> u8 {
        match self {
            EncryptionMode::Disabled => 0,
            EncryptionMode::Enabled => 1,
            EncryptionMode::Forced => 2,
        }
    }

    fn from_index(index: u8) -> Self {
        match index {
            1 => EncryptionMode::Enabled,
            2 => EncryptionMode::Forced,
            _ => EncryptionMode::Disabled,
        }
    }
}

/// librqbit implements no MSE/PE handshake in any release so far: there is no
/// RC4/Diffie-Hellman layer, and the upstream work is still an open pull
/// request (ikatson/rqbit#633). Encrypted peer traffic is therefore not
/// possible here.
///
/// Consequences, kept explicit so nothing is silently downgraded:
/// - `Disabled` runs plaintext, as asked.
/// - `Enabled` behaves like FDM's "enabled": plaintext is allowed as the
///   fallback, so it is honoured, but it does not add encryption either.
/// - `Forced` refuses to start the torrent instead of leaking plaintext.
pub const MSE_SUPPORTED: bool = false;

static ENCRYPTION: AtomicU8 = AtomicU8::new(0);

pub fn encryption() -> EncryptionMode {
    EncryptionMode::from_index(ENCRYPTION.load(Ordering::Relaxed))
}

/// Set the process-wide MSE policy. Takes effect for torrents started after
/// the call; running torrents keep the mode they were admitted with.
pub fn set_encryption(mode: EncryptionMode) {
    ENCRYPTION.store(mode.index(), Ordering::Relaxed);
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

/// True when the queue URL should be handled by the torrent engine.
pub fn is_torrent(raw: &str) -> bool {
    intake::parse(raw).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encryption_modes_round_trip() {
        for mode in [
            EncryptionMode::Disabled,
            EncryptionMode::Enabled,
            EncryptionMode::Forced,
        ] {
            assert_eq!(EncryptionMode::from_index(mode.index()), mode);
        }
    }

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
