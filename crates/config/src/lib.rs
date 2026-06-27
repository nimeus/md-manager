//! `mdm-config` — layered configuration and observability setup shared by all binaries.
//!
//! Loads config from TOML + environment + profiles (figment), wraps secrets in
//! `secrecy::SecretString` (e.g. `DATABASE_URL`, the API-key HMAC pepper, Logto creds),
//! and initialises `tracing`. Implemented per `TODO.md` Phase 0.

// Intentionally empty until the config layer is implemented.
