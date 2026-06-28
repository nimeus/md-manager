//! Remote MCP over Streamable HTTP (`POST /mcp`) + OAuth discovery (`/.well-known/...`).
//!
//! Same 15 tools as the stdio server (schemas shared via `mdm_core::mcp`), but dispatched
//! directly to the db service. Auth is the dual scheme (API key or OAuth JWT); on a missing/
//! invalid token the endpoint returns 401 with a `WWW-Authenticate` challenge pointing at the
//! protected-resource metadata (per the MCP authorization spec).

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    response::{IntoResponse, Response},
};
use mdm_core::mcp::{PROTOCOL_FALLBACK, SERVER_NAME, tool_definitions};
use mdm_core::model::{AuthContext, SearchHit, VersionKind};
use mdm_db::UpdateOutcome;
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::state::{AppState, authenticate};

/// RFC 9728 Protected Resource Metadata. 404 when OAuth isn't configured.
pub async fn protected_resource_metadata(State(s): State<AppState>) -> Response {
    let Some(issuer) = &s.issuer else {
        return StatusCode::NOT_FOUND.into_response();
    };
    Json(json!({
        "resource": s.resource_url.as_str(),
        "authorization_servers": [issuer.as_str()],
        "scopes_supported": ["mcp:read", "mcp:write"],
        "bearer_methods_supported": ["header"],
    }))
    .into_response()
}

/// Streamable HTTP MCP endpoint. Accepts a single JSON-RPC message or a batch.
pub async fn mcp_http(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let token = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));
    let ctx = match token {
        Some(t) => authenticate(&s, t, None).await.ok(),
        None => None,
    };
    let Some(ctx) = ctx else {
        return challenge(&s);
    };

    let replies: Vec<Value> = match &body {
        Value::Array(msgs) => {
            let mut out = Vec::new();
            for m in msgs {
                if let Some(r) = handle_one(&s, &ctx, m).await {
                    out.push(r);
                }
            }
            out
        }
        other => handle_one(&s, &ctx, other).await.into_iter().collect(),
    };

    if replies.is_empty() {
        return StatusCode::ACCEPTED.into_response();
    }
    if body.is_array() {
        Json(Value::Array(replies)).into_response()
    } else {
        Json(replies.into_iter().next().unwrap()).into_response()
    }
}

fn challenge(s: &AppState) -> Response {
    let prm = format!("{}/.well-known/oauth-protected-resource", s.resource_url);
    (
        StatusCode::UNAUTHORIZED,
        [(
            axum::http::header::WWW_AUTHENTICATE,
            format!("Bearer resource_metadata=\"{prm}\", error=\"invalid_token\""),
        )],
        Json(json!({ "error": "unauthorized" })),
    )
        .into_response()
}

async fn handle_one(state: &AppState, ctx: &AuthContext, msg: &Value) -> Option<Value> {
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let id = msg.get("id").cloned()?; // notifications (no id) ⇒ no response

    match method {
        "initialize" => {
            let pv = msg
                .get("params")
                .and_then(|p| p.get("protocolVersion"))
                .and_then(Value::as_str)
                .unwrap_or(PROTOCOL_FALLBACK);
            Some(ok(
                id,
                json!({
                    "protocolVersion": pv,
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION") }
                }),
            ))
        }
        "ping" => Some(ok(id, json!({}))),
        "tools/list" => Some(ok(id, json!({ "tools": tool_definitions() }))),
        "tools/call" => {
            let params = msg.get("params").cloned().unwrap_or(Value::Null);
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            Some(ok(
                id,
                tool_result(call_tool(state, ctx, name, &args).await),
            ))
        }
        other => Some(err(id, -32601, &format!("method not found: {other}"))),
    }
}

fn ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}
fn err(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}
fn tool_result(outcome: Result<String, String>) -> Value {
    let (text, is_error) = match outcome {
        Ok(t) => (t, false),
        Err(e) => (e, true),
    };
    json!({ "content": [{ "type": "text", "text": text }], "isError": is_error })
}

fn arg_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing required string argument: {key}"))
}
fn arg_uuid(args: &Value, key: &str) -> Result<Uuid, String> {
    Uuid::parse_str(arg_str(args, key)?).map_err(|_| format!("{key} is not a valid UUID"))
}
fn e(err: mdm_core::Error) -> String {
    format!("error: {err}")
}
fn pretty<T: Serialize>(v: &T) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default()
}

