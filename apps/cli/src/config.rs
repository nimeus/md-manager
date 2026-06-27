//! CLI profile config: a small JSON file storing the API URL + key.
//! Resolution precedence (highest first): command-line flag, env var, this file, default.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
pub struct StoredConfig {
    pub api_url: Option<String>,
    pub api_key: Option<String>,
}

pub fn config_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
            home.join(".config")
        });
    base.join("md-manager").join("config.json")
}

pub fn load() -> StoredConfig {
    std::fs::read_to_string(config_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(cfg: &StoredConfig) -> anyhow::Result<PathBuf> {
    let path = config_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(cfg)?)?;
    Ok(path)
}
