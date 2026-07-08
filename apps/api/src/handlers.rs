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
    let (org, user, key) =
        s.db.bootstrap(
            &req.email,
            &req.display_name,
            &req.org_slug,
            &req.org_name,
            &req.key_name,
        )
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "org": org, "user": user, "api_key": key })),
    )
        .into_response())
}

// ---- web sign-in (Google) + orgs + invitations -----------------------------

/// Exchange a Google ID token (verified server-side) for a web session token, provisioning
/// the user + their orgs just in time. Called only by the Next.js BFF after a Google login.
pub async fn auth_google(
    State(s): State<AppState>,
    Json(req): Json<AuthGoogleReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let google = s
        .google
        .as_ref()
        .ok_or_else(|| ApiError(mdm_core::Error::invalid("Google sign-in is not configured")))?;
    let identity = google.validate(&req.id_token).await.map_err(|e| {
        tracing::debug!(%e, "google id_token rejected");
        ApiError(mdm_core::Error::Unauthorized)
    })?;
    let provisioned =
        s.db.provision_google_user(&identity.sub, &identity.email, &identity.name)
            .await?;
    let token = crate::session::issue(
        &s.session_secret,
        provisioned.user_id,
        s.session_ttl_secs,
        unix_now(),
    );
    Ok(Json(json!({
        "session_token": token,
        "user": {
            "id": provisioned.user_id,
            "email": provisioned.email,
            "name": provisioned.display_name,
        },
        "orgs": provisioned.orgs,
    })))
}

/// All organizations the authenticated user belongs to (powers the org switcher).
pub async fn list_my_orgs(
    State(s): State<AppState>,
    Auth(ctx): Auth,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(s.db.list_user_orgs(ctx.user_id).await?)))
}

/// Create a new organization; the caller becomes its owner.
pub async fn create_org(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Json(req): Json<CreateOrgReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let org = s.db.create_org(&ctx, &req.slug, &req.name).await?;
    Ok(Json(json!(org)))
}

/// Pending invitations for the caller's current org (owner/admin).
pub async fn list_invitations(
    State(s): State<AppState>,
    Auth(ctx): Auth,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(s.db.list_invitations(&ctx).await?)))
}

/// Invite a teammate by email to the caller's current org (owner/admin). Returns the one-time
/// token so the BFF can build a shareable accept link (and/or email it).
pub async fn create_invitation(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Json(req): Json<CreateInvitationReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let role = OrgRole::from_db(&req.role)?;
    let created = s.db.create_invitation(&ctx, &req.email, role).await?;
    Ok(Json(
        json!({ "invitation": created.invitation, "token": created.token }),
    ))
}

/// Revoke a pending invitation (owner/admin).
pub async fn revoke_invitation(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    s.db.revoke_invitation(&ctx, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn unix_now() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub async fn whoami(
    State(_s): State<AppState>,
    Auth(ctx): Auth,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!({
        "org_id": ctx.org_id,
        "user_id": ctx.user_id,
        "actor_type": ctx.actor_type,
        "role": ctx.org_role,
    })))
}

pub async fn list_orgs(
    State(s): State<AppState>,
    Auth(ctx): Auth,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(s.db.list_orgs(&ctx).await?)))
}

// --- projects --------------------------------------------------------------

pub async fn list_projects(
    State(s): State<AppState>,
    Auth(ctx): Auth,
) -> ApiResult<Json<serde_json::Value>> {
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
    let docs =
        s.db.list_documents(&ctx, project_id, clamp_limit(q.limit, 50))
            .await?;
    Ok(Json(json!(docs)))
}

pub async fn create_document(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(project_id): Path<Uuid>,
    Json(req): Json<CreateDocReq>,
) -> ApiResult<Response> {
    let doc =
        s.db.create_document(&ctx, project_id, &req.path, &req.title, &req.content)
            .await?;
    Ok((StatusCode::CREATED, Json(doc)).into_response())
}

// --- documents -------------------------------------------------------------

