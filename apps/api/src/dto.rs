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
}
