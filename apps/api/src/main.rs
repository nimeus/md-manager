//! md-manager HTTP API server (Axum).

mod dto;
mod error;
mod handlers;
mod state;

use std::sync::Arc;

use axum::{
    Router,
    routing::{delete, get, post},
};
use mdm_config::Config;
use mdm_db::Db;
use tower_http::trace::TraceLayer;

use crate::state::AppState;

fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(handlers::healthz))
        .route("/v1/bootstrap", post(handlers::bootstrap))
        .route("/v1/me", get(handlers::whoami))
        .route("/v1/orgs", get(handlers::list_orgs))
        .route(
            "/v1/projects",
            get(handlers::list_projects).post(handlers::create_project),
        )
        .route("/v1/projects/{slug}", get(handlers::get_project))
        .route(
            "/v1/projects/{project_id}/documents",
            get(handlers::list_documents).post(handlers::create_document),
        )
        .route("/v1/documents/by-path", get(handlers::get_document_by_path))
        .route(
            "/v1/documents/{id}",
            get(handlers::get_document)
                .put(handlers::update_document)
                .delete(handlers::delete_document),
        )
        .route("/v1/documents/{id}/append", post(handlers::append_document))
        .route("/v1/documents/{id}/move", post(handlers::move_document))
        .route("/v1/documents/{id}/undelete", post(handlers::undelete_document))
        .route("/v1/documents/{id}/history", get(handlers::history))
        .route("/v1/documents/{id}/versions/{version}", get(handlers::get_version))
        .route("/v1/documents/{id}/restore", post(handlers::restore_version))
        .route(
            "/v1/documents/{id}/tags",
            get(handlers::list_document_tags).post(handlers::add_document_tag),
        )
        .route("/v1/tags", get(handlers::list_tags))
        .route("/v1/search", get(handlers::search))
        .route(
            "/v1/api-keys",
            get(handlers::list_api_keys).post(handlers::create_api_key),
        )
        .route("/v1/api-keys/{id}", delete(handlers::revoke_api_key))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Config::load()?;
    mdm_config::tracing_init::init(cfg.log_format);

    // Run migrations as the owner role, then connect as the app role.
    Db::run_migrations(cfg.migration_database_url.expose()).await?;
    let db = Db::connect(
        cfg.database_url.expose(),
        cfg.db_max_connections,
        cfg.api_key_pepper.expose().to_string(),
        cfg.max_doc_bytes,
        cfg.autosave_debounce_secs,
    )
    .await?;
    db.assert_app_role_not_bypassrls().await?;

    let state = AppState {
        db,
        bootstrap_token: Arc::new(cfg.admin_bootstrap_token.expose().to_string()),
    };

    let listener = tokio::net::TcpListener::bind(cfg.api_addr).await?;
    tracing::info!(addr = %cfg.api_addr, "md-manager API listening");
    axum::serve(listener, router(state)).await?;
    Ok(())
}
