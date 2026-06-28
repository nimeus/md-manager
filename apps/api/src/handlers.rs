//! HTTP handlers. Thin wrappers over the `mdm-db` service; all rules live there.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use mdm_core::model::OrgRole;
use serde_json::json;
use uuid::Uuid;

use crate::dto::*;
use crate::error::ApiError;
use crate::state::{AppState, Auth};

type ApiResult<T> = Result<T, ApiError>;

fn clamp_limit(limit: Option<i64>, default: i64) -> i64 {
    limit.unwrap_or(default).clamp(1, 100)
}

pub async fn healthz() -> &'static str {
    "ok"
}

pub async fn bootstrap(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<BootstrapReq>,
) -> ApiResult<Response> {
    let presented = headers
        .get("x-bootstrap-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if presented.is_empty() || presented != s.bootstrap_token.as_str() {
        return Err(ApiError(mdm_core::Error::Unauthorized));
    }
    let (org, user, key) = s
        .db
        .bootstrap(&req.email, &req.display_name, &req.org_slug, &req.org_name, &req.key_name)
        .await?;
    Ok((StatusCode::CREATED, Json(json!({ "org": org, "user": user, "api_key": key }))).into_response())
}

pub async fn whoami(State(_s): State<AppState>, Auth(ctx): Auth) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!({
        "org_id": ctx.org_id,
        "user_id": ctx.user_id,
        "actor_type": ctx.actor_type,
        "role": ctx.org_role,
    })))
}

pub async fn list_orgs(State(s): State<AppState>, Auth(ctx): Auth) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(s.db.list_orgs(&ctx).await?)))
}

// --- projects --------------------------------------------------------------

pub async fn list_projects(State(s): State<AppState>, Auth(ctx): Auth) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(s.db.list_projects(&ctx).await?)))
}

pub async fn create_project(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Json(req): Json<CreateProjectReq>,
) -> ApiResult<Response> {
    let project = s.db.create_project(&ctx, &req.slug, &req.name).await?;
    Ok((StatusCode::CREATED, Json(project)).into_response())
}

pub async fn get_project(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(slug): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(s.db.get_project_by_slug(&ctx, &slug).await?)))
}

pub async fn list_documents(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(project_id): Path<Uuid>,
    Query(q): Query<ListDocsQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let docs = s.db.list_documents(&ctx, project_id, clamp_limit(q.limit, 50)).await?;
    Ok(Json(json!(docs)))
}

pub async fn create_document(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(project_id): Path<Uuid>,
    Json(req): Json<CreateDocReq>,
) -> ApiResult<Response> {
    let doc = s.db.create_document(&ctx, project_id, &req.path, &req.title, &req.content).await?;
    Ok((StatusCode::CREATED, Json(doc)).into_response())
}

// --- documents -------------------------------------------------------------

pub async fn get_document_by_path(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Query(q): Query<ByPathQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(s.db.get_document_by_path(&ctx, q.project_id, &q.path).await?)))
}

pub async fn get_document(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(s.db.get_document(&ctx, id).await?)))
}

pub async fn update_document(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateDocReq>,
) -> ApiResult<Response> {
    let kind = req.version_kind();
    match s.db.update_document(&ctx, id, &req.content, req.expected_version, kind).await? {
        mdm_db::UpdateOutcome::Updated(doc) => Ok((StatusCode::OK, Json(doc)).into_response()),
        mdm_db::UpdateOutcome::Conflict { current_version, current_content, base_content } => Ok((
            StatusCode::CONFLICT,
            Json(json!({
                "error": "conflict",
                "message": "the document was modified since expected_version",
                "current_version": current_version,
                "current_content": current_content,
                "base_content": base_content,
            })),
        )
            .into_response()),
    }
}

pub async fn append_document(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
    Json(req): Json<AppendReq>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(s.db.append_to_document(&ctx, id, &req.content).await?)))
}

pub async fn move_document(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
    Json(req): Json<MoveReq>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(s.db.move_document(&ctx, id, &req.path).await?)))
}

pub async fn delete_document(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    s.db.delete_document(&ctx, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn undelete_document(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(s.db.undelete_document(&ctx, id).await?)))
}

pub async fn history(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(s.db.get_history(&ctx, id).await?)))
}

pub async fn get_version(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path((id, version)): Path<(Uuid, i64)>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(s.db.get_version(&ctx, id, version).await?)))
}

pub async fn restore_version(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
    Json(req): Json<RestoreReq>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(s.db.restore_version(&ctx, id, req.version).await?)))
}

// --- tags ------------------------------------------------------------------

pub async fn list_tags(State(s): State<AppState>, Auth(ctx): Auth) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(s.db.list_tags(&ctx).await?)))
}

pub async fn list_document_tags(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(s.db.list_document_tags(&ctx, id).await?)))
}

pub async fn add_document_tag(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
    Json(req): Json<TagReq>,
) -> ApiResult<Response> {
    let tag = s.db.add_document_tag(&ctx, id, &req.name).await?;
    Ok((StatusCode::CREATED, Json(tag)).into_response())
}

// --- categories ------------------------------------------------------------

pub async fn list_categories(State(s): State<AppState>, Auth(ctx): Auth) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(s.db.list_categories(&ctx).await?)))
}

pub async fn create_category(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Json(req): Json<CreateCategoryReq>,
) -> ApiResult<Response> {
    let cat = s.db.create_category(&ctx, req.parent_id, &req.slug, &req.name).await?;
    Ok((StatusCode::CREATED, Json(cat)).into_response())
}

pub async fn list_category_documents(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(s.db.list_documents_in_category(&ctx, id).await?)))
}

pub async fn list_document_categories(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(s.db.list_document_categories(&ctx, id).await?)))
}

pub async fn categorize_document(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
    Json(req): Json<CategorizeReq>,
) -> ApiResult<StatusCode> {
    s.db.categorize_document(&ctx, id, req.category_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// --- search ----------------------------------------------------------------

pub async fn search(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Query(q): Query<SearchQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let hits = s.db.search(&ctx, q.project_id, &q.q, clamp_limit(q.limit, 20)).await?;
    Ok(Json(json!(hits)))
}

// --- api keys --------------------------------------------------------------

pub async fn list_api_keys(State(s): State<AppState>, Auth(ctx): Auth) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(s.db.list_api_keys(&ctx).await?)))
}

pub async fn create_api_key(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Json(req): Json<CreateKeyReq>,
) -> ApiResult<Response> {
    let role = match req.role.as_deref() {
        Some(r) => OrgRole::from_db(r)?,
        None => OrgRole::Member,
    };
    let key = s.db.create_api_key(&ctx, &req.name, role).await?;
    Ok((StatusCode::CREATED, Json(key)).into_response())
}

pub async fn revoke_api_key(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    s.db.revoke_api_key(&ctx, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
