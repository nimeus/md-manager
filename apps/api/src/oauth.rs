//! OAuth 2.1 resource-server token validation for the remote MCP endpoint.
//!
//! Validates RS256 access tokens issued by the configured authorization server (Logto):
//! signature against the cached JWKS, `iss`, `exp`/`nbf`, and crucially `aud` == the
//! canonical resource URI (RFC 8707 resource→audience binding — the most common silent
//! breakage). Extracts `sub` and the org claim. See `docs/PLAN.md` §5.

use jsonwebtoken::jwk::{Jwk, JwkSet};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use mdm_config::OAuthSettings;
use serde_json::Value;
use tokio::sync::RwLock;

#[derive(Debug)]
pub enum OAuthError {
    /// The token is missing/expired/wrong audience/bad signature, etc.
    Invalid(String),
    /// The JWKS could not be fetched.
    Transport(String),
}

impl std::fmt::Display for OAuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OAuthError::Invalid(m) => write!(f, "invalid token: {m}"),
            OAuthError::Transport(m) => write!(f, "JWKS fetch failed: {m}"),
        }
    }
}

/// Claims extracted from a validated access token.
pub struct OAuthClaims {
    pub sub: String,
    pub org: String,
}

/// Validates access tokens and caches the authorization server's JWKS.
pub struct OAuthValidator {
    issuer: String,
    audience: String,
    org_claim: String,
    jwks_url: String,
    http: reqwest::Client,
    cache: RwLock<JwkSet>,
}

impl OAuthValidator {
    pub fn new(settings: &OAuthSettings) -> Self {
        Self {
            issuer: settings.issuer.clone(),
            audience: settings.audience.clone(),
            org_claim: settings.org_claim.clone(),
            jwks_url: settings.jwks_url.clone(),
            http: reqwest::Client::new(),
            cache: RwLock::new(JwkSet { keys: Vec::new() }),
        }
    }

    async fn fetch_jwks(&self) -> Result<JwkSet, OAuthError> {
        self.http
            .get(&self.jwks_url)
            .send()
            .await
            .map_err(|e| OAuthError::Transport(e.to_string()))?
            .json::<JwkSet>()
            .await
            .map_err(|e| OAuthError::Transport(e.to_string()))
    }

    /// Find the signing key for `kid`, refreshing the cache once on a miss.
    async fn jwk_for(&self, kid: &str) -> Result<Jwk, OAuthError> {
        if let Some(jwk) = self.cache.read().await.find(kid).cloned() {
            return Ok(jwk);
        }
        let fresh = self.fetch_jwks().await?;
        let found = fresh.find(kid).cloned();
        *self.cache.write().await = fresh;
        found.ok_or_else(|| OAuthError::Invalid(format!("unknown key id: {kid}")))
    }

    /// Validate a bearer token and return its subject + org claim.
    pub async fn validate(&self, token: &str) -> Result<OAuthClaims, OAuthError> {
        let header = decode_header(token).map_err(|e| OAuthError::Invalid(e.to_string()))?;
        let kid = header
            .kid
            .ok_or_else(|| OAuthError::Invalid("token header missing kid".into()))?;
        let jwk = self.jwk_for(&kid).await?;
        let key = DecodingKey::from_jwk(&jwk).map_err(|e| OAuthError::Invalid(e.to_string()))?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[self.audience.as_str()]);
        validation.set_issuer(&[self.issuer.as_str()]);

