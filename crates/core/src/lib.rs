//! `mdm-core` — the framework- and database-agnostic heart of md-manager.
//!
//! Pure domain logic shared by every surface: models, the permission model (RBAC),
//! 3-way merge for concurrent edits, header-aware markdown chunking for full-text
//! search, input validation, and crypto helpers (content hashing + API-key HMAC).
//!
//! The `db` crate orchestrates SQL/transactions and calls into this crate so the API,
//! MCP, and CLI surfaces all enforce identical rules. See `docs/PLAN.md` and `CLAUDE.md`.

pub mod chunk;
pub mod crypto;
pub mod error;
pub mod ids;
pub mod mcp;
pub mod merge;
pub mod model;
pub mod rbac;
pub mod validate;

pub use error::{Error, Result};
pub use model::{
    ActorType, ApiKeyCreated, ApiKeyInfo, AuthContext, Category, Document, DocumentSummary,
    DocumentVersion, Organization, OrgRole, Project, Role, SearchHit, Tag, User, VersionKind,
    VersionSummary,
};
