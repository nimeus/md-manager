//! Built-in OAuth 2.1 Authorization Server endpoints (enabled when `MDM_OAUTH_MODE=builtin`).
//!
//! - Discovery: `GET /.well-known/oauth-authorization-server` (RFC 8414). The protected-resource
//!   metadata (RFC 9728) lives in [`crate::mcp`].
//! - `POST /oauth/register` — Dynamic Client Registration (RFC 7591), anonymous + IP-rate-limited.
//! - `GET  /oauth/authorize` — validates the request and 302s the browser to the web consent page.
//! - `POST /oauth/token` — `authorization_code` (PKCE) + `refresh_token` grants (RFC 6749 errors).
//! - `POST /oauth/revoke` — RFC 7009.
//! - BFF (session-authed, called by the Next.js consent page): get request, approve, deny.
//!
//! The API is the authorization server *and* the resource server; the web app only renders the
//! consent UI. The consenting user's identity comes from their forwarded `mss_` session (the
//! `Auth` extractor) — never from the request body.

use axum::{
    Json,
    body::Bytes,
    extract::{Form, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
};
use mdm_core::oauth::{OAuthErrorCode, PKCE_METHOD_S256, validate_redirect_uri};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::error::ApiError;
use crate::state::{AppState, Auth};

type ApiResult<T> = Result<T, ApiError>;

const NO_STORE: (header::HeaderName, &str) = (header::CACHE_CONTROL, "no-store");
const CACHE_1H: (header::HeaderName, &str) = (header::CACHE_CONTROL, "public, max-age=3600");
const CORS_ANY: (header::HeaderName, &str) = (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*");

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// An RFC 6749 error body (`{error, error_description}`) at the given status.
fn oauth_error(status: StatusCode, code: OAuthErrorCode, desc: &str) -> Response {
    (
        status,
        [NO_STORE],
        Json(json!({ "error": code.as_str(), "error_description": desc })),
    )
        .into_response()
}

// ── Discovery (RFC 8414) ─────────────────────────────────────────────────────────────────

pub async fn authorization_server_metadata(State(s): State<AppState>) -> Response {
    // Only the built-in AS publishes this; an external issuer publishes its own.
    let (Some(issuer), Some(_)) = (&s.issuer, &s.builtin_oauth) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let issuer = issuer.as_str();
    let body = json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{issuer}/oauth/authorize"),
        "token_endpoint": format!("{issuer}/oauth/token"),
        "registration_endpoint": format!("{issuer}/oauth/register"),
        "revocation_endpoint": format!("{issuer}/oauth/revoke"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["none", "client_secret_post"],
        "scopes_supported": ["mcp"],
    });
    (StatusCode::OK, [CACHE_1H, CORS_ANY], Json(body)).into_response()
}

// ── Dynamic Client Registration (RFC 7591) ───────────────────────────────────────────────

#[derive(Deserialize)]
struct DcrReq {
    #[serde(default)]
    redirect_uris: Vec<String>,
    client_name: Option<String>,
    token_endpoint_auth_method: Option<String>,
}

pub async fn register(State(s): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    if s.builtin_oauth.is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }
    // Anonymous endpoint → per-IP rate limit is the only spam defense.
    let ip = client_ip(&headers);
    if s.dcr_limiter.check_key(&ip).is_err() {
        return oauth_error(
            StatusCode::TOO_MANY_REQUESTS,
            OAuthErrorCode::InvalidRequest,
            "registration rate limit exceeded",
        );
    }

    let req: DcrReq = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => {
            return oauth_error(
                StatusCode::BAD_REQUEST,
                OAuthErrorCode::InvalidRequest,
                "request body must be valid JSON client metadata",
            );
        }
    };
    if req.redirect_uris.is_empty() || req.redirect_uris.len() > 10 {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            OAuthErrorCode::InvalidRequest,
            "redirect_uris is required (1–10 https/loopback URIs)",
        );
    }
    for uri in &req.redirect_uris {
        if let Err(e) = validate_redirect_uri(uri) {
            return oauth_error(StatusCode::BAD_REQUEST, OAuthErrorCode::InvalidRequest, &e);
        }
    }

    // Claude/ChatGPT register as public clients (token_endpoint_auth_method = "none").
    let public = !matches!(
        req.token_endpoint_auth_method.as_deref(),
        Some("client_secret_basic") | Some("client_secret_post")
    );
    let name: String = req
        .client_name
        .unwrap_or_else(|| "MCP client".to_string())
        .chars()
        .take(200)
        .collect();

    match s.db.register_oauth_client(&name, &req.redirect_uris, public).await {
        Ok(reg) => {
            // RFC 7591 response. Omit absent fields entirely (no null/empty — Claude is strict).
            let mut out = json!({
                "client_id": reg.client_id,
                "client_id_issued_at": unix_now(),
                "redirect_uris": req.redirect_uris,
                "grant_types": ["authorization_code", "refresh_token"],
                "response_types": ["code"],
                "token_endpoint_auth_method": if public { "none" } else { "client_secret_post" },
                "client_name": name,
                "scope": "mcp",
            });
            if let Some(secret) = reg.client_secret {
                out["client_secret"] = json!(secret);
                out["client_secret_expires_at"] = json!(0); // never expires
            }
            (StatusCode::CREATED, [NO_STORE], Json(out)).into_response()
        }
        Err(_) => oauth_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            OAuthErrorCode::ServerError,
            "registration failed",
        ),
    }
}