async fn call_tool(
    state: &AppState,
    ctx: &AuthContext,
    name: &str,
    args: &Value,
) -> Result<String, String> {
    match name {
        "list_projects" => state
            .db
            .list_projects(ctx)
            .await
            .map(|v| pretty(&v))
            .map_err(e),
        "create_project" => state
            .db
            .create_project(ctx, arg_str(args, "slug")?, arg_str(args, "name")?)
            .await
            .map(|v| pretty(&v))
            .map_err(e),
        "list_documents" => state
            .db
            .list_documents(
                ctx,
                arg_uuid(args, "project_id")?,
                args.get("limit").and_then(Value::as_i64).unwrap_or(50),
            )
            .await
            .map(|v| pretty(&v))
            .map_err(e),
        "create_doc" => state
            .db
            .create_document(
                ctx,
                arg_uuid(args, "project_id")?,
                arg_str(args, "path")?,
                arg_str(args, "title")?,
                args.get("content").and_then(Value::as_str).unwrap_or(""),
            )
            .await
            .map(|d| format!("Created document {} (version {})", d.id, d.current_version))
            .map_err(e),
        "get_doc" => state
            .db
            .get_document(ctx, arg_uuid(args, "document_id")?)
            .await
            .map(|d| d.content)
            .map_err(e),
        "get_doc_by_path" => state
            .db
            .get_document_by_path(ctx, arg_uuid(args, "project_id")?, arg_str(args, "path")?)
            .await
            .map(|d| d.content)
            .map_err(e),
        "update_doc" => {
            let id = arg_uuid(args, "document_id")?;
            let expected = args
                .get("expected_version")
                .and_then(Value::as_i64)
                .ok_or("missing required integer argument: expected_version")?;
            let kind = match args.get("kind").and_then(Value::as_str) {
                Some("autosave") => VersionKind::Autosave,
                _ => VersionKind::Checkpoint,
            };
            match state
                .db
                .update_document(ctx, id, arg_str(args, "content")?, expected, kind)
                .await
                .map_err(e)?
            {
                UpdateOutcome::Updated(d) => {
                    Ok(format!("Updated to version {}", d.current_version))
                }
                UpdateOutcome::Conflict {
                    current_version,
                    current_content,
                    base_content,
                } => Err(format!(
                    "CONFLICT: document is now at version {current_version}; your expected_version was \
                     stale so nothing was written. Re-fetch and merge.\n\n--- CURRENT ---\n{current_content}\n\
                     --- BASE ---\n{base_content}"
                )),
            }
        }
        "append_to_doc" => state
            .db
            .append_to_document(
                ctx,
                arg_uuid(args, "document_id")?,
                arg_str(args, "content")?,
            )
            .await
            .map(|d| format!("Appended; now version {}", d.current_version))
            .map_err(e),
        "move_doc" => state
            .db
            .move_document(
                ctx,
                arg_uuid(args, "document_id")?,
                arg_str(args, "new_path")?,
            )
            .await
            .map(|d| format!("Moved to {}", d.path))
            .map_err(e),
        "delete_doc" => state
            .db
            .delete_document(ctx, arg_uuid(args, "document_id")?)
            .await
            .map(|_| "Deleted (soft).".to_string())
            .map_err(e),
        "restore_version" => {
            let id = arg_uuid(args, "document_id")?;
            let version = args
                .get("version")
                .and_then(Value::as_i64)
                .ok_or("missing required integer argument: version")?;
            state
                .db
                .restore_version(ctx, id, version)
                .await
                .map(|d| format!("Restored; now version {}", d.current_version))
                .map_err(e)
        }
        "get_doc_history" => state
            .db
            .get_history(ctx, arg_uuid(args, "document_id")?)
            .await
            .map(|v| pretty(&v))
            .map_err(e),
        "search_docs" => {
            let pid = args
                .get("project_id")
                .and_then(Value::as_str)
                .and_then(|s| Uuid::parse_str(s).ok());
            let mode = args
                .get("mode")
                .and_then(Value::as_str)
                .unwrap_or("keyword");
            crate::handlers::run_search(
                state,
                ctx,
                pid,
                arg_str(args, "query")?,
                mode,
                args.get("limit").and_then(Value::as_i64).unwrap_or(20),
            )
            .await
            .map(|hits| format_search(&hits))
            .map_err(|ae| e(ae.0))
        }
        "list_tags" => state.db.list_tags(ctx).await.map(|v| pretty(&v)).map_err(e),
        "add_tag" => state
            .db
            .add_document_tag(ctx, arg_uuid(args, "document_id")?, arg_str(args, "name")?)
            .await
            .map(|t| format!("Tagged with {}", t.name))
            .map_err(e),
        "list_categories" => state
            .db
            .list_categories(ctx)
            .await
            .map(|v| pretty(&v))
            .map_err(e),
        "create_category" => {
            let parent = match args.get("parent_id").and_then(Value::as_str) {
                Some(s) => Some(
                    Uuid::parse_str(s).map_err(|_| "parent_id is not a valid UUID".to_string())?,
                ),
                None => None,
            };
            state
                .db
                .create_category(ctx, parent, arg_str(args, "slug")?, arg_str(args, "name")?)
                .await
                .map(|v| pretty(&v))
                .map_err(e)
        }
        "categorize_doc" => state
            .db
            .categorize_document(
                ctx,
                arg_uuid(args, "document_id")?,
                arg_uuid(args, "category_id")?,
            )
            .await
            .map(|_| "Filed under category.".to_string())
            .map_err(e),
        "list_docs_by_tag" => state
            .db
            .list_documents_with_tag(
                ctx,
                arg_str(args, "tag")?,
                args.get("limit").and_then(Value::as_i64).unwrap_or(50),
            )
            .await
            .map(|v| pretty(&v))
            .map_err(e),
        "list_docs_by_category" => state
            .db
            .list_documents_in_category(ctx, arg_uuid(args, "category_id")?)
            .await
            .map(|v| pretty(&v))
            .map_err(e),
        other => Err(format!("unknown tool: {other}")),
    }
}

fn format_search(hits: &[SearchHit]) -> String {
    if hits.is_empty() {
        return "No matches.".to_string();
    }
    hits.iter()
        .map(|h| {
            format!(
                "{}  [{}]  (rank {:.3}) id={}\n    {}",
                h.path,
                h.title,
                h.rank,
                h.document_id,
                h.snippet.replace('\n', " ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
