//! `FromRow` structs mirroring table columns, with conversions to `mdm-core` models.
//! Enum columns are stored as text and converted here.

use mdm_core::model::{
    ActorType, ApiKeyInfo, Category, Document, DocumentSummary, DocumentVersion, OrgRole,
    Organization, Project, SearchHit, ShareLinkInfo, SharedDocument, Tag, Team, User, VersionKind,
    VersionSummary,
};
use mdm_core::{Error, Result};
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
}
impl From<UserRow> for User {
    fn from(r: UserRow) -> Self {
        User {
            id: r.id,
            email: r.email,
            display_name: r.display_name,
        }
    }
}

#[derive(FromRow)]
pub struct OrgRow {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub created_at: OffsetDateTime,
}
impl From<OrgRow> for Organization {
    fn from(r: OrgRow) -> Self {
        Organization {
            id: r.id,
            slug: r.slug,
            name: r.name,
            created_at: r.created_at,
        }
    }
}

#[derive(FromRow)]
pub struct ProjectRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub slug: String,
    pub name: String,
    pub created_at: OffsetDateTime,
}
impl From<ProjectRow> for Project {
    fn from(r: ProjectRow) -> Self {
        Project {
            id: r.id,
            org_id: r.org_id,
            slug: r.slug,
            name: r.name,
            created_at: r.created_at,
        }
    }
}

#[derive(FromRow)]
pub struct DocumentRow {
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
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}
impl From<DocumentRow> for Document {
    fn from(r: DocumentRow) -> Self {
        Document {
            id: r.id,
            org_id: r.org_id,
            project_id: r.project_id,
            path: r.path,
            title: r.title,
            content: r.content,
            content_hash: r.content_hash,
            current_version: r.current_version,
            created_by: r.created_by,
            updated_by: r.updated_by,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(FromRow)]
pub struct DocSummaryRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub path: String,
    pub title: String,
    pub current_version: i64,
    pub updated_at: OffsetDateTime,
}
impl From<DocSummaryRow> for DocumentSummary {
    fn from(r: DocSummaryRow) -> Self {
        DocumentSummary {
            id: r.id,
            project_id: r.project_id,
            path: r.path,
            title: r.title,
            current_version: r.current_version,
            updated_at: r.updated_at,
        }
    }
}

#[derive(FromRow)]
pub struct VersionRow {
    pub id: Uuid,
    pub document_id: Uuid,
    pub version: i64,
    pub content: String,
    pub content_hash: String,
    pub version_kind: String,
    pub actor_type: String,
    pub actor_id: Uuid,
    pub created_at: OffsetDateTime,
}
impl VersionRow {
    pub fn into_core(self) -> Result<DocumentVersion> {
        Ok(DocumentVersion {
            id: self.id,
            document_id: self.document_id,
            version: self.version,
            content: self.content,
            content_hash: self.content_hash,
            version_kind: VersionKind::from_db(&self.version_kind)?,
            actor_type: ActorType::from_db(&self.actor_type)?,
            actor_id: self.actor_id,
            created_at: self.created_at,
        })
    }
}

#[derive(FromRow)]
pub struct VersionSummaryRow {
    pub version: i64,
    pub version_kind: String,
    pub actor_type: String,
    pub actor_id: Uuid,
    pub content_hash: String,
    pub created_at: OffsetDateTime,
}
impl VersionSummaryRow {
    pub fn into_core(self) -> Result<VersionSummary> {
        Ok(VersionSummary {
            version: self.version,
            version_kind: VersionKind::from_db(&self.version_kind)?,
            actor_type: ActorType::from_db(&self.actor_type)?,
            actor_id: self.actor_id,
            content_hash: self.content_hash,
            created_at: self.created_at,
        })
    }
}

#[derive(FromRow)]
pub struct TagRow {
    pub id: Uuid,
    pub name: String,
}
impl From<TagRow> for Tag {
    fn from(r: TagRow) -> Self {
        Tag {
            id: r.id,
            name: r.name,
        }
    }
}

#[derive(FromRow)]
pub struct TeamRow {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub created_at: OffsetDateTime,
}
impl From<TeamRow> for Team {
    fn from(r: TeamRow) -> Self {
        Team {
            id: r.id,
            slug: r.slug,
            name: r.name,
            created_at: r.created_at,
        }
    }
}

#[derive(FromRow)]
pub struct CategoryRow {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub slug: String,
    pub name: String,
    pub created_at: OffsetDateTime,
}
impl From<CategoryRow> for Category {
    fn from(r: CategoryRow) -> Self {
        Category {
            id: r.id,
            parent_id: r.parent_id,
            slug: r.slug,
            name: r.name,
            created_at: r.created_at,
        }
    }
}

#[derive(FromRow)]
pub struct ApiKeyInfoRow {
    pub id: Uuid,
    pub name: String,
    pub key_prefix: String,
    pub role: String,
    pub created_at: OffsetDateTime,
    pub last_used_at: Option<OffsetDateTime>,
    pub revoked_at: Option<OffsetDateTime>,
}
impl ApiKeyInfoRow {
    pub fn into_core(self) -> Result<ApiKeyInfo> {
        Ok(ApiKeyInfo {
            id: self.id,
            name: self.name,
            key_prefix: self.key_prefix,
            role: OrgRole::from_db(&self.role)?,
            created_at: self.created_at,
            last_used_at: self.last_used_at,
            revoked_at: self.revoked_at,
        })
    }
}

/// Row used during API-key authentication (cross-org lookup by prefix).
#[derive(FromRow)]
pub struct ApiKeyAuthRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub key_hash: String,
    pub role: String,
    pub created_by: Uuid,
    pub revoked_at: Option<OffsetDateTime>,
    pub expires_at: Option<OffsetDateTime>,
}
impl ApiKeyAuthRow {
    pub fn role(&self) -> Result<OrgRole> {
        OrgRole::from_db(&self.role)
    }
}

