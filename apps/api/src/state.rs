//! Shared application state and authentication (API key OR OAuth JWT).

use std::sync::Arc;

use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};
use mdm_core::AuthContext;
use mdm_db::Db;

use crate::error::ApiError;
use crate::oauth::OAuthValidator;

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
}

/// Resolve a bearer token to an [`AuthContext`]:
/// - `mk_…` ⇒ API key (terminal agents / CLI)
/// - otherwise ⇒ OAuth JWT (web connectors), if OAuth is configured.
pub async fn authenticate(state: &AppState, token: &str) -> Result<AuthContext, mdm_core::Error> {
    let token = token.trim();
    if token.starts_with("mk_") {
        return state.db.authenticate_api_key(token).await;
    }
    if let Some(oauth) = &state.oauth {
        let claims = oauth.validate(token).await.map_err(|err| {
            tracing::debug!(?err, "OAuth token validation failed");
            mdm_core::Error::Unauthorized
        })?;
        let org_id = uuid::Uuid::parse_str(&claims.org)
            .map_err(|_| mdm_core::Error::invalid("org claim is not a valid org id"))?;
        return state.db.authenticate_oauth(&claims.sub, org_id).await;
    }
    Err(mdm_core::Error::Unauthorized)
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
        let ctx = authenticate(state, token).await?;
        Ok(Auth(ctx))
    }
}
