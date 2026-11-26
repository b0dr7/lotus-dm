use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::ui::theme::ThemeMode;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme_mode: ThemeMode,
    pub default_folder: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::Dark,
            default_folder: default_download_folder(),
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

/// %APPDATA%/LotusDM on Windows, ~/.config/LotusDM elsewhere.
pub fn settings_dir() -> Option<PathBuf> {
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
