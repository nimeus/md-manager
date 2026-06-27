//! md-manager MCP server.
//!
//! Speaks the Model Context Protocol over stdio (newline-delimited JSON-RPC 2.0), exposing
//! the document tools to AI agent hosts (Claude Code/Desktop, Gemini CLI, Codex, …).
//! It authenticates to the md-manager HTTP API with an API key from the environment and
//! never touches the database directly, so the same rules (RLS, RBAC, versioning) apply.
//!
//! Config (env): `MDM_API_URL` (default http://127.0.0.1:8787), `MDM_API_KEY` (required).
//! Diagnostics go to stderr; only JSON-RPC messages go to stdout.

use mdm_client::{Client, ClientError, UpdateResult};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

const PROTOCOL_FALLBACK: &str = "2024-11-05";

#[tokio::main]
async fn main() {
    let api_url = std::env::var("MDM_API_URL").unwrap_or_else(|_| "http://127.0.0.1:8787".into());
    let api_key = match std::env::var("MDM_API_KEY") {
        Ok(k) if !k.trim().is_empty() => k,
        _ => {
            eprintln!("mdm-mcp: MDM_API_KEY is required (an `mk_…` API key)");
            std::process::exit(1);
        }
    };
    let client = Client::new(api_url, api_key);
    eprintln!("mdm-mcp: ready on stdio");

    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("mdm-mcp: bad JSON: {e}");
                continue;
            }
        };
        if let Some(reply) = handle(&client, &msg).await {
            let mut s = reply.to_string();
            s.push('\n');
            if stdout.write_all(s.as_bytes()).await.is_err() {
                break;
            }
            let _ = stdout.flush().await;
        }
    }
}

