//! Domain models. Phase 0 establishes the permission lattice and request context;
//! org/project/document/version/tag/category structs land in Phase 0/1 per `TODO.md`.

use uuid::Uuid;

/// Permission lattice: `None < Viewer < Commenter < Editor < Admin`.
///
/// Declaration order defines the ordering used by the RBAC resolver, so the derived
/// `Ord` lets us take the most-permissive grant with `.max()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    None,
    Viewer,
    Commenter,
    Editor,
    Admin,
}

/// Whether the actor behind a request is a human user or an agent (via API key / connector).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorType {
    User,
    Agent,
}

/// Resolved request context. Every surface (api/mcp/cli) produces one of these before
/// touching the database; `org_id` is bound into the Postgres RLS session by `mdm-db`.
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub org_id: Uuid,
    pub user_id: Uuid,
    pub actor_type: ActorType,
}
