//! Shared application state and authentication (API key OR OAuth JWT).

use std::sync::Arc;

use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};
use governor::DefaultKeyedRateLimiter;
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

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub bootstrap_token: Arc<String>,
    /// OAuth resource-server validator (Some when OAuth is configured).
    pub oauth: Option<Arc<OAuthValidator>>,
    /// Public base URL of this resource server (for discovery metadata + challenges).
    pub resource_url: Arc<String>,
    /// Authorization-server issuer URL (Some when OAuth is configured).
    pub issuer: Option<Arc<String>>,
    /// Per-user request rate limiter.
    pub rate_limiter: Arc<RateLimiter>,
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
/// - otherwise ⇒ OAuth JWT (web connectors), if OAuth is configured.
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