// ── Authorize → consent ──────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub(crate) struct AuthorizeQuery {
    response_type: Option<String>,
    client_id: Option<String>,
    redirect_uri: Option<String>,
    code_challenge: Option<String>,
    code_challenge_method: Option<String>,
    state: Option<String>,
    scope: Option<String>,
    resource: Option<String>,
}

pub async fn authorize(State(s): State<AppState>, Query(q): Query<AuthorizeQuery>) -> Response {
    let Some(settings) = &s.builtin_oauth else {
        return StatusCode::NOT_FOUND.into_response();
    };

    // Validate client + redirect_uri BEFORE trusting the redirect target (open-redirect guard).
    let Some(client_id) = q.client_id.as_deref() else {
        return error_page("missing client_id");
    };
    let client = match s.db.find_oauth_client(client_id).await {
        Ok(c) => c,
        Err(_) => return error_page("unknown or revoked client_id"),
    };
    let Some(redirect_uri) = q.redirect_uri.as_deref() else {
        return error_page("missing redirect_uri");
    };
    if !client.redirect_uris.iter().any(|u| u == redirect_uri)
        || validate_redirect_uri(redirect_uri).is_err()
    {
        return error_page("redirect_uri is not registered for this client");
    }

    // redirect_uri is now trusted — protocol errors go back to the client via redirect.
    let st = q.state.as_deref();
    if q.response_type.as_deref() != Some("code") {
        return redirect_error(redirect_uri, OAuthErrorCode::InvalidRequest, "response_type must be code", st);
    }
    let Some(challenge) = q.code_challenge.as_deref() else {
        return redirect_error(redirect_uri, OAuthErrorCode::InvalidRequest, "code_challenge is required (PKCE)", st);
    };
    if q.code_challenge_method.as_deref().unwrap_or("") != PKCE_METHOD_S256 {
        return redirect_error(redirect_uri, OAuthErrorCode::InvalidRequest, "code_challenge_method must be S256", st);
    }
    let resource = s.mcp_resource.as_str();
    if let Some(r) = q.resource.as_deref()
        && r != resource
    {
        return redirect_error(redirect_uri, OAuthErrorCode::InvalidTarget, "resource does not match this server", st);
    }
    let scope = q.scope.as_deref().unwrap_or("mcp");

    match s
        .db
        .create_authorization_request(
            client.db_id,
            redirect_uri,
            challenge,
            PKCE_METHOD_S256,
            resource,
            scope,
            st,
            settings.request_ttl_secs,
        )
        .await
    {
        Ok(req_id) => {
            let consent = format!("{}/oauth/consent?request_id={}", settings.web_url, req_id);
            Redirect::to(&consent).into_response()
        }
        Err(_) => redirect_error(redirect_uri, OAuthErrorCode::ServerError, "could not start authorization", st),
    }
}

// ── Token ────────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub(crate) struct TokenForm {
    grant_type: Option<String>,
    code: Option<String>,
    redirect_uri: Option<String>,
    code_verifier: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
    refresh_token: Option<String>,
    resource: Option<String>,
}

pub async fn token(State(s): State<AppState>, Form(f): Form<TokenForm>) -> Response {
    let Some(settings) = &s.builtin_oauth else {
        return StatusCode::NOT_FOUND.into_response();
    };

    // Client auth from the body. Public clients (Claude/ChatGPT) send only client_id; PKCE is
    // their proof. Confidential clients send client_secret (client_secret_post).
    let Some(client_id) = f.client_id.as_deref() else {
        return oauth_error(StatusCode::UNAUTHORIZED, OAuthErrorCode::InvalidClient, "missing client_id");
    };
    let client = match s.db.authenticate_oauth_client(client_id, f.client_secret.as_deref()).await {
        Ok(c) => c,
        Err(_) => {
            return oauth_error(StatusCode::UNAUTHORIZED, OAuthErrorCode::InvalidClient, "client authentication failed");
        }
    };

    match f.grant_type.as_deref() {
        Some("authorization_code") => {
            let (Some(code), Some(redirect_uri), Some(verifier)) =
                (f.code.as_deref(), f.redirect_uri.as_deref(), f.code_verifier.as_deref())
            else {
                return oauth_error(
                    StatusCode::BAD_REQUEST,
                    OAuthErrorCode::InvalidRequest,
                    "code, redirect_uri and code_verifier are required",
                );
            };
            match s
                .db
                .exchange_auth_code(
                    client.db_id,
                    code,
                    redirect_uri,
                    verifier,
                    f.resource.as_deref(),
                    settings.access_ttl_secs,
                    settings.refresh_ttl_secs,
                )
                .await
            {
                Ok(t) => token_response(t),
                Err(_) => oauth_error(StatusCode::BAD_REQUEST, OAuthErrorCode::InvalidGrant, "invalid authorization code"),
            }
        }
        Some("refresh_token") => {
            let Some(rt) = f.refresh_token.as_deref() else {
                return oauth_error(StatusCode::BAD_REQUEST, OAuthErrorCode::InvalidRequest, "refresh_token is required");
            };
            match s
                .db
                .refresh_oauth_token(client.db_id, rt, settings.access_ttl_secs, settings.refresh_ttl_secs)
                .await
            {
                Ok(t) => token_response(t),
                Err(_) => oauth_error(StatusCode::BAD_REQUEST, OAuthErrorCode::InvalidGrant, "invalid or reused refresh token"),
            }
        }
        other => oauth_error(
            StatusCode::BAD_REQUEST,
            OAuthErrorCode::UnsupportedGrantType,
            &format!("unsupported grant_type: {}", other.unwrap_or("(none)")),
        ),
    }
}

