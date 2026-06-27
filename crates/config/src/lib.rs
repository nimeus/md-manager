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
    /// OAuth 2.1 resource-server settings for the remote MCP endpoint. When all of
    /// `oauth_issuer` / `oauth_jwks_url` / `oauth_audience` are set, JWT auth is enabled
    /// (in addition to API keys) and the discovery endpoint advertises the issuer.
    pub oauth_issuer: Option<String>,
    pub oauth_jwks_url: Option<String>,
    /// The canonical resource URI; an access token's `aud` must equal this exactly.
    pub oauth_audience: Option<String>,
    /// JWT claim carrying the organization id (default `org`).
    pub oauth_org_claim: String,
    /// Public base URL of this MCP resource server (used in discovery metadata).
    /// Defaults to `http://<api_addr>`.
    pub public_url: Option<String>,
}

/// Resolved OAuth resource-server settings.
#[derive(Debug, Clone)]
pub struct OAuthSettings {
    pub issuer: String,
    pub jwks_url: String,
    pub audience: String,
    pub org_claim: String,
}

impl Config {
    /// OAuth settings if fully configured.
    pub fn oauth(&self) -> Option<OAuthSettings> {
        match (&self.oauth_issuer, &self.oauth_jwks_url, &self.oauth_audience) {
            (Some(issuer), Some(jwks_url), Some(audience)) => Some(OAuthSettings {
                issuer: issuer.clone(),
                jwks_url: jwks_url.clone(),
                audience: audience.clone(),
                org_claim: self.oauth_org_claim.clone(),
            }),
            _ => None,
        }
    }

    /// Public base URL for discovery metadata.
    pub fn public_base_url(&self) -> String {
        self.public_url
            .clone()
            .unwrap_or_else(|| format!("http://{}", self.api_addr))
    }
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
            oauth_issuer: None,
            oauth_jwks_url: None,
            oauth_audience: None,
            oauth_org_claim: "org".to_string(),
            public_url: None,
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
