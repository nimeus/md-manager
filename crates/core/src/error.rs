//! Typed domain errors. Each surface maps these to its transport (HTTP status in `api`,
//! MCP error payloads in `mcp`, exit codes in `cli`).

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("not found")]
    NotFound,

    #[error("authentication required or invalid credentials")]
    Unauthorized,

    #[error("permission denied")]
    Forbidden,

    /// Optimistic-concurrency conflict: the document changed since `expected`.
    /// Surfaced as HTTP 409 carrying current + base content for a 3-way merge
    /// (see `docs/PLAN.md` §6).
    #[error("version conflict (expected {expected}, current {current})")]
    Conflict { expected: i64, current: i64 },

    /// A uniqueness violation (e.g. a doc already exists at that path).
    #[error("already exists: {0}")]
    AlreadyExists(String),

    #[error("invalid input: {0}")]
    Invalid(String),

    /// Rate limit or resource quota exceeded. Surfaced as HTTP 429.
    #[error("too many requests: {0}")]
    TooManyRequests(String),

    /// An unexpected internal failure (DB error, etc.). Surfaced as HTTP 500;
    /// the detail is logged, not returned to the caller.
    #[error("internal error")]
    Internal(String),
}

impl Error {
    pub fn invalid(msg: impl Into<String>) -> Self {
        Error::Invalid(msg.into())
    }
}
