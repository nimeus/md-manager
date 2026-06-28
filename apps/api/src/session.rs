//! Web session tokens. After a Google sign-in is verified and the user provisioned, the API
//! issues its own signed session token (HS256 over a server secret) identifying the user. The
//! BFF stores it in the httpOnly session cookie and sends it as `Authorization: Bearer mss_…`
//! on every call; the org is chosen per-request via the `X-Org-Id` header (the org switcher).
//!
//! The `mss_` prefix keeps these unambiguous from `mk_` API keys and from RS256 OAuth/Logto
//! JWTs, so the auth extractor can route on it cheaply.

use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const SESSION_PREFIX: &str = "mss_";
const SESSION_TYP: &str = "mdm_session";

#[derive(Serialize, Deserialize)]
struct SessionClaims {
    sub: String, // user id
    typ: String,
    exp: i64,
    iat: i64,
}

/// Mint a session token for `user_id`, valid for `ttl_secs`. `now` is the current unix time
/// (passed in so this stays pure/testable).
pub fn issue(secret: &str, user_id: Uuid, ttl_secs: i64, now: i64) -> String {
    let claims = SessionClaims {
        sub: user_id.to_string(),
        typ: SESSION_TYP.to_string(),
        exp: now + ttl_secs,
        iat: now,
    };
    let token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .expect("HS256 session encode never fails");
    format!("{SESSION_PREFIX}{token}")
}

/// Verify a `mss_…` session token and return the user id. Checks signature, type, and expiry.
pub fn verify(secret: &str, token: &str) -> Result<Uuid, String> {
    let raw = token
        .strip_prefix(SESSION_PREFIX)
        .ok_or_else(|| "not a session token".to_string())?;
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_required_spec_claims(&["exp"]);
    let data = decode::<SessionClaims>(
        raw,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|e| e.to_string())?;
    if data.claims.typ != SESSION_TYP {
        return Err("wrong token type".into());
    }
    Uuid::parse_str(&data.claims.sub).map_err(|_| "bad subject".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    // exp = now + ttl; verify() checks exp against the real clock, so issue tokens whose exp is
    // far in the future (now=0, huge ttl ≈ year 2286) to stay clock-independent.
    const FAR_TTL: i64 = 10_000_000_000;

    #[test]
    fn round_trips() {
        let uid = Uuid::from_u128(0x019f_0000_0000_7000_8000_0000_0000_0001);
        let token = issue("secret", uid, FAR_TTL, 0);
        assert!(token.starts_with(SESSION_PREFIX));
        assert_eq!(verify("secret", &token).unwrap(), uid);
    }

    #[test]
    fn rejects_wrong_secret() {
        let token = issue("secret", Uuid::nil(), FAR_TTL, 0);
        assert!(verify("other-secret", &token).is_err());
    }

    #[test]
    fn rejects_expired() {
        // issued far in the past with a 1s ttl
        let token = issue("secret", Uuid::nil(), 1, 1_000_000_000);
        assert!(verify("secret", &token).is_err(), "expired token must fail");
    }

    #[test]
    fn rejects_non_session_prefix() {
        assert!(verify("secret", "mk_whatever").is_err());
    }
}
