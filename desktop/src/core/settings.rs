use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::engine::torrent::{EncryptionMode, Policy};
use crate::ui::theme::ThemeMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TrafficMode {
    #[default]
    Custom,
    Low,
    Medium,
    High,
    Unlimited,
}

impl TrafficMode {
    pub fn label(self) -> &'static str {
        match self {
            TrafficMode::Custom => "Custom",
            TrafficMode::Low => "Low",
            TrafficMode::Medium => "Medium",
            TrafficMode::High => "High",
            TrafficMode::Unlimited => "Unlimited",
        }
    }
    pub fn suggested_kbps(self) -> u64 {
        match self {
            TrafficMode::Custom => 0,
            TrafficMode::Low => 256,
            TrafficMode::Medium => 1024,
            TrafficMode::High => 4096,
            TrafficMode::Unlimited => 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme_mode: ThemeMode,
    pub default_folder: String,
    pub global_speed_limit_kbps: u64,
    pub traffic_mode: TrafficMode,
    pub max_connections_per_server: u8,
    pub max_concurrent_downloads: u8,
    pub clipboard_watch: bool,
    pub bridge_enabled: bool,
    pub bridge_port: u16,
    pub auto_start_enabled: bool,
    pub auto_start_hour: u8,
    pub auto_start_minute: u8,
    pub auto_shutdown: bool,
    pub launch_at_startup: bool,
    pub portable: bool,
    pub close_to_tray: bool,
    pub start_hidden: bool,
    pub notifications: bool,
    pub torrent_encryption: EncryptionMode,
    pub torrent_dht: bool,
    pub torrent_lsd: bool,
    pub torrent_seed_ratio: f64,
    pub torrent_seed_minutes: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::System,
            default_folder: default_download_folder(),
            global_speed_limit_kbps: 0,
            clipboard_watch: false,
            bridge_enabled: true,
            bridge_port: crate::core::bridge::DEFAULT_PORT,
            auto_start_enabled: false,
            auto_start_hour: 0,
            auto_start_minute: 0,
            traffic_mode: TrafficMode::Custom,
            max_connections_per_server: 8,
            max_concurrent_downloads: 3,
            auto_shutdown: false,
            launch_at_startup: false,
            portable: false,
            close_to_tray: true,
            start_hidden: false,
            notifications: true,
            torrent_encryption: EncryptionMode::Disabled,
            torrent_dht: true,
            torrent_lsd: true,
            torrent_seed_ratio: 0.0,
            torrent_seed_minutes: 0,
        }
    }
}

impl Settings {
    pub fn torrent_policy(&self) -> Policy {
        Policy {
            encryption: self.torrent_encryption,
            dht: self.torrent_dht,
            lsd: self.torrent_lsd,
            seed_ratio: self.torrent_seed_ratio,
            seed_minutes: self.torrent_seed_minutes,
        }
    }
}

fn default_download_folder() -> String {
    dirs::download_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join("Downloads")))
        .map(|downloads| downloads.join("LotusDM"))
        .unwrap_or_else(|| PathBuf::from("Downloads").join("LotusDM"))
        .to_string_lossy()
        .into_owned()
}

pub fn settings_dir() -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if dir.join("portable").is_file() {
                return Some(dir.join("data"));
            }
        }
    }
    dirs::config_dir().map(|dir| dir.join("LotusDM"))
}

pub fn settings_path() -> Option<PathBuf> {
    settings_dir().map(|dir| dir.join("settings.json"))
}

pub fn load() -> Settings {
    let Some(path) = settings_path() else {
        return Settings::default();
    };
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

pub fn save(settings: &Settings) -> Result<(), String> {
    let dir = settings_dir().ok_or_else(|| "no config directory".to_string())?;
    fs::create_dir_all(&dir).map_err(|err| format!("create {}: {err}", dir.display()))?;
    let json = serde_json::to_string_pretty(settings).map_err(|err| err.to_string())?;
    fs::write(dir.join("settings.json"), json).map_err(|err| err.to_string())
}
