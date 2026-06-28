//! Shared MCP tool **schema** definitions — the contract advertised by both the stdio MCP
//! server (`apps/mcp`, dispatches via HTTP client) and the API-hosted Streamable HTTP MCP
//! endpoint (`apps/api`, dispatches to the db service directly). Keeping the schemas here
//! prevents the two surfaces from drifting; only the dispatch differs.

use serde_json::{Value, json};

pub const PROTOCOL_FALLBACK: &str = "2024-11-05";
pub const SERVER_NAME: &str = "md-manager";

/// The 15 advertised tools, as JSON-Schema tool definitions for `tools/list`.
pub fn tool_definitions() -> Value {
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
        tool("list_categories", "List categories (org-scoped, hierarchical, cross-project).", json!({}), &[]),
        tool("create_category", "Create a category, optionally under a parent.",
             json!({ "slug": { "type": "string" }, "name": { "type": "string" }, "parent_id": { "type": "string" } }),
             &["slug", "name"]),
        tool("categorize_doc", "File a document under a category.",
             json!({ "document_id": { "type": "string" }, "category_id": { "type": "string" } }),
             &["document_id", "category_id"]),
    ])
}

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": { "type": "object", "properties": properties, "required": required }
    })
}
