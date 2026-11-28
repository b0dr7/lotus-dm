use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};

#[cfg(target_pointer_width = "64")]
pub use librqbit as rqbitapi;
#[cfg(target_pointer_width = "32")]
pub use librqbit_release as rqbitapi;

pub mod intake;
pub mod session;
pub mod worker;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncryptionMode {
    #[default]
    Disabled,
    Enabled,
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

#[cfg(target_pointer_width = "64")]
pub fn apply_mse(opts: &mut rqbitapi::SessionOptions, mode: EncryptionMode) {
    use rqbitapi::MseMode;

    opts.mse_mode = match mode {
        EncryptionMode::Disabled => MseMode::Disabled,
        EncryptionMode::Enabled => MseMode::Enabled,
        EncryptionMode::Forced => MseMode::Forced,
    };
}

#[cfg(not(target_pointer_width = "64"))]
pub fn apply_mse(_opts: &mut rqbitapi::SessionOptions, _mode: EncryptionMode) {}

#[derive(Clone, Copy, Debug)]
pub struct Policy {
    pub encryption: EncryptionMode,
    pub dht: bool,
    pub lsd: bool,
    pub seed_ratio: f64,
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

pub fn set_policy(new: Policy) {
    *POLICY.lock().unwrap_or_else(PoisonError::into_inner) = new;
    if session::policy_changed(&new) {
        std::thread::spawn(session::refresh_policy);
    }
}

pub fn is_torrent(raw: &str) -> bool {
    intake::parse(raw).is_some()
}

#[cfg(all(test, target_pointer_width = "64"))]
mod tests {
    use super::*;

    #[test]
    fn encryption_modes_map_onto_rqbits_mse_modes() {
        let mut opts = rqbitapi::SessionOptions::default();
        apply_mse(&mut opts, EncryptionMode::Forced);
        assert_eq!(opts.mse_mode, rqbitapi::MseMode::Forced);
        apply_mse(&mut opts, EncryptionMode::Disabled);
        assert_eq!(opts.mse_mode, rqbitapi::MseMode::Disabled);
    }
}
