//! Content hashing and API-key crypto.
//!
//! - `content_hash`: SHA-256 of a document body (used for optimistic concurrency).
//! - API keys: a ≥256-bit random secret, stored as HMAC-SHA256(pepper, secret) and looked
//!   up by an indexed prefix, verified in constant time. See `docs/PLAN.md` §5.

use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

/// Hex-encoded SHA-256 of a document body.
pub fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

/// Hex-encoded HMAC-SHA256 of `msg` under `key`.
pub fn hmac_hex(key: &[u8], msg: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(msg);
    hex::encode(mac.finalize().into_bytes())
}

/// A freshly minted API key: the full secret (shown once) and its lookup prefix.
#[derive(Debug, Clone)]
pub struct GeneratedKey {
    pub secret: String,
    pub prefix: String,
}

/// Generate a new API key: `mk_<64 hex chars>` from 32 CSPRNG bytes.
pub fn generate_api_key() -> GeneratedKey {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    let secret = format!("mk_{}", hex::encode(bytes));
    let prefix = secret[..11].to_string(); // "mk_" + 8 hex chars
    GeneratedKey { secret, prefix }
}

/// The lookup prefix for a presented key (first 11 chars), if it looks well-formed.
pub fn key_prefix(secret: &str) -> Option<String> {
    if secret.starts_with("mk_") && secret.len() >= 11 {
        Some(secret[..11].to_string())
    } else {
        None
    }
}

/// Hash an API key for storage (HMAC-SHA256 with the server pepper).
pub fn hash_api_key(pepper: &str, secret: &str) -> String {
    hmac_hex(pepper.as_bytes(), secret.as_bytes())
}

/// Constant-time check that `secret` hashes to `stored_hash` under `pepper`.
pub fn verify_api_key(pepper: &str, secret: &str, stored_hash: &str) -> bool {
    let computed = hash_api_key(pepper, secret);
    computed.as_bytes().ct_eq(stored_hash.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hash_is_stable_and_distinct() {
        assert_eq!(content_hash("abc"), content_hash("abc"));
        assert_ne!(content_hash("abc"), content_hash("abd"));
        assert_eq!(content_hash("abc").len(), 64);
    }

    #[test]
    fn api_key_roundtrip() {
        let pepper = "pepper";
        let k = generate_api_key();
        assert!(k.secret.starts_with("mk_"));
        assert_eq!(k.prefix.len(), 11);
        assert_eq!(key_prefix(&k.secret).as_deref(), Some(k.prefix.as_str()));

        let hash = hash_api_key(pepper, &k.secret);
        assert!(verify_api_key(pepper, &k.secret, &hash));
        assert!(!verify_api_key(pepper, "mk_wrong", &hash));
        assert!(!verify_api_key("other-pepper", &k.secret, &hash));
    }
}
