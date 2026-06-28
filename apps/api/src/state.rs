//! Shared application state and authentication (API key, web session, built-in OAuth connector
//! token, or external OAuth JWT).

use std::net::IpAddr;
use std::sync::Arc;

use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};
use governor::DefaultKeyedRateLimiter;
use mdm_config::BuiltinOAuthSettings;
use mdm_core::AuthContext;
use mdm_db::Db;
use mdm_embed::Embedder;
use uuid::Uuid;

use crate::error::ApiError;
use crate::google::GoogleValidator;
use crate::oauth::OAuthValidator;
use crate::session;

/// Per-user request rate limiter (keyed by the authenticated user id).
pub type RateLimiter = DefaultKeyedRateLimiter<Uuid>;
/// Per-IP rate limiter (keyed by client IP) for anonymous Dynamic Client Registration.
pub type IpRateLimiter = DefaultKeyedRateLimiter<IpAddr>;

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub bootstrap_token: Arc<String>,
    /// External OAuth resource-server validator (Some only in `logto` mode).
    pub oauth: Option<Arc<OAuthValidator>>,
    /// Public base URL of this resource server (for discovery metadata + challenges).
    pub resource_url: Arc<String>,
    /// Canonical MCP resource URI (`<resource_url>/mcp`) — bound into connector tokens and
    /// checked on every `/mcp` request (RFC 8707 audience).
    pub mcp_resource: Arc<String>,
    /// Authorization-server issuer URL (Some when OAuth is configured; self in builtin mode).
    pub issuer: Option<Arc<String>>,
    /// Built-in OAuth authorization-server settings (Some when `MDM_OAUTH_MODE=builtin`).
    pub builtin_oauth: Option<Arc<BuiltinOAuthSettings>>,
    /// Per-user request rate limiter.
    pub rate_limiter: Arc<RateLimiter>,
    /// Per-IP limiter for anonymous DCR registrations.
    pub dcr_limiter: Arc<IpRateLimiter>,
    /// Embeddings client (Some when semantic search is configured/enabled).
    pub embedder: Option<Arc<Embedder>>,
    /// Google ID-token validator for web sign-in (Some when `MDM_GOOGLE_CLIENT_ID` is set).
    pub google: Option<Arc<GoogleValidator>>,
    /// HS256 secret for signing/verifying web session tokens (`mss_…`).
    pub session_secret: Arc<String>,
    /// Web session lifetime (seconds).
    pub session_ttl_secs: i64,
}

/// Resolve a bearer token to an [`AuthContext`]:
/// - `mk_…` ⇒ API key (terminal agents / CLI)
/// - `mss_…` ⇒ web session token (browser via the Next.js BFF); `org_override` (the
///   `X-Org-Id` header) selects which of the user's orgs to act in (the org switcher)
/// - `mo_…` ⇒ built-in OAuth connector token (Claude.ai / ChatGPT). The org is intrinsic to the
///   token, so `org_override` is **ignored** — a connector must not switch orgs via a header.
/// - otherwise ⇒ external OAuth JWT (Logto), if configured.
pub async fn authenticate(
    state: &AppState,
    token: &str,
    org_override: Option<Uuid>,
) -> Result<AuthContext, mdm_core::Error> {
    let token = token.trim();
    let ctx = if token.starts_with("mk_") {
        state.db.authenticate_api_key(token).await?
    } else if token.starts_with(session::SESSION_PREFIX) {
        let user_id = session::verify(&state.session_secret, token).map_err(|err| {
            tracing::debug!(%err, "session token validation failed");
            mdm_core::Error::Unauthorized
        })?;
        state.db.authenticate_session(user_id, org_override).await?
    } else if token.starts_with("mo_") {
        // Built-in OAuth connector access token; the org is bound into the token itself.
        state
            .db
            .authenticate_oauth_access_token(token, &state.mcp_resource)
            .await?
    } else if let Some(oauth) = &state.oauth {
        let claims = oauth.validate(token).await.map_err(|err| {
            tracing::debug!(?err, "OAuth token validation failed");
            mdm_core::Error::Unauthorized
        })?;
        let org_id = uuid::Uuid::parse_str(&claims.org)
            .map_err(|_| mdm_core::Error::invalid("org claim is not a valid org id"))?;
        state.db.authenticate_oauth(&claims.sub, org_id).await?
    } else {
        return Err(mdm_core::Error::Unauthorized);
    };

    // Per-user rate limit (applies to both REST and the MCP endpoint).
    if state.rate_limiter.check_key(&ctx.user_id).is_err() {
        return Err(mdm_core::Error::TooManyRequests(
            "rate limit exceeded".into(),
        ));
    }
    Ok(ctx)
}

/// Extractor yielding the resolved [`AuthContext`]; 401 if missing/invalid.
pub struct Auth(pub AuthContext);

impl FromRequestParts<AppState> for Auth {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .ok_or(ApiError(mdm_core::Error::Unauthorized))?;
        // The org switcher: web sessions pick which org to act in via this header.
        let org_override = parts
            .headers
            .get("x-org-id")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| Uuid::parse_str(s).ok());
        let ctx = authenticate(state, token, org_override).await?;
        Ok(Auth(ctx))
    }
}
