//! md-manager HTTP API server (Axum).

mod dto;
mod error;
mod handlers;
mod mcp;
mod oauth;
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
        // Remote MCP (Streamable HTTP) + OAuth discovery
        .route("/mcp", post(mcp::mcp_http))
        .route(
            "/.well-known/oauth-protected-resource",
            get(mcp::protected_resource_metadata),
        )
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
        .route(
            "/v1/documents/{id}/undelete",
            post(handlers::undelete_document),
        )
        .route("/v1/documents/{id}/history", get(handlers::history))
        .route(
            "/v1/documents/{id}/versions/{version}",
            get(handlers::get_version),
        )
        .route(
            "/v1/documents/{id}/restore",
            post(handlers::restore_version),
        )
        .route(
            "/v1/documents/{id}/tags",
            get(handlers::list_document_tags).post(handlers::add_document_tag),
        )
        .route(
            "/v1/documents/{id}/categories",
            get(handlers::list_document_categories).post(handlers::categorize_document),
        )
        .route("/v1/tags", get(handlers::list_tags))
        .route(
            "/v1/categories",
            get(handlers::list_categories).post(handlers::create_category),
        )
        .route(
            "/v1/categories/{id}/documents",
            get(handlers::list_category_documents),
        )
        .route(
            "/v1/teams",
            get(handlers::list_teams).post(handlers::create_team),
        )
        .route("/v1/teams/{id}/members", post(handlers::add_team_member))
        .route("/v1/projects/{id}/grants", post(handlers::grant_project))
        .route("/v1/documents/{id}/grants", post(handlers::grant_document))
        .route("/v1/search", get(handlers::search))
        .route(
            "/v1/documents/{id}/shares",
            get(handlers::list_shares).post(handlers::create_share),
        )
        .route("/v1/shares/{id}", delete(handlers::revoke_share))
        // Public, unauthenticated read-only document view.
        .route("/v1/shared/{token}", get(handlers::get_shared))
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
        cfg.max_docs_per_project,
    )
    .await?;
    db.assert_app_role_not_bypassrls().await?;

    let per_minute = std::num::NonZeroU32::new(cfg.rate_limit_per_minute.max(1)).unwrap();
    let rate_limiter: Arc<state::RateLimiter> = Arc::new(governor::RateLimiter::keyed(
        governor::Quota::per_minute(per_minute),
    ));

    let oauth = cfg
        .oauth()
        .map(|settings| Arc::new(oauth::OAuthValidator::new(&settings)));
    if oauth.is_some() {
        tracing::info!("OAuth resource server enabled for the /mcp endpoint");
    }

    // Embeddings (semantic search) — optional, fully env-driven.
    let embedder = cfg.embedding().map(|s| {
        Arc::new(mdm_embed::Embedder::new(
            s.base_url,
            s.api_key,
            s.model,
            s.dimensions as usize,
            std::time::Duration::from_secs(s.timeout_secs),
            s.referer,
            s.title,
        ))
    });
    if let (Some(settings), Some(emb)) = (cfg.embedding(), embedder.clone()) {
        let store = mdm_db::EmbeddingStore::connect(
            cfg.migration_database_url.expose(),
            settings.dimensions,
        )
        .await?;
        spawn_embedding_worker(
            store,
            emb,
            settings.batch_size,
            settings.worker_interval_secs,
        );
        tracing::info!(model = %settings.model, dims = settings.dimensions, "embedding worker started");
    }

    let state = AppState {
        db,
        bootstrap_token: Arc::new(cfg.admin_bootstrap_token.expose().to_string()),
        oauth,
        resource_url: Arc::new(cfg.public_base_url()),
        issuer: cfg.oauth().map(|s| Arc::new(s.issuer)),
        rate_limiter,
        embedder,
    };

    let listener = tokio::net::TcpListener::bind(cfg.api_addr).await?;
    tracing::info!(addr = %cfg.api_addr, "md-manager API listening");
    axum::serve(listener, router(state)).await?;
    Ok(())
}

/// Background task: embed chunks lacking an embedding, in batches, off the write path.
fn spawn_embedding_worker(
    store: mdm_db::EmbeddingStore,
    embedder: Arc<mdm_embed::Embedder>,
    batch: i64,
    interval_secs: u64,
) {
    let interval = std::time::Duration::from_secs(interval_secs);
    tokio::spawn(async move {
        loop {
            // Reuse embeddings for identical content before calling the provider.
            if let Ok(n) = store.dedup_by_content_hash().await
                && n > 0
            {
                tracing::debug!(copied = n, "reused embeddings for duplicate chunks");
            }
            match store.pending(batch).await {
                Ok(chunks) if !chunks.is_empty() => {
                    let texts: Vec<String> = chunks.iter().map(|(_, t)| t.clone()).collect();
                    match embedder.embed(&texts).await {
                        Ok(vectors) if vectors.len() == chunks.len() => {
                            for ((cid, _), vec) in chunks.iter().zip(vectors) {
                                if let Err(e) = store.store(*cid, &vec).await {
                                    tracing::error!(error = %e, "failed to store embedding");
                                }
                            }
                        }
                        Ok(_) => tracing::warn!("embedding count mismatch; skipping batch"),
                        Err(e) => {
                            tracing::warn!(error = %e, "embedding API call failed; backing off");
                            tokio::time::sleep(interval).await;
                        }
                    }
                }
                Ok(_) => tokio::time::sleep(interval).await,
                Err(e) => {
                    tracing::error!(error = %e, "embedding worker query failed");
                    tokio::time::sleep(interval).await;
                }
            }
        }
    });
}
