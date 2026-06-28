//! Verify Google **ID tokens** server-side, so the backend independently confirms a Google
//! sign-in (it doesn't just trust the BFF's say-so). Same shape as [`crate::oauth`]: RS256 against
//! Google's cached JWKS, `iss` ∈ {accounts.google.com, https://accounts.google.com}, and
//! `aud` == our OAuth client id. Extracts the subject + verified email + name.

use jsonwebtoken::jwk::{Jwk, JwkSet};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde_json::Value;
use tokio::sync::RwLock;

const GOOGLE_JWKS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";
const GOOGLE_ISSUERS: [&str; 2] = ["https://accounts.google.com", "accounts.google.com"];

#[derive(Debug)]
pub enum GoogleError {
    Invalid(String),
    Transport(String),
}

impl std::fmt::Display for GoogleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GoogleError::Invalid(m) => write!(f, "invalid Google token: {m}"),
            GoogleError::Transport(m) => write!(f, "Google JWKS fetch failed: {m}"),
        }
    }
}

/// A verified Google identity from an ID token.
pub struct GoogleIdentity {
    pub sub: String,
    pub email: String,
    pub name: String,
}

/// Validates Google ID tokens and caches Google's JWKS.
pub struct GoogleValidator {
    client_id: String,
    http: reqwest::Client,
    cache: RwLock<JwkSet>,
}

impl GoogleValidator {
    pub fn new(client_id: &str) -> Self {
        Self {
            client_id: client_id.to_string(),
            http: reqwest::Client::new(),
            cache: RwLock::new(JwkSet { keys: Vec::new() }),
        }
    }

    async fn fetch_jwks(&self) -> Result<JwkSet, GoogleError> {
        self.http
            .get(GOOGLE_JWKS_URL)
            .send()
            .await
            .map_err(|e| GoogleError::Transport(e.to_string()))?
            .json::<JwkSet>()
            .await
            .map_err(|e| GoogleError::Transport(e.to_string()))
    }

    async fn jwk_for(&self, kid: &str) -> Result<Jwk, GoogleError> {
        if let Some(jwk) = self.cache.read().await.find(kid).cloned() {
            return Ok(jwk);
        }
        let fresh = self.fetch_jwks().await?;
        let found = fresh.find(kid).cloned();
        *self.cache.write().await = fresh;
        found.ok_or_else(|| GoogleError::Invalid(format!("unknown key id: {kid}")))
    }

    /// Validate a Google ID token and return its subject, verified email, and name.
    pub async fn validate(&self, id_token: &str) -> Result<GoogleIdentity, GoogleError> {
        let header = decode_header(id_token).map_err(|e| GoogleError::Invalid(e.to_string()))?;
        let kid = header
            .kid
            .ok_or_else(|| GoogleError::Invalid("token header missing kid".into()))?;
        let jwk = self.jwk_for(&kid).await?;
        let key = DecodingKey::from_jwk(&jwk).map_err(|e| GoogleError::Invalid(e.to_string()))?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[self.client_id.as_str()]);
        validation.set_issuer(&GOOGLE_ISSUERS);

        let data = decode::<Value>(id_token, &key, &validation)
            .map_err(|e| GoogleError::Invalid(e.to_string()))?;
        let c = data.claims;

        // Require a verified email (Google sends `email_verified` as a bool, occasionally a string).
        let verified = match c.get("email_verified") {
            Some(Value::Bool(b)) => *b,
            Some(Value::String(s)) => s == "true",
            _ => false,
        };
        if !verified {
            return Err(GoogleError::Invalid("email is not verified".into()));
        }
        let sub = str_claim(&c, "sub")?;
        let email = str_claim(&c, "email")?;
        let name = c
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        Ok(GoogleIdentity { sub, email, name })
    }

    #[cfg(test)]
    fn from_jwks(client_id: &str, jwks: JwkSet) -> Self {
        Self {
            client_id: client_id.into(),
            http: reqwest::Client::new(),
            cache: RwLock::new(jwks),
        }
    }
}

fn str_claim(claims: &Value, key: &str) -> Result<String, GoogleError> {
    claims
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| GoogleError::Invalid(format!("token missing '{key}'")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{EncodingKey, Header};
    use serde_json::json;

    const TEST_KEY_PEM: &str = include_str!("../tests/fixtures/test_key.pem");
    const TEST_N: &str = "x8C_6EfaL_Ri5KSPYhrrE7Rmitq49OQ5By4QwKcTJm7Qr3a_65kZrZaf901ZeMlqFOraazlZVftgkBSQijnZFyYzzsQ36GuuZhuZVm64lfCCvNGspXqW2Voej01CVGF_Stg2tzvZvx7F-ei7YN5_hkXOF6ijSL2y5piMAfEmhec8OW6LJmYAkvNAQWertt4hnf_KGDZWNBB1L8fjSQhp6_HIcTFZvhmph9n1KUvCgPxF6TC59tOBjDJsobeqA5E-OmExOoAZ1YOGT6XKjj4urhHTfm8aWHcOesUSUPAb9GLRprDfZWmIiFB1YqzwAkQvg5jTAbxF9fuD2fHxOq5W4Q";

    fn test_jwks() -> JwkSet {
        serde_json::from_value(json!({ "keys": [{
            "kty": "RSA", "use": "sig", "alg": "RS256", "kid": "test", "n": TEST_N, "e": "AQAB"
        }]}))
        .expect("valid jwks")
    }

    fn mint(claims: Value) -> String {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some("test".into());
        let key = EncodingKey::from_rsa_pem(TEST_KEY_PEM.as_bytes()).expect("encoding key");
        jsonwebtoken::encode(&header, &claims, &key).expect("sign")
    }

    const FUTURE_EXP: i64 = 4_070_908_800; // 2099-01-01

    #[tokio::test]
    async fn accepts_valid_google_token() {
        let v = GoogleValidator::from_jwks("client-123.apps.googleusercontent.com", test_jwks());
        let token = mint(json!({
            "sub": "google-sub-1", "email": "alice@gmail.com", "email_verified": true,
            "name": "Alice", "aud": "client-123.apps.googleusercontent.com",
            "iss": "https://accounts.google.com", "exp": FUTURE_EXP
        }));
        let id = v.validate(&token).await.expect("valid");
        assert_eq!(id.sub, "google-sub-1");
        assert_eq!(id.email, "alice@gmail.com");
        assert_eq!(id.name, "Alice");
    }

    #[tokio::test]
    async fn rejects_unverified_email() {
        let v = GoogleValidator::from_jwks("client-123.apps.googleusercontent.com", test_jwks());
        let token = mint(json!({
            "sub": "s", "email": "x@gmail.com", "email_verified": false,
            "aud": "client-123.apps.googleusercontent.com",
            "iss": "https://accounts.google.com", "exp": FUTURE_EXP
        }));
        assert!(
            v.validate(&token).await.is_err(),
            "must reject unverified email"
        );
    }

    #[tokio::test]
    async fn rejects_wrong_audience() {
        let v = GoogleValidator::from_jwks("client-123.apps.googleusercontent.com", test_jwks());
        let token = mint(json!({
            "sub": "s", "email": "x@gmail.com", "email_verified": true,
            "aud": "some-other-client", "iss": "https://accounts.google.com", "exp": FUTURE_EXP
        }));
        assert!(v.validate(&token).await.is_err(), "must reject wrong aud");
    }
}