pub async fn get_document_by_path(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Query(q): Query<ByPathQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(
        s.db.get_document_by_path(&ctx, q.project_id, &q.path)
            .await?
    )))
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
    match s
        .db
        .update_document(&ctx, id, &req.content, req.expected_version, kind)
        .await?
    {
        mdm_db::UpdateOutcome::Updated(doc) => Ok((StatusCode::OK, Json(doc)).into_response()),
        mdm_db::UpdateOutcome::Conflict {
            current_version,
            current_content,
            base_content,
        } => Ok((
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
    Ok(Json(json!(
        s.db.append_to_document(&ctx, id, &req.content).await?
    )))
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
    Ok(Json(json!(
        s.db.restore_version(&ctx, id, req.version).await?
    )))
}

// --- tags ------------------------------------------------------------------

pub async fn list_tags(
    State(s): State<AppState>,
    Auth(ctx): Auth,
) -> ApiResult<Json<serde_json::Value>> {
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

pub async fn list_tag_documents(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(name): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(
        s.db.list_documents_with_tag(&ctx, &name, 200).await?
    )))
}

// --- categories ------------------------------------------------------------

pub async fn list_categories(
    State(s): State<AppState>,
    Auth(ctx): Auth,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(s.db.list_categories(&ctx).await?)))
}

pub async fn create_category(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Json(req): Json<CreateCategoryReq>,
) -> ApiResult<Response> {
    let cat =
        s.db.create_category(&ctx, req.parent_id, &req.slug, &req.name)
            .await?;
    Ok((StatusCode::CREATED, Json(cat)).into_response())
}

pub async fn list_category_documents(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(
        s.db.list_documents_in_category(&ctx, id).await?
    )))
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

// --- teams + grants --------------------------------------------------------

pub async fn list_teams(
    State(s): State<AppState>,
    Auth(ctx): Auth,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(s.db.list_teams(&ctx).await?)))
}

pub async fn create_team(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Json(req): Json<CreateTeamReq>,
) -> ApiResult<Response> {
    let team = s.db.create_team(&ctx, &req.slug, &req.name).await?;
    Ok((StatusCode::CREATED, Json(team)).into_response())
}

