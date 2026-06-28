//! Domain models, the role enums, and the resolved request context.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::Error;

/// A member's role within an organization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrgRole {
    Owner,
    Admin,
    Member,
    Viewer,
}

impl OrgRole {
    pub fn as_str(self) -> &'static str {
        match self {
            OrgRole::Owner => "owner",
            OrgRole::Admin => "admin",
            OrgRole::Member => "member",
            OrgRole::Viewer => "viewer",
        }
    }

    pub fn from_db(s: &str) -> Result<Self, Error> {
        match s {
            "owner" => Ok(OrgRole::Owner),
            "admin" => Ok(OrgRole::Admin),
            "member" => Ok(OrgRole::Member),
            "viewer" => Ok(OrgRole::Viewer),
            other => Err(Error::invalid(format!("unknown org role: {other}"))),
        }
    }

    /// Map an org role onto the document-capability lattice (see [`Role`]).
    pub fn capability(self) -> Role {
        match self {
            OrgRole::Owner | OrgRole::Admin => Role::Admin,
            OrgRole::Member => Role::Editor,
            OrgRole::Viewer => Role::Viewer,
        }
    }

    /// Numeric rank for comparing org roles (`owner` highest).
    pub fn rank(self) -> u8 {
        match self {
            OrgRole::Owner => 4,
            OrgRole::Admin => 3,
            OrgRole::Member => 2,
            OrgRole::Viewer => 1,
        }
    }

    /// The less-privileged of two roles (used to clamp an API key to its creator's role).
    pub fn min(self, other: OrgRole) -> OrgRole {
        if self.rank() <= other.rank() {
            self
        } else {
            other
        }
    }
}

/// Document permission lattice: `None < Viewer < Commenter < Editor < Admin`.
/// Declaration order defines the ordering used to take the most-permissive grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    None,
    Viewer,
    Commenter,
    Editor,
    Admin,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::None => "none",
            Role::Viewer => "viewer",
            Role::Commenter => "commenter",
            Role::Editor => "editor",
            Role::Admin => "admin",
        }
    }

    pub fn from_db(s: &str) -> Result<Self, Error> {
        match s {
            "none" => Ok(Role::None),
            "viewer" => Ok(Role::Viewer),
            "commenter" => Ok(Role::Commenter),
            "editor" => Ok(Role::Editor),
            "admin" => Ok(Role::Admin),
            other => Err(Error::invalid(format!("unknown role: {other}"))),
        }
    }
}

/// Whether the actor behind a request is a human user or an agent (API key / connector).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActorType {
    User,
    Agent,
}

impl ActorType {
    pub fn as_str(self) -> &'static str {
        match self {
            ActorType::User => "user",
            ActorType::Agent => "agent",
        }
    }

    pub fn from_db(s: &str) -> Result<Self, Error> {
        match s {
            "user" => Ok(ActorType::User),
            "agent" => Ok(ActorType::Agent),
            other => Err(Error::invalid(format!("unknown actor type: {other}"))),
        }
    }
}

/// Distinguishes human "checkpoint" saves from agent "autosave" churn (see `docs/PLAN.md` §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VersionKind {
    Checkpoint,
    Autosave,
}

impl VersionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            VersionKind::Checkpoint => "checkpoint",
            VersionKind::Autosave => "autosave",
        }
    }

    pub fn from_db(s: &str) -> Result<Self, Error> {
        match s {
            "checkpoint" => Ok(VersionKind::Checkpoint),
            "autosave" => Ok(VersionKind::Autosave),
            other => Err(Error::invalid(format!("unknown version kind: {other}"))),
        }
    }
}

/// The resolved principal for a request. Every surface produces one before touching the
/// database; `org_id` is bound into the Postgres RLS session by `mdm-db`.
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub org_id: Uuid,
    pub user_id: Uuid,
    pub actor_type: ActorType,
    /// Effective org role for this request (for an API key: min(key role, creator's role)).
    pub org_role: OrgRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: Uuid,
    pub org_id: Uuid,
    pub slug: String,
    pub name: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: Uuid,
    pub org_id: Uuid,
    pub project_id: Uuid,
    pub path: String,
    pub title: String,
    pub content: String,
    pub content_hash: String,
    pub current_version: i64,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

/// Listing view of a document (no body).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSummary {
    pub id: Uuid,
    pub project_id: Uuid,
    pub path: String,
    pub title: String,
    pub current_version: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentVersion {
    pub id: Uuid,
    pub document_id: Uuid,
    pub version: i64,
    pub content: String,
    pub content_hash: String,
    pub version_kind: VersionKind,
    pub actor_type: ActorType,
    pub actor_id: Uuid,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

/// History-listing view of a version (no body).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionSummary {
    pub version: i64,
    pub version_kind: VersionKind,
    pub actor_type: ActorType,
    pub actor_id: Uuid,
    pub content_hash: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: Uuid,
    pub name: String,
}

/// A team within an org — a named group of members that grants can target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

/// An org-scoped, hierarchical category (crosses projects). `parent_id` is `None` at the root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub slug: String,
    pub name: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

/// Public info about an API key (never includes the secret or its hash).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyInfo {
    pub id: Uuid,
    pub name: String,
    pub key_prefix: String,
    pub role: OrgRole,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_used_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub revoked_at: Option<OffsetDateTime>,
}

/// Returned exactly once when a key is minted; `secret` is shown only here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyCreated {
    #[serde(flatten)]
    pub info: ApiKeyInfo,
    pub secret: String,
}

/// Public info about a share link (never includes the token or its hash).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareLinkInfo {
    pub id: Uuid,
    pub document_id: Uuid,
    pub token_prefix: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub expires_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub revoked_at: Option<OffsetDateTime>,
}

/// Returned once when a share link is created; `token` is shown only here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareLinkCreated {
    #[serde(flatten)]
    pub info: ShareLinkInfo,
    pub token: String,
}

/// The read-only document view returned when resolving a (public) share link.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedDocument {
    pub document_id: Uuid,
    pub path: String,
    pub title: String,
    pub content: String,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

/// A full-text search hit, aggregated to the document level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub document_id: Uuid,
    pub project_id: Uuid,
    pub path: String,
    pub title: String,
    pub heading_path: String,
    pub snippet: String,
    pub rank: f32,
}