fn token_response(t: mdm_db::IssuedTokens) -> Response {
    (
        StatusCode::OK,
        [NO_STORE],
        Json(json!({
            "access_token": t.access_token,
            "token_type": "Bearer",
            "expires_in": t.access_expires_in,
            "refresh_token": t.refresh_token,
            "scope": t.scope,
        })),
    )
        .into_response()
}

// ── Revoke (RFC 7009) ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub(crate) struct RevokeForm {
    token: Option<String>,
}

pub async fn revoke(State(s): State<AppState>, Form(f): Form<RevokeForm>) -> Response {
    if s.builtin_oauth.is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }
    if let Some(t) = f.token.as_deref() {
        let _ = s.db.revoke_oauth_token(t).await;
    }
    StatusCode::OK.into_response() // RFC 7009: always 200 (don't reveal token existence)
}

// ── BFF: consent display + approve/deny (session-authed via the Auth extractor) ───────────

pub async fn get_authorization_request(
    State(s): State<AppState>,
    _auth: Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let d = s.db.get_authorization_request_display(id).await?;
    Ok(Json(json!({
        "client_name": d.client_name,
        "scope": d.scope,
        "redirect_uri": d.redirect_uri,
    })))
}

#[derive(Deserialize)]
pub(crate) struct ApproveReq {
    org_id: Uuid,
}

pub async fn approve(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
    Json(req): Json<ApproveReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let settings = s
        .builtin_oauth
        .as_ref()
        .ok_or(ApiError(mdm_core::Error::NotFound))?;
    // The consenting user is the verified session user (ctx.user_id) — NOT the request body.
    let minted = s
        .db
        .approve_authorization_request(id, ctx.user_id, req.org_id, settings.code_ttl_secs)
        .await?;
    let mut params: Vec<(&str, &str)> = vec![("code", &minted.code)];
    if let Some(st) = &minted.state {
        params.push(("state", st));
    }
    Ok(Json(json!({ "redirect_to": append_query(&minted.redirect_uri, &params) })))
}

pub async fn deny(
    State(s): State<AppState>,
    _auth: Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let out = s.db.deny_authorization_request(id).await?;
    let mut params: Vec<(&str, &str)> = vec![("error", "access_denied")];
    if let Some(st) = &out.state {
        params.push(("state", st));
    }
    Ok(Json(json!({ "redirect_to": append_query(&out.redirect_uri, &params) })))
}

// ── helpers ──────────────────────────────────────────────────────────────────────────────

/// A plain-text 400 for authorize errors we can't safely redirect (bad client / redirect_uri).
fn error_page(msg: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        format!("Authorization error: {msg}\n"),
    )
        .into_response()
}

/// 303 back to the client's (already-validated) redirect_uri carrying an OAuth error + state.
fn redirect_error(
    redirect_uri: &str,
    code: OAuthErrorCode,
    desc: &str,
    state: Option<&str>,
) -> Response {
    let mut params: Vec<(&str, &str)> = vec![("error", code.as_str()), ("error_description", desc)];
    if let Some(st) = state {
        params.push(("state", st));
    }
    Redirect::to(&append_query(redirect_uri, &params)).into_response()
}

/// Client IP for rate-limiting: first hop of `X-Forwarded-For` (set by the proxy), else loopback.
fn client_ip(headers: &HeaderMap) -> std::net::IpAddr {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(str::trim)
        .and_then(|s| s.parse().ok())
        .unwrap_or(std::net::IpAddr::from([127, 0, 0, 1]))
}

/// Append `params` (+ url-encoded values) to a URL, choosing `?`/`&` correctly.
fn append_query(base: &str, params: &[(&str, &str)]) -> String {
    let sep = if base.contains('?') { '&' } else { '?' };
    let qs = params
        .iter()
        .map(|(k, v)| format!("{k}={}", pct(v)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{base}{sep}{qs}")
}

/// Percent-encode a query value (RFC 3986 unreserved stay; everything else is %XX).
fn pct(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