pub async fn add_team_member(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(team_id): Path<Uuid>,
    Json(req): Json<AddTeamMemberReq>,
) -> ApiResult<StatusCode> {
    s.db.add_team_member(&ctx, team_id, req.user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn grant_project(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(project_id): Path<Uuid>,
    Json(req): Json<GrantReq>,
) -> ApiResult<StatusCode> {
    let role = mdm_core::Role::from_db(&req.role)?;
    s.db.grant_project(&ctx, project_id, &req.subject_type, req.subject_id, role)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn grant_document(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(doc_id): Path<Uuid>,
    Json(req): Json<GrantReq>,
) -> ApiResult<StatusCode> {
    let role = mdm_core::Role::from_db(&req.role)?;
    s.db.grant_document(&ctx, doc_id, &req.subject_type, req.subject_id, role)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// --- share links -----------------------------------------------------------

pub async fn create_share(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
    Json(req): Json<CreateShareReq>,
) -> ApiResult<Response> {
    let link =
        s.db.create_share_link(
            &ctx,
            id,
            &req.audience,
            &req.recipients,
            req.expires_in_days,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(link)).into_response())
}

pub async fn list_shares(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(s.db.list_share_links(&ctx, id).await?)))
}

pub async fn revoke_share(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    s.db.revoke_share_link(&ctx, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// PUBLIC — no auth. The token is the authorization; invalid/expired/revoked → 404.
pub async fn get_shared(
    State(s): State<AppState>,
    headers: HeaderMap,
    Path(token): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    // Optional viewer identity (a web session) — required for `members`/`emails` shares.
    let viewer = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .filter(|t| t.starts_with(crate::session::SESSION_PREFIX))
        .and_then(|t| crate::session::verify(&s.session_secret, t).ok());
    Ok(Json(json!(s.db.resolve_share_link(&token, viewer).await?)))
}

// --- audit -----------------------------------------------------------------

pub async fn list_audit(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Query(q): Query<AuditQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let entries =
        s.db.list_audit(
            &ctx,
            q.target.as_deref(),
            q.action.as_deref(),
            clamp_limit(q.limit, 50),
        )
        .await?;
    Ok(Json(json!(entries)))
}

// --- search ----------------------------------------------------------------

pub async fn search(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Query(q): Query<SearchQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let mode = q.mode.as_deref().unwrap_or("keyword");
    let hits = run_search(&s, &ctx, q.project_id, &q.q, mode, clamp_limit(q.limit, 20)).await?;
    Ok(Json(json!(hits)))
}

/// Shared search dispatch (keyword | semantic | hybrid) used by the REST + MCP surfaces.
/// Semantic/hybrid embed the query via the configured embeddings provider.
pub async fn run_search(
    state: &AppState,
    ctx: &mdm_core::AuthContext,
    project_id: Option<Uuid>,
    query: &str,
    mode: &str,
    limit: i64,
) -> ApiResult<Vec<mdm_core::model::SearchHit>> {
    match mode {
        "semantic" | "hybrid" => {
            let embedder = state.embedder.as_ref().ok_or_else(|| {
                ApiError(mdm_core::Error::invalid(
                    "semantic search is not enabled on this server",
                ))
            })?;
            let inputs = [query.to_string()];
            let mut vectors = embedder.embed(&inputs).await.map_err(|e| {
                tracing::warn!(error = %e, "query embedding failed");
                ApiError(mdm_core::Error::Internal(e.to_string()))
            })?;
            let qvec = vectors
                .pop()
                .ok_or_else(|| ApiError(mdm_core::Error::Internal("empty embedding".into())))?;
            if mode == "hybrid" {
                Ok(state
                    .db
                    .hybrid_search(ctx, project_id, query, &qvec, limit)
                    .await?)
            } else {
                Ok(state
                    .db
                    .semantic_search(ctx, project_id, &qvec, limit)
                    .await?)
            }
        }
        _ => Ok(state.db.search(ctx, project_id, query, limit).await?),
    }
}

// --- api keys --------------------------------------------------------------

pub async fn list_api_keys(
    State(s): State<AppState>,
    Auth(ctx): Auth,
) -> ApiResult<Json<serde_json::Value>> {
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

// ---- Connected apps (built-in OAuth connector grants) -----------------------

/// List the signed-in user's active connector grants across their orgs.
pub async fn list_oauth_grants(
    State(s): State<AppState>,
    Auth(ctx): Auth,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(s.db.list_oauth_grants(&ctx).await?)))
}

/// Revoke a connector's grant in one org.
pub async fn revoke_oauth_grant(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(client_id): Path<String>,
    Json(req): Json<RevokeGrantReq>,
) -> ApiResult<StatusCode> {
    s.db.revoke_oauth_grant(&ctx, &client_id, req.org_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Move a live connection to another of the user's orgs (token re-binds; no reconnect).
pub async fn switch_oauth_grant(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(client_id): Path<String>,
    Json(req): Json<SwitchGrantReq>,
) -> ApiResult<StatusCode> {
    s.db.switch_oauth_grant(&ctx, &client_id, req.from_org_id, req.to_org_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---- members + invite acceptance --------------------------------------------

pub async fn list_members(
    State(s): State<AppState>,
    Auth(ctx): Auth,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!(s.db.list_members(&ctx).await?)))
}

pub async fn update_member(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(user_id): Path<Uuid>,
    Json(req): Json<UpdateMemberReq>,
) -> ApiResult<StatusCode> {
    s.db.update_member_role(&ctx, user_id, OrgRole::from_db(&req.role)?)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn remove_member(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Path(user_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    s.db.remove_member(&ctx, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Accept an invitation by its link token (session-authed). Returns the joined org.
pub async fn accept_invitation(
    State(s): State<AppState>,
    Auth(ctx): Auth,
    Json(req): Json<AcceptInviteReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let org =
        s.db.accept_invitation_by_token(ctx.user_id, &req.token)
            .await?;
    Ok(Json(json!(org)))
}
