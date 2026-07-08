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

/// A resolved principal: a single-org request context, or an all-orgs connector (the user id;
/// the org is selected per call). [`authenticate`] is the single-org view used by REST.
#[derive(Debug, Clone)]
pub enum Principal {
    Org(AuthContext),
    AllOrgs(Uuid),
}

/// Resolve a bearer token to a [`Principal`]:
/// - `mk_…` ⇒ API key (single org) · `mss_…` ⇒ web session (`X-Org-Id` picks the org)
/// - `mo_…` ⇒ built-in OAuth connector: a single-org token (org intrinsic, `X-Org-Id` ignored)
///   or an **all-orgs** token (`AllOrgs(user_id)` — the agent picks the org per call)
/// - otherwise ⇒ external OAuth JWT (Logto), if configured.
pub async fn authenticate_principal(
    state: &AppState,
    token: &str,
    org_override: Option<Uuid>,
) -> Result<Principal, mdm_core::Error> {
    let token = token.trim();
    let principal = if token.starts_with("mk_") {
        Principal::Org(state.db.authenticate_api_key(token).await?)
    } else if token.starts_with(session::SESSION_PREFIX) {
        let user_id = session::verify(&state.session_secret, token).map_err(|err| {
            tracing::debug!(%err, "session token validation failed");
            mdm_core::Error::Unauthorized
        })?;
        Principal::Org(state.db.authenticate_session(user_id, org_override).await?)
    } else if token.starts_with("mo_") {
        match state
            .db
            .authenticate_oauth_access_token(token, &state.mcp_resource)
            .await?
        {
            mdm_db::OAuthAccess::Org(ctx) => Principal::Org(ctx),
            mdm_db::OAuthAccess::AllOrgs { user_id } => Principal::AllOrgs(user_id),
        }
    } else if let Some(oauth) = &state.oauth {
        let claims = oauth.validate(token).await.map_err(|err| {
            tracing::debug!(?err, "OAuth token validation failed");
            mdm_core::Error::Unauthorized
        })?;
        let org_id = uuid::Uuid::parse_str(&claims.org)
            .map_err(|_| mdm_core::Error::invalid("org claim is not a valid org id"))?;
        Principal::Org(state.db.authenticate_oauth(&claims.sub, org_id).await?)
    } else {
        return Err(mdm_core::Error::Unauthorized);
    };

    // Per-user rate limit (applies to both REST and the MCP endpoint).
    let user_id = match &principal {
        Principal::Org(ctx) => ctx.user_id,
        Principal::AllOrgs(uid) => *uid,
    };
    if state.rate_limiter.check_key(&user_id).is_err() {
        return Err(mdm_core::Error::TooManyRequests(
            "rate limit exceeded".into(),
        ));
    }
    Ok(principal)
}

/// Resolve a bearer token to a single-org [`AuthContext`] (REST). An all-orgs connector token
/// collapses to one org via `org_override` (the `X-Org-Id` header), else the user's first org.
pub async fn authenticate(
    state: &AppState,
    token: &str,
    org_override: Option<Uuid>,
) -> Result<AuthContext, mdm_core::Error> {
    match authenticate_principal(state, token, org_override).await? {
        Principal::Org(ctx) => Ok(ctx),
        Principal::AllOrgs(user_id) => state.db.authenticate_session(user_id, org_override).await,
    }
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