        let data = decode::<Value>(token, &key, &validation)
            .map_err(|e| OAuthError::Invalid(e.to_string()))?;
        let claims = data.claims;
        let sub = claims
            .get("sub")
            .and_then(Value::as_str)
            .ok_or_else(|| OAuthError::Invalid("token missing sub".into()))?
            .to_string();
        let org = claims
            .get(&self.org_claim)
            .and_then(Value::as_str)
            .ok_or_else(|| {
                OAuthError::Invalid(format!("token missing org claim '{}'", self.org_claim))
            })?
            .to_string();
        Ok(OAuthClaims { sub, org })
    }

    #[cfg(test)]
    fn from_jwks(issuer: &str, audience: &str, org_claim: &str, jwks: JwkSet) -> Self {
        Self {
            issuer: issuer.into(),
            audience: audience.into(),
            org_claim: org_claim.into(),
            jwks_url: String::new(),
            http: reqwest::Client::new(),
            cache: RwLock::new(jwks),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{EncodingKey, Header};
    use serde_json::json;

    const TEST_KEY_PEM: &str = include_str!("../tests/fixtures/test_key.pem");
    // base64url modulus of the fixture key; exponent is 65537 ("AQAB"); kid "test".
    const TEST_N: &str = "x8C_6EfaL_Ri5KSPYhrrE7Rmitq49OQ5By4QwKcTJm7Qr3a_65kZrZaf901ZeMlqFOraazlZVftgkBSQijnZFyYzzsQ36GuuZhuZVm64lfCCvNGspXqW2Voej01CVGF_Stg2tzvZvx7F-ei7YN5_hkXOF6ijSL2y5piMAfEmhec8OW6LJmYAkvNAQWertt4hnf_KGDZWNBB1L8fjSQhp6_HIcTFZvhmph9n1KUvCgPxF6TC59tOBjDJsobeqA5E-OmExOoAZ1YOGT6XKjj4urhHTfm8aWHcOesUSUPAb9GLRprDfZWmIiFB1YqzwAkQvg5jTAbxF9fuD2fHxOq5W4Q";

    fn test_jwks() -> JwkSet {
        let v = json!({ "keys": [{
            "kty": "RSA", "use": "sig", "alg": "RS256", "kid": "test",
            "n": TEST_N, "e": "AQAB"
        }]});
        serde_json::from_value(v).expect("valid jwks")
    }

    fn mint(claims: Value) -> String {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some("test".into());
        let key = EncodingKey::from_rsa_pem(TEST_KEY_PEM.as_bytes()).expect("encoding key");
        jsonwebtoken::encode(&header, &claims, &key).expect("sign")
    }

    fn future_exp() -> i64 {
        // 2099-01-01; avoids needing the clock in tests.
        4_070_908_800
    }

    #[tokio::test]
    async fn accepts_valid_token_and_extracts_claims() {
        let v = OAuthValidator::from_jwks(
            "https://auth.example.com",
            "https://mcp.example.com",
            "org",
            test_jwks(),
        );
        let token = mint(json!({
            "sub": "logto|user123",
            "org": "019f0000-0000-7000-8000-000000000001",
            "aud": "https://mcp.example.com",
            "iss": "https://auth.example.com",
            "exp": future_exp()
        }));
        let claims = v.validate(&token).await.expect("valid");
        assert_eq!(claims.sub, "logto|user123");
        assert_eq!(claims.org, "019f0000-0000-7000-8000-000000000001");
    }

    #[tokio::test]
    async fn rejects_wrong_audience() {
        let v = OAuthValidator::from_jwks(
            "https://auth.example.com",
            "https://mcp.example.com",
            "org",
            test_jwks(),
        );
        let token = mint(json!({
            "sub": "u", "org": "o",
            "aud": "https://WRONG.example.com",
            "iss": "https://auth.example.com",
            "exp": future_exp()
        }));
        assert!(v.validate(&token).await.is_err(), "must reject wrong aud");
    }

    #[tokio::test]
    async fn rejects_expired_token() {
        let v = OAuthValidator::from_jwks(
            "https://auth.example.com",
            "https://mcp.example.com",
            "org",
            test_jwks(),
        );
        let token = mint(json!({
            "sub": "u", "org": "o",
            "aud": "https://mcp.example.com",
            "iss": "https://auth.example.com",
            "exp": 1_000_000_000  // year 2001
        }));
        assert!(
            v.validate(&token).await.is_err(),
            "must reject expired token"
        );
    }
}