/// Handle one JSON-RPC message. Returns `Some(response)` for requests, `None` for
/// notifications (which carry no `id`).
async fn handle(client: &Client, msg: &Value) -> Option<Value> {
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let id = msg.get("id").cloned();

    // Notifications have no id and expect no response.
    if id.is_none() {
        return None;
    }
    let id = id.unwrap();

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
                    "serverInfo": { "name": "md-manager", "version": env!("CARGO_PKG_VERSION") }
                }),
            ))
        }
        "ping" => Some(ok(id, json!({}))),
        "tools/list" => Some(ok(id, json!({ "tools": tool_defs() }))),
        "tools/call" => {
            let params = msg.get("params").cloned().unwrap_or(Value::Null);
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            let result = call_tool(client, name, &args).await;
            Some(ok(id, tool_result(result)))
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

/// Wrap a tool outcome in the MCP `tools/call` result shape.
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

fn fmt_err(e: ClientError) -> String {
    format!("error: {e}")
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}

async fn call_tool(client: &Client, name: &str, args: &Value) -> Result<String, String> {
    match name {
        "list_projects" => client.list_projects().await.map(|v| pretty(&v)).map_err(fmt_err),
        "create_project" => client
            .create_project(arg_str(args, "slug")?, arg_str(args, "name")?)
            .await
            .map(|v| pretty(&v))
            .map_err(fmt_err),
        "list_documents" => client
            .list_documents(arg_str(args, "project_id")?, args.get("limit").and_then(Value::as_i64))
            .await
            .map(|v| pretty(&v))
            .map_err(fmt_err),
        "create_doc" => client
            .create_document(
                arg_str(args, "project_id")?,
                arg_str(args, "path")?,
                arg_str(args, "title")?,
                args.get("content").and_then(Value::as_str).unwrap_or(""),
            )
            .await
            .map(|v| format!("Created document {} (version {})", v["id"], v["current_version"]))
            .map_err(fmt_err),
        // Reading returns the RAW markdown body — the key agent affordance.
        "get_doc" => client
            .get_document(arg_str(args, "document_id")?)
            .await
            .map(|v| v["content"].as_str().unwrap_or_default().to_string())
            .map_err(fmt_err),
        "get_doc_by_path" => client
            .get_document_by_path(arg_str(args, "project_id")?, arg_str(args, "path")?)
            .await
            .map(|v| v["content"].as_str().unwrap_or_default().to_string())
            .map_err(fmt_err),
        "update_doc" => {
            let kind = args.get("kind").and_then(Value::as_str).unwrap_or("checkpoint");
            let expected = args
                .get("expected_version")
                .and_then(Value::as_i64)
                .ok_or("missing required integer argument: expected_version")?;
            match client
                .update_document(arg_str(args, "document_id")?, arg_str(args, "content")?, expected, kind)
                .await
                .map_err(fmt_err)?
            {
                UpdateResult::Updated(v) => Ok(format!("Updated to version {}", v["current_version"])),
                UpdateResult::Conflict { current_version, current_content, base_content } => {
                    Err(format!(
                        "CONFLICT: the document is now at version {current_version}; your \
                         expected_version was stale, so nothing was written. Re-fetch and merge.\n\n\
                         --- CURRENT (version {current_version}) ---\n{current_content}\n\
                         --- BASE (your expected_version) ---\n{base_content}"
                    ))
                }
            }
        }
        "append_to_doc" => client
            .append_document(arg_str(args, "document_id")?, arg_str(args, "content")?)
            .await
            .map(|v| format!("Appended; now version {}", v["current_version"]))
            .map_err(fmt_err),
        "move_doc" => client
            .move_document(arg_str(args, "document_id")?, arg_str(args, "new_path")?)
            .await
            .map(|v| format!("Moved to {}", v["path"]))
            .map_err(fmt_err),
        "delete_doc" => client
            .delete_document(arg_str(args, "document_id")?)
            .await
            .map(|_| "Deleted (soft).".to_string())
            .map_err(fmt_err),
        "restore_version" => {
            let version = args
                .get("version")
                .and_then(Value::as_i64)
                .ok_or("missing required integer argument: version")?;
            client
                .restore_version(arg_str(args, "document_id")?, version)
                .await
                .map(|v| format!("Restored; now version {}", v["current_version"]))
                .map_err(fmt_err)
        }
        "get_doc_history" => client
            .history(arg_str(args, "document_id")?)
            .await
            .map(|v| pretty(&v))
            .map_err(fmt_err),
        "search_docs" => client
            .search(
                arg_str(args, "query")?,
                args.get("project_id").and_then(Value::as_str),
                args.get("limit").and_then(Value::as_i64),
            )
            .await
            .map(|v| format_search(&v))
            .map_err(fmt_err),
        "list_tags" => client.list_tags().await.map(|v| pretty(&v)).map_err(fmt_err),
        "add_tag" => client
            .add_document_tag(arg_str(args, "document_id")?, arg_str(args, "name")?)
            .await
            .map(|v| format!("Tagged with {}", v["name"]))
            .map_err(fmt_err),
        other => Err(format!("unknown tool: {other}")),
    }
}

fn format_search(v: &Value) -> String {
    let Some(arr) = v.as_array() else {
        return pretty(v);
    };
    if arr.is_empty() {
        return "No matches.".to_string();
    }
    arr.iter()
        .map(|h| {
            format!(
                "{}  [{}]  (rank {:.3}) id={}\n    {}",
                h["path"].as_str().unwrap_or(""),
                h["title"].as_str().unwrap_or(""),
                h["rank"].as_f64().unwrap_or(0.0),
                h["document_id"].as_str().unwrap_or(""),
                h["snippet"].as_str().unwrap_or("").replace('\n', " "),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The advertised tool surface. Descriptions and JSON Schemas guide the agent.
fn tool_defs() -> Value {
    let doc_id = json!({ "document_id": { "type": "string", "description": "Document UUID" } });
    json!([
        tool("list_projects", "List projects in your organization.", json!({}), &[]),
        tool("create_project", "Create a project (document container).",
             json!({ "slug": { "type": "string" }, "name": { "type": "string" } }), &["slug", "name"]),
        tool("list_documents", "List documents in a project.",
             json!({ "project_id": { "type": "string" }, "limit": { "type": "integer" } }), &["project_id"]),
        tool("create_doc", "Create a markdown document in a project.",
             json!({ "project_id": { "type": "string" }, "path": { "type": "string", "description": "e.g. guides/setup" },
                     "title": { "type": "string" }, "content": { "type": "string" } }),
             &["project_id", "path", "title"]),
        tool("get_doc", "Get a document's raw markdown by id.", json!(doc_id), &["document_id"]),
        tool("get_doc_by_path", "Get a document's raw markdown by project + path.",
             json!({ "project_id": { "type": "string" }, "path": { "type": "string" } }), &["project_id", "path"]),
        tool("update_doc",
             "Replace a document's content. Requires expected_version for optimistic concurrency; \
              on a stale version nothing is written and the current+base content is returned to merge. \
              kind: 'checkpoint' (default) or 'autosave'.",
             json!({ "document_id": { "type": "string" }, "content": { "type": "string" },
                     "expected_version": { "type": "integer" }, "kind": { "type": "string", "enum": ["checkpoint", "autosave"] } }),
             &["document_id", "content", "expected_version"]),
        tool("append_to_doc", "Append text to a document (atomic; creates a new version).",
             json!({ "document_id": { "type": "string" }, "content": { "type": "string" } }), &["document_id", "content"]),
        tool("move_doc", "Change a document's path.",
             json!({ "document_id": { "type": "string" }, "new_path": { "type": "string" } }), &["document_id", "new_path"]),
        tool("delete_doc", "Soft-delete a document.", json!(doc_id), &["document_id"]),
        tool("restore_version", "Restore a document to a prior version (new checkpoint).",
             json!({ "document_id": { "type": "string" }, "version": { "type": "integer" } }), &["document_id", "version"]),
        tool("get_doc_history", "List a document's version history.", json!(doc_id), &["document_id"]),
        tool("search_docs", "Keyword full-text search across documents (optionally one project).",
             json!({ "query": { "type": "string" }, "project_id": { "type": "string" }, "limit": { "type": "integer" } }),
             &["query"]),
        tool("list_tags", "List tags in your organization.", json!({}), &[]),
        tool("add_tag", "Attach a tag to a document (creating the tag if needed).",
             json!({ "document_id": { "type": "string" }, "name": { "type": "string" } }), &["document_id", "name"]),
    ])
}

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": required,
        }
    })
}
