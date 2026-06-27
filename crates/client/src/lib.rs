//! Async HTTP client for the md-manager API.
//!
//! Returns `serde_json::Value` (the API's JSON) to keep the client decoupled from the
//! server's internal types; callers format as needed. Shared by the MCP server and CLI.

use serde_json::{Value, json};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("{message}")]
    Api {
        status: u16,
        code: String,
        message: String,
    },
    #[error("request failed: {0}")]
    Transport(String),
}

impl ClientError {
    pub fn status(&self) -> Option<u16> {
        match self {
            ClientError::Api { status, .. } => Some(*status),
            ClientError::Transport(_) => None,
        }
    }
}

/// Result of an update: either the updated document, or a version conflict carrying the
/// data needed for a 3-way merge.
pub enum UpdateResult {
    Updated(Value),
    Conflict {
        current_version: i64,
        current_content: String,
        base_content: String,
    },
}

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    base: String,
    token: String,
}

type R<T> = Result<T, ClientError>;

impl Client {
    pub fn new(base_url: impl Into<String>, token: impl Into<String>) -> Self {
        let base = base_url.into().trim_end_matches('/').to_string();
        Client {
            http: reqwest::Client::new(),
            base,
            token: token.into(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    async fn run(&self, rb: reqwest::RequestBuilder) -> R<Value> {
        let resp = rb
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;
        if status.is_success() {
            if text.trim().is_empty() {
                return Ok(Value::Null);
            }
            serde_json::from_str(&text).map_err(|e| ClientError::Transport(e.to_string()))
        } else {
            Err(api_error(status.as_u16(), &text))
        }
    }

    // --- identity / orgs / projects --------------------------------------

    pub async fn whoami(&self) -> R<Value> {
        self.run(self.http.get(self.url("/v1/me"))).await
    }

    pub async fn list_orgs(&self) -> R<Value> {
        self.run(self.http.get(self.url("/v1/orgs"))).await
    }

    pub async fn list_projects(&self) -> R<Value> {
        self.run(self.http.get(self.url("/v1/projects"))).await
    }

    pub async fn create_project(&self, slug: &str, name: &str) -> R<Value> {
        self.run(
            self.http
                .post(self.url("/v1/projects"))
                .json(&json!({ "slug": slug, "name": name })),
        )
        .await
    }

    pub async fn get_project(&self, slug: &str) -> R<Value> {
        self.run(self.http.get(self.url(&format!("/v1/projects/{slug}"))))
            .await
    }

    // --- documents -------------------------------------------------------

    pub async fn list_documents(&self, project_id: &str, limit: Option<i64>) -> R<Value> {
        let mut url = self.url(&format!("/v1/projects/{project_id}/documents"));
        if let Some(l) = limit {
            url.push_str(&format!("?limit={l}"));
        }
        self.run(self.http.get(url)).await
    }

    pub async fn create_document(
        &self,
        project_id: &str,
        path: &str,
        title: &str,
        content: &str,
    ) -> R<Value> {
        self.run(
            self.http
                .post(self.url(&format!("/v1/projects/{project_id}/documents")))
                .json(&json!({ "path": path, "title": title, "content": content })),
        )
        .await
    }

    pub async fn get_document(&self, id: &str) -> R<Value> {
        self.run(self.http.get(self.url(&format!("/v1/documents/{id}"))))
            .await
    }

    pub async fn get_document_by_path(&self, project_id: &str, path: &str) -> R<Value> {
        self.run(
            self.http
                .get(self.url("/v1/documents/by-path"))
                .query(&[("project_id", project_id), ("path", path)]),
        )
        .await
    }

    pub async fn update_document(
        &self,
        id: &str,
        content: &str,
        expected_version: i64,
        kind: &str,
    ) -> R<UpdateResult> {
        let resp = self
            .http
            .put(self.url(&format!("/v1/documents/{id}")))
            .bearer_auth(&self.token)
            .json(&json!({
                "content": content,
                "expected_version": expected_version,
                "kind": kind,
            }))
            .send()
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;
        let status = resp.status();
        let v: Value = resp
            .json()
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;
        if status.is_success() {
            Ok(UpdateResult::Updated(v))
        } else if status.as_u16() == 409 {
            Ok(UpdateResult::Conflict {
                current_version: v["current_version"].as_i64().unwrap_or(0),
                current_content: v["current_content"].as_str().unwrap_or_default().to_string(),
                base_content: v["base_content"].as_str().unwrap_or_default().to_string(),
            })
        } else {
            Err(api_error(status.as_u16(), &v.to_string()))
        }
    }

    pub async fn append_document(&self, id: &str, content: &str) -> R<Value> {
        self.run(
            self.http
                .post(self.url(&format!("/v1/documents/{id}/append")))
                .json(&json!({ "content": content })),
        )
        .await
    }

    pub async fn move_document(&self, id: &str, new_path: &str) -> R<Value> {
        self.run(
            self.http
                .post(self.url(&format!("/v1/documents/{id}/move")))
                .json(&json!({ "path": new_path })),
        )
        .await
    }

    pub async fn delete_document(&self, id: &str) -> R<Value> {
        self.run(self.http.delete(self.url(&format!("/v1/documents/{id}"))))
            .await
    }

    pub async fn undelete_document(&self, id: &str) -> R<Value> {
        self.run(self.http.post(self.url(&format!("/v1/documents/{id}/undelete"))))
            .await
    }

    pub async fn history(&self, id: &str) -> R<Value> {
        self.run(self.http.get(self.url(&format!("/v1/documents/{id}/history"))))
            .await
    }

    pub async fn restore_version(&self, id: &str, version: i64) -> R<Value> {
        self.run(
            self.http
                .post(self.url(&format!("/v1/documents/{id}/restore")))
                .json(&json!({ "version": version })),
        )
        .await
    }

    // --- tags / search ---------------------------------------------------

    pub async fn list_tags(&self) -> R<Value> {
        self.run(self.http.get(self.url("/v1/tags"))).await
    }

    pub async fn add_document_tag(&self, id: &str, name: &str) -> R<Value> {
        self.run(
            self.http
                .post(self.url(&format!("/v1/documents/{id}/tags")))
                .json(&json!({ "name": name })),
        )
        .await
    }

    pub async fn search(&self, query: &str, project_id: Option<&str>, limit: Option<i64>) -> R<Value> {
        let mut q: Vec<(String, String)> = vec![("q".into(), query.to_string())];
        if let Some(p) = project_id {
            q.push(("project_id".into(), p.to_string()));
        }
        if let Some(l) = limit {
            q.push(("limit".into(), l.to_string()));
        }
        self.run(self.http.get(self.url("/v1/search")).query(&q)).await
    }

    // --- api keys --------------------------------------------------------

    pub async fn list_api_keys(&self) -> R<Value> {
        self.run(self.http.get(self.url("/v1/api-keys"))).await
    }

    pub async fn create_api_key(&self, name: &str, role: &str) -> R<Value> {
        self.run(
            self.http
                .post(self.url("/v1/api-keys"))
                .json(&json!({ "name": name, "role": role })),
        )
        .await
    }

    pub async fn revoke_api_key(&self, id: &str) -> R<Value> {
        self.run(self.http.delete(self.url(&format!("/v1/api-keys/{id}"))))
            .await
    }

    // --- bootstrap (unauthenticated; uses the bootstrap token) -----------

    pub async fn bootstrap(
        &self,
        bootstrap_token: &str,
        email: &str,
        display_name: &str,
        org_slug: &str,
        org_name: &str,
        key_name: &str,
    ) -> R<Value> {
        let resp = self
            .http
            .post(self.url("/v1/bootstrap"))
            .header("x-bootstrap-token", bootstrap_token)
            .json(&json!({
                "email": email,
                "display_name": display_name,
                "org_slug": org_slug,
                "org_name": org_name,
                "key_name": key_name,
            }))
            .send()
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if status.is_success() {
            serde_json::from_str(&text).map_err(|e| ClientError::Transport(e.to_string()))
        } else {
            Err(api_error(status.as_u16(), &text))
        }
    }
}

fn api_error(status: u16, body: &str) -> ClientError {
    let (code, message) = serde_json::from_str::<Value>(body)
        .ok()
        .map(|v| {
            (
                v["error"].as_str().unwrap_or("error").to_string(),
                v["message"].as_str().unwrap_or(body).to_string(),
            )
        })
        .unwrap_or_else(|| ("error".to_string(), body.to_string()));
    ClientError::Api { status, code, message }
}
