//! Request/response payloads for the HTTP API.

use mdm_core::model::VersionKind;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct BootstrapReq {
    pub email: String,
    pub display_name: String,
    pub org_slug: String,
    pub org_name: String,
    #[serde(default = "default_key_name")]
    pub key_name: String,
}
fn default_key_name() -> String {
    "default".to_string()
}

#[derive(Deserialize)]
pub struct CreateProjectReq {
    pub slug: String,
    pub name: String,
}

/// The Next.js BFF posts a verified Google ID token; the API confirms it independently.
#[derive(Deserialize)]
pub struct AuthGoogleReq {
    pub id_token: String,
}

#[derive(Deserialize)]
pub struct CreateOrgReq {
    pub slug: String,
    pub name: String,
}

#[derive(Deserialize)]
pub struct CreateInvitationReq {
    pub email: String,
    /// admin | member | viewer (default member)
    #[serde(default = "default_invite_role")]
    pub role: String,
}
fn default_invite_role() -> String {
    "member".to_string()
}

#[derive(Deserialize)]
pub struct CreateDocReq {
    pub path: String,
    pub title: String,
    #[serde(default)]
    pub content: String,
}

#[derive(Deserialize)]
pub struct UpdateDocReq {
    pub content: String,
    pub expected_version: i64,
    #[serde(default)]
    pub kind: Option<String>,
}

impl UpdateDocReq {
    pub fn version_kind(&self) -> VersionKind {
        match self.kind.as_deref() {
            Some("autosave") => VersionKind::Autosave,
            _ => VersionKind::Checkpoint,
        }
    }
}

#[derive(Deserialize)]
pub struct AppendReq {
    pub content: String,
}

#[derive(Deserialize)]
pub struct MoveReq {
    pub path: String,
}

#[derive(Deserialize)]
pub struct RestoreReq {
    pub version: i64,
}

#[derive(Deserialize)]
pub struct TagReq {
    pub name: String,
}

#[derive(Deserialize)]
pub struct CreateKeyReq {
    pub name: String,
    #[serde(default)]
    pub role: Option<String>,
}

#[derive(Deserialize)]
pub struct ListDocsQuery {
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct ByPathQuery {
    pub project_id: Uuid,
    pub path: String,
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub project_id: Option<Uuid>,
    pub limit: Option<i64>,
    /// keyword (default) | semantic | hybrid
    pub mode: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateShareReq {
    #[serde(default)]
    pub expires_in_days: Option<i64>,
}

#[derive(Deserialize)]
pub struct AuditQuery {
    pub limit: Option<i64>,
    pub target: Option<String>,
    pub action: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateCategoryReq {
    #[serde(default)]
    pub parent_id: Option<Uuid>,
    pub slug: String,
    pub name: String,
}

#[derive(Deserialize)]
pub struct CategorizeReq {
    pub category_id: Uuid,
}

#[derive(Deserialize)]
pub struct CreateTeamReq {
    pub slug: String,
    pub name: String,
}

#[derive(Deserialize)]
pub struct AddTeamMemberReq {
    pub user_id: Uuid,
}

/// A project/document grant. `subject_type` is "user" or "team"; for document grants
/// `role` may be "none" (an explicit deny).
#[derive(Deserialize)]
pub struct GrantReq {
    pub subject_type: String,
    pub subject_id: Uuid,
    pub role: String,
}
