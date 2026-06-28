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
    /// Optional superuser connection used ONCE at startup to auto-provision the `md_owner` /
    /// `md_app` roles (taken from the two URLs above), so a managed Postgres needs no manual
    /// SQL. Point it at the app database. Omit it if you create the roles yourself.
    pub setup_database_url: Option<Secret>,
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
    /// Max documents allowed per project (abuse guard against agent create-loops).
    pub max_docs_per_project: i64,
    /// Per-user API request rate limit (requests per minute).
    pub rate_limit_per_minute: u32,
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

    // --- Web sign-in (Google + web sessions) ---
    /// Google OAuth client id. When set, `POST /v1/auth/google` verifies Google ID tokens
    /// (aud == this) and issues web session tokens. Required for the web app's Google login.
    pub google_client_id: Option<String>,
    /// HS256 secret the API signs web session tokens (`mss_…`) with. Change in production.
    pub session_secret: Secret,
    /// Web session lifetime in seconds (default 30 days).
    pub session_ttl_secs: i64,

    // --- Embeddings (semantic search) — all env-driven, OpenAI-compatible API shape. ---
    /// Enable embedding indexing + semantic/hybrid search.
    pub embedding_enabled: bool,
    /// Embeddings API base URL (OpenAI-compatible). Defaults to OpenRouter; override for any provider.
    pub embedding_base_url: String,
    /// Bearer API key for the embeddings provider.
    pub embedding_api_key: Secret,
    /// Embedding model id (e.g. an OpenRouter / OpenAI embedding model).
    pub embedding_model: String,
    /// Embedding vector dimensions (must match the model). Sets the pgvector column width.
    pub embedding_dimensions: i32,
    /// Max chunks per embedding API request.
    pub embedding_batch_size: i64,
    /// Embedding HTTP request timeout (seconds).
    pub embedding_timeout_secs: u64,
    /// How often the background embedding worker polls for unembedded chunks (seconds).
    pub embedding_worker_interval_secs: u64,
    /// Base seconds for exponential backoff between retries of a failing chunk
    /// (delay = base · 2^attempts, capped). Keeps one poison chunk from looping.
    pub embedding_backoff_base_secs: i64,
    /// After this many consecutive failures a chunk is dead-lettered (skipped, surfaced
    /// to ops) so it can never starve the queue. 0 disables dead-lettering (retry forever).
    pub embedding_max_attempts: i32,
    /// Optional OpenRouter `HTTP-Referer` header (app attribution).
    pub embedding_referer: Option<String>,
    /// Optional OpenRouter `X-Title` header (app attribution).
    pub embedding_title: Option<String>,
}

/// Resolved embedding settings (present only when enabled + fully configured).
#[derive(Debug, Clone)]
pub struct EmbeddingSettings {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub dimensions: i32,
    pub batch_size: i64,
    pub timeout_secs: u64,
    pub worker_interval_secs: u64,
    pub backoff_base_secs: i64,
    pub max_attempts: i32,
    pub referer: Option<String>,
    pub title: Option<String>,
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
        match (
            &self.oauth_issuer,
            &self.oauth_jwks_url,
            &self.oauth_audience,
        ) {
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

    /// Embedding settings, if enabled and fully configured (model + api key + dimensions).
    pub fn embedding(&self) -> Option<EmbeddingSettings> {
        if !self.embedding_enabled
            || self.embedding_model.trim().is_empty()
            || self.embedding_api_key.expose().trim().is_empty()
            || self.embedding_dimensions <= 0
        {
            return None;
        }
        Some(EmbeddingSettings {
            base_url: self.embedding_base_url.trim_end_matches('/').to_string(),
            api_key: self.embedding_api_key.expose().to_string(),
            model: self.embedding_model.clone(),
            dimensions: self.embedding_dimensions,
            batch_size: self.embedding_batch_size.max(1),
            timeout_secs: self.embedding_timeout_secs,
            worker_interval_secs: self.embedding_worker_interval_secs.max(1),
            backoff_base_secs: self.embedding_backoff_base_secs.max(1),
            max_attempts: self.embedding_max_attempts.max(0),
            referer: self.embedding_referer.clone(),
            title: self.embedding_title.clone(),
        })
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
            setup_database_url: None,
            api_addr: "127.0.0.1:8080".parse().expect("valid default addr"),
            api_key_pepper: Secret::new("dev-insecure-pepper-change-me".into()),
            admin_bootstrap_token: Secret::new("dev-bootstrap-token".into()),
            max_doc_bytes: 1_000_000,
            autosave_debounce_secs: 30,
            max_docs_per_project: 5_000,
            rate_limit_per_minute: 120,
            db_max_connections: 10,
            log_format: LogFormat::Pretty,
            oauth_issuer: None,
            oauth_jwks_url: None,
            oauth_audience: None,
            oauth_org_claim: "org".to_string(),
            public_url: None,
            google_client_id: None,
            session_secret: Secret::new("dev-insecure-session-secret-change-me".into()),
            session_ttl_secs: 60 * 60 * 24 * 30,
            embedding_enabled: false,
            embedding_base_url: "https://openrouter.ai/api/v1".to_string(),
            embedding_api_key: Secret::new(String::new()),
            embedding_model: String::new(),
            embedding_dimensions: 1536,
            embedding_batch_size: 32,
            embedding_timeout_secs: 30,
            embedding_worker_interval_secs: 10,
            embedding_backoff_base_secs: 30,
            embedding_max_attempts: 8,
            embedding_referer: None,
            embedding_title: None,
        }
    }
}

impl Config {
    /// Load configuration from defaults + `config.toml` + `MDM_`-prefixed env vars.
    ///
    /// Example: `MDM_DATABASE_URL=...`, `MDM_API_ADDR=0.0.0.0:8080`, `MDM_LOG_FORMAT=json`.
    // figment::Error is large, but this is a one-shot startup path.
    #[allow(clippy::result_large_err)]
    pub fn load() -> Result<Self, ConfigError> {
        Self::load_from("config.toml")
    }

    /// Like [`Config::load`] but with an explicit TOML path (used in tests).
    #[allow(clippy::result_large_err)]
    pub fn load_from(toml_path: &str) -> Result<Self, ConfigError> {
        let cfg = Figment::from(Serialized::defaults(Config::default()))
            .merge(Toml::file(toml_path))
            .merge(Env::prefixed("MDM_"))
            .extract()?;
        Ok(cfg)
    }
}
