//! Pure OAuth 2.1 authorization-server helpers: PKCE S256 verification, redirect-URI
//! validation, token generation, and the RFC 6749 error vocabulary.
//!
//! The `db` crate persists/validates clients, codes and tokens; the `api` crate handles the
//! HTTP endpoints. Tokens reuse the API-key HMAC scheme in [`crate::crypto`], so they're
//! stored as HMAC-SHA256(pepper, secret) + a lookup prefix and verified in constant time.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::crypto::{self, GeneratedKey};

/// Token schemes (prefixes) for the built-in AS. Distinct from `mk_` (API keys) and `mss_`
/// (web sessions) so the resource server can branch on the prefix.
pub const ACCESS_TOKEN_SCHEME: &str = "mo"; //   mo_…   access token  (sent to /mcp)
pub const REFRESH_TOKEN_SCHEME: &str = "mor"; // mor_…  refresh token (sent to /oauth/token)
pub const AUTH_CODE_SCHEME: &str = "moc"; //     moc_…  authorization code
pub const CLIENT_SECRET_SCHEME: &str = "mocs"; // mocs_… confidential client secret
pub const CLIENT_ID_SCHEME: &str = "cid"; //     cid_…  public client id

/// The only PKCE method we accept (OAuth 2.1 forbids `plain`).
pub const PKCE_METHOD_S256: &str = "S256";

pub fn generate_access_token() -> GeneratedKey {
    crypto::generate_token(ACCESS_TOKEN_SCHEME)
}
pub fn generate_refresh_token() -> GeneratedKey {
    crypto::generate_token(REFRESH_TOKEN_SCHEME)
}
pub fn generate_auth_code() -> GeneratedKey {
    crypto::generate_token(AUTH_CODE_SCHEME)
}
pub fn generate_client_secret() -> GeneratedKey {
    crypto::generate_token(CLIENT_SECRET_SCHEME)
}
/// A DCR-issued client id (`cid_…`) — an opaque, unguessable label (not a secret).
pub fn generate_client_id() -> String {
    crypto::generate_token(CLIENT_ID_SCHEME).secret
}

/// Verify a PKCE `code_verifier` against a stored S256 `code_challenge`.
///
/// `challenge == BASE64URL-NO-PAD( SHA256( ASCII(verifier) ) )` (RFC 7636 §4.6), compared in
/// constant time. **Must** use base64url-without-padding — a hex comparison silently rejects
/// every real client.
pub fn verify_pkce_s256(verifier: &str, challenge: &str) -> bool {
    // RFC 7636 §4.1: the verifier is 43–128 chars of the unreserved set.
    if verifier.len() < 43 || verifier.len() > 128 {
        return false;
    }
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let computed = URL_SAFE_NO_PAD.encode(hasher.finalize());
    computed.as_bytes().ct_eq(challenge.as_bytes()).into()
}

/// Validate a redirect URI for registration / authorize.
///
/// Allows `https://` on any host, or `http://` **only** for loopback (localhost / 127.0.0.1 /
/// ::1) for local dev. Rejects fragments and any other scheme. This is the open-redirect
/// defense: the AS must never 302 to a URI that fails this *and* isn't byte-equal to one the
/// client registered.
pub fn validate_redirect_uri(uri: &str) -> Result<(), String> {
    if uri.contains('#') {
        return Err("redirect_uri must not contain a fragment".into());
    }
    let (scheme, rest) = uri
        .split_once("://")
        .ok_or("redirect_uri must be an absolute URL")?;
    let host = host_of(rest);
    match scheme {
        "https" => {
            if host.is_empty() {
                Err("redirect_uri host is empty".into())
            } else {
                Ok(())
            }
        }
        "http" => {
            if is_loopback(host) {
                Ok(())
            } else {
                Err("http redirect_uri is only allowed for loopback (localhost / 127.0.0.1)".into())
            }
        }
        _ => Err("redirect_uri scheme must be https (or http for loopback)".into()),
    }
}

/// Extract the host from the part after `scheme://`: `[userinfo@]host[:port][/path][?query]`.
fn host_of(rest: &str) -> &str {
    let authority = rest.split(['/', '?']).next().unwrap_or(rest);
    let authority = authority
        .rsplit_once('@')
        .map(|(_, h)| h)
        .unwrap_or(authority);
    if authority.starts_with('[') {
        // IPv6 literal: [host]:port
        if let Some(idx) = authority.find(']') {
            return &authority[1..idx];
        }
    }
    authority.split(':').next().unwrap_or(authority)
}

fn is_loopback(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

/// RFC 6749 (+ RFC 8707) error codes for the token / authorize endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthErrorCode {
    InvalidRequest,
    InvalidClient,
    InvalidGrant,
    UnauthorizedClient,
    UnsupportedGrantType,
    InvalidScope,
    InvalidTarget,
    AccessDenied,
    ServerError,
}

impl OAuthErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            OAuthErrorCode::InvalidRequest => "invalid_request",
            OAuthErrorCode::InvalidClient => "invalid_client",
            OAuthErrorCode::InvalidGrant => "invalid_grant",
            OAuthErrorCode::UnauthorizedClient => "unauthorized_client",
            OAuthErrorCode::UnsupportedGrantType => "unsupported_grant_type",
            OAuthErrorCode::InvalidScope => "invalid_scope",
            OAuthErrorCode::InvalidTarget => "invalid_target",
            OAuthErrorCode::AccessDenied => "access_denied",
            OAuthErrorCode::ServerError => "server_error",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_s256_matches_rfc7636_vector() {
        // RFC 7636 Appendix B.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert!(verify_pkce_s256(verifier, challenge));
        assert!(!verify_pkce_s256("the-wrong-verifier-the-wrong-verifier-xxxxx", challenge));
    }

    #[test]
    fn pkce_rejects_out_of_range_verifier() {
        // Too short (< 43 chars) — never compute the hash.
        assert!(!verify_pkce_s256("short", "anything"));
    }

    #[test]
    fn redirect_uri_rules() {
        assert!(validate_redirect_uri("https://claude.ai/api/mcp/auth_callback").is_ok());
        assert!(validate_redirect_uri("https://chatgpt.com/connector/oauth/abc").is_ok());
        assert!(validate_redirect_uri("http://localhost:6274/callback").is_ok());
        assert!(validate_redirect_uri("http://127.0.0.1:8080/cb").is_ok());
        // Rejected:
        assert!(validate_redirect_uri("http://evil.example.com/cb").is_err()); // http non-loopback
        assert!(validate_redirect_uri("https://claude.ai/cb#frag").is_err()); // fragment
        assert!(validate_redirect_uri("javascript:alert(1)").is_err()); // bad scheme
        assert!(validate_redirect_uri("ftp://host/cb").is_err());
        assert!(validate_redirect_uri("not-a-url").is_err());
    }

    #[test]
    fn token_schemes_are_disjoint_prefixes() {
        // The resource server branches on `mo_`; refresh/code/secret must not collide with it.
        assert!(generate_access_token().secret.starts_with("mo_"));
        assert!(generate_refresh_token().secret.starts_with("mor_"));
        assert!(generate_auth_code().secret.starts_with("moc_"));
        assert!(!generate_refresh_token().secret.starts_with("mo_"));
        assert!(!generate_auth_code().secret.starts_with("mo_"));
    }
}