#[derive(FromRow)]
pub struct SearchRow {
    pub document_id: Uuid,
    pub project_id: Uuid,
    pub path: String,
    pub title: String,
    pub heading_path: String,
    pub snippet: String,
    pub rank: f32,
}
impl From<SearchRow> for SearchHit {
    fn from(r: SearchRow) -> Self {
        SearchHit {
            document_id: r.document_id,
            project_id: r.project_id,
            path: r.path,
            title: r.title,
            heading_path: r.heading_path,
            snippet: r.snippet,
            rank: r.rank,
        }
    }
}

#[derive(FromRow)]
pub struct ShareLinkInfoRow {
    pub id: Uuid,
    pub document_id: Uuid,
    pub token_prefix: String,
    pub created_at: OffsetDateTime,
    pub expires_at: Option<OffsetDateTime>,
    pub revoked_at: Option<OffsetDateTime>,
}
impl From<ShareLinkInfoRow> for ShareLinkInfo {
    fn from(r: ShareLinkInfoRow) -> Self {
        ShareLinkInfo {
            id: r.id,
            document_id: r.document_id,
            token_prefix: r.token_prefix,
            created_at: r.created_at,
            expires_at: r.expires_at,
            revoked_at: r.revoked_at,
        }
    }
}

/// Row used to resolve a presented share token (cross-org lookup by prefix).
#[derive(FromRow)]
pub struct ShareLinkAuthRow {
    pub org_id: Uuid,
    pub document_id: Uuid,
    pub token_hash: String,
    pub expires_at: Option<OffsetDateTime>,
    pub revoked_at: Option<OffsetDateTime>,
}

#[derive(FromRow)]
pub struct SharedDocRow {
    pub document_id: Uuid,
    pub path: String,
    pub title: String,
    pub content: String,
    pub updated_at: OffsetDateTime,
}
impl From<SharedDocRow> for SharedDocument {
    fn from(r: SharedDocRow) -> Self {
        SharedDocument {
            document_id: r.document_id,
            path: r.path,
            title: r.title,
            content: r.content,
            updated_at: r.updated_at,
        }
    }
}

/// Helper: turn a missing role string into an invalid-input error.
#[allow(dead_code)]
pub fn parse_org_role(s: &str) -> Result<OrgRole> {
    OrgRole::from_db(s).map_err(|_| Error::invalid("invalid role"))
}
