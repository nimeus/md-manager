//! Shared application state and the authentication extractor.

use std::sync::Arc;

use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};
use mdm_core::AuthContext;
use mdm_db::Db;

use crate::error::ApiError;

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub bootstrap_token: Arc<String>,
}

/// Extractor that authenticates the `Authorization: Bearer mk_…` API key and yields the
/// resolved [`AuthContext`]. Reject with 401 if missing/invalid.
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
        let ctx = state.db.authenticate_api_key(token.trim()).await?;
        Ok(Auth(ctx))
    }
}
