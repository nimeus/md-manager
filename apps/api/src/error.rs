//! Mapping domain errors onto HTTP responses.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

/// Wraps a [`mdm_core::Error`] so it can be returned from handlers via `?`.
pub struct ApiError(pub mdm_core::Error);

impl From<mdm_core::Error> for ApiError {
    fn from(e: mdm_core::Error) -> Self {
        ApiError(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        use mdm_core::Error::*;
        let (status, code) = match &self.0 {
            NotFound => (StatusCode::NOT_FOUND, "not_found"),
            Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            Conflict { .. } => (StatusCode::CONFLICT, "conflict"),
            AlreadyExists(_) => (StatusCode::CONFLICT, "already_exists"),
            Invalid(_) => (StatusCode::BAD_REQUEST, "invalid"),
            TooManyRequests(_) => (StatusCode::TOO_MANY_REQUESTS, "too_many_requests"),
            Internal(detail) => {
                tracing::error!(detail, "internal error");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal")
            }
        };
        // `Internal`'s Display is generic ("internal error"), so detail never leaks.
        let body = json!({ "error": code, "message": self.0.to_string() });
        (status, Json(body)).into_response()
    }
}
