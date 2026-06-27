//! `mdm-core` — the framework- and database-agnostic heart of md-manager.
//!
//! Holds domain models, repository **traits**, the RBAC resolver, and typed errors.
//! The `api`, `mcp`, and `cli` binaries are thin transport wiring over the services
//! defined here, so all three enforce identical validation, RBAC, versioning, and
//! concurrency rules. See `docs/PLAN.md` and `CLAUDE.md`.
//!
//! This is the Phase 0 skeleton; modules are fleshed out per `TODO.md`.

pub mod error;
pub mod model;
pub mod rbac;

pub use error::{Error, Result};
pub use model::{ActorType, AuthContext, Role};
