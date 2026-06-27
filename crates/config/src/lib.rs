//! `mdm-config` — layered configuration and observability setup shared by all binaries.
//!
//! Config is loaded (in increasing precedence) from built-in defaults, an optional
//! `config.toml`, and `MDM_`-prefixed environment variables. Secrets are wrapped in
//! [`Secret`] so they never leak through `Debug`/logs.

use std::net::SocketAddr;

use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use serde::{Deserialize, Serialize};

mod secret;
pub use secret::Secret;

pub mod tracing_init;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to load configuration: {0}")]
    Figment(#[from] figment::Error),
}

/// Output format for structured logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    Pretty,
    Json,
}

/// Application configuration. See `docs/PLAN.md` §7 (secrets) and `CLAUDE.md`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Connection string for the app runtime role (`md_app`, non-owner, `NOBYPASSRLS`).
    pub database_url: Secret,
    /// Connection string for the owner/migrator role (`md_owner`), used to run migrations.
    pub migration_database_url: Secret,
    /// Address the HTTP API binds to.
    pub api_addr: SocketAddr,
    /// Server-side pepper mixed into the HMAC of API keys and share tokens.
    pub api_key_pepper: Secret,
    /// One-time token (sent as `X-Bootstrap-Token`) gating the dev bootstrap endpoint.
    pub admin_bootstrap_token: Secret,
    /// Maximum allowed document body size in bytes (rejected above this).
    pub max_doc_bytes: i64,
    /// Window within which consecutive same-actor autosaves are coalesced into one version.
    pub autosave_debounce_secs: i64,
    /// Max DB pool connections.
    pub db_max_connections: u32,
    /// Structured-log output format.
    pub log_format: LogFormat,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database_url: Secret::new(
                "postgres://md_app:md_app_dev@localhost:5432/md_manager".into(),
            ),
            migration_database_url: Secret::new(
                "postgres://md_owner:md_owner_dev@localhost:5432/md_manager".into(),
            ),
            api_addr: "127.0.0.1:8080".parse().expect("valid default addr"),
            api_key_pepper: Secret::new("dev-insecure-pepper-change-me".into()),
            admin_bootstrap_token: Secret::new("dev-bootstrap-token".into()),
            max_doc_bytes: 1_000_000,
            autosave_debounce_secs: 30,
            db_max_connections: 10,
            log_format: LogFormat::Pretty,
        }
    }
}

impl Config {
    /// Load configuration from defaults + `config.toml` + `MDM_`-prefixed env vars.
    ///
    /// Example: `MDM_DATABASE_URL=...`, `MDM_API_ADDR=0.0.0.0:8080`, `MDM_LOG_FORMAT=json`.
    pub fn load() -> Result<Self, ConfigError> {
        Self::load_from("config.toml")
    }

    /// Like [`Config::load`] but with an explicit TOML path (used in tests).
    pub fn load_from(toml_path: &str) -> Result<Self, ConfigError> {
        let cfg = Figment::from(Serialized::defaults(Config::default()))
            .merge(Toml::file(toml_path))
            .merge(Env::prefixed("MDM_"))
            .extract()?;
        Ok(cfg)
    }
}
