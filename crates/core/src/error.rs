//! Typed domain errors. Binaries map these onto transport-specific responses
//! (HTTP status codes in `api`, MCP error payloads in `mcp`, exit codes in `cli`).

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("not found")]
    NotFound,

    #[error("permission denied")]
    Forbidden,

    /// Optimistic-concurrency conflict: the document changed since `expected`.
    /// The transport layer surfaces this as HTTP 409 carrying current + base content
    /// so the caller can perform a 3-way merge (see `docs/PLAN.md` §6).
    #[error("version conflict (expected {expected}, current {current})")]
    VersionConflict { expected: i64, current: i64 },

    #[error("invalid input: {0}")]
    Invalid(String),
}
