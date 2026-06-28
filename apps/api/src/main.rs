//! md-manager HTTP API server (Axum).

mod dto;
mod error;
mod google;
mod handlers;
mod mcp;
mod oauth;
mod oauth_server;
mod session;
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
        // Built-in OAuth 2.1 authorization server (native connector)
        .route(
            "/.well-known/oauth-authorization-server",
            get(oauth_server::authorization_server_metadata),
        )
        .route("/oauth/register", post(oauth_server::register))
        .route("/oauth/authorize", get(oauth_server::authorize))
        .route("/oauth/token", post(oauth_server::token))
        .route("/oauth/revoke", post(oauth_server::revoke))
        .route(
            "/v1/oauth/authorization-requests/{id}",
            get(oauth_server::get_authorization_request),
        )
        .route(
            "/v1/oauth/authorization-requests/{id}/approve",
            post(oauth_server::approve),
        )
        .route(
            "/v1/oauth/authorization-requests/{id}/deny",
            post(oauth_server::deny),
        )
        // Connected apps — manage connector grants (list / switch org / revoke)
        .route("/v1/oauth/grants", get(handlers::list_oauth_grants))
        .route(
            "/v1/oauth/grants/{client_id}/revoke",
            post(handlers::revoke_oauth_grant),
        )
        .route(
            "/v1/oauth/grants/{client_id}/switch",
            post(handlers::switch_oauth_grant),
        )
        .route("/v1/bootstrap", post(handlers::bootstrap))
        // Web sign-in: BFF exchanges a verified Google ID token for a session token.
        .route("/v1/auth/google", post(handlers::auth_google))
        .route("/v1/me", get(handlers::whoami))
        .route("/v1/me/orgs", get(handlers::list_my_orgs))
        .route(
            "/v1/orgs",
            get(handlers::list_orgs).post(handlers::create_org),
        )
        .route(
            "/v1/invitations",
            get(handlers::list_invitations).post(handlers::create_invitation),
        )
        .route("/v1/invitations/{id}", delete(handlers::revoke_invitation))
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
            "/v1/tags/{name}/documents",
            get(handlers::list_tag_documents),
        )
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
        .route("/v1/audit", get(handlers::list_audit))
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

    // Optionally auto-provision the md_owner / md_app roles from a superuser URL (managed
    // Postgres convenience — no manual SQL). Then run migrations as the owner role.
    if let Some(setup) = &cfg.setup_database_url {
        tracing::info!("provisioning database roles from MDM_SETUP_DATABASE_URL");
        Db::provision_roles(
            setup.expose(),
            cfg.migration_database_url.expose(),
            cfg.database_url.expose(),
        )
        .await?;
    }
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

    // OAuth: the built-in authorization server (native connector) OR an external JWT issuer
    // (Logto). Built-in mode takes precedence and is its own issuer.
    let builtin_oauth = cfg.oauth_builtin();
    let oauth = if builtin_oauth.is_some() {
        None
    } else {
        cfg.oauth()
            .map(|settings| Arc::new(oauth::OAuthValidator::new(&settings)))
    };
    let issuer: Option<Arc<String>> = if builtin_oauth.is_some() {
        Some(Arc::new(cfg.public_base_url()))
    } else {
        cfg.oauth().map(|s| Arc::new(s.issuer))
    };
    match (&builtin_oauth, &oauth) {
        (Some(_), _) => {
            tracing::info!("built-in OAuth authorization server enabled (native Claude/ChatGPT connector)")
        }
        (None, Some(_)) => tracing::info!("external OAuth (JWT) resource server enabled for /mcp"),
        _ => {}
    }
    // Per-IP limiter for anonymous Dynamic Client Registration.
    let dcr_per_hour = std::num::NonZeroU32::new(cfg.oauth_dcr_per_hour.max(1)).unwrap();
    let dcr_limiter: Arc<state::IpRateLimiter> = Arc::new(governor::RateLimiter::keyed(
        governor::Quota::per_hour(dcr_per_hour),
    ));

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
        spawn_embedding_worker(store, emb, &settings);
        tracing::info!(model = %settings.model, dims = settings.dimensions, "embedding worker started");
    }

    // Google web sign-in (optional, env-driven).
    let google = cfg
        .google_client_id
        .as_deref()
        .filter(|id| !id.trim().is_empty())
        .map(|id| Arc::new(google::GoogleValidator::new(id)));
    if google.is_some() {
        tracing::info!("Google web sign-in enabled (POST /v1/auth/google)");
    }

    let state = AppState {
        db,
        bootstrap_token: Arc::new(cfg.admin_bootstrap_token.expose().to_string()),
        oauth,
        resource_url: Arc::new(cfg.public_base_url()),
        mcp_resource: Arc::new(cfg.mcp_resource()),
        issuer,
        builtin_oauth: builtin_oauth.map(Arc::new),
        rate_limiter,
        dcr_limiter,
        embedder,
        google,
        session_secret: Arc::new(cfg.session_secret.expose().to_string()),
        session_ttl_secs: cfg.session_ttl_secs,
    };

    // Built-in AS housekeeping: periodically sweep expired authorization requests + codes
    // (Postgres has no TTL GC, and these are short-lived rows).
    if state.builtin_oauth.is_some() {
        let db = state.db.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(3600));
            loop {
                tick.tick().await;
                match db.cleanup_expired_oauth().await {
                    Ok(n) if n > 0 => {
                        tracing::debug!(deleted = n, "swept expired OAuth requests/codes")
                    }
                    Ok(_) => {}
                    Err(e) => tracing::warn!(error = %e, "OAuth cleanup failed"),
                }
            }
        });
    }

    let listener = tokio::net::TcpListener::bind(cfg.api_addr).await?;
    tracing::info!(addr = %cfg.api_addr, "md-manager API listening");
    axum::serve(listener, router(state)).await?;
    Ok(())
}

/// Background task: embed chunks lacking an embedding, in batches, off the write path.
///
/// The queue is self-healing: a failed batch is retried one chunk at a time so a single
/// poison chunk can't block its batch-mates; every failure records exponential backoff (so a
/// persistently-failing chunk stops being re-fetched immediately) and is dead-lettered after
/// `max_attempts` consecutive failures so it can never starve the queue.
fn spawn_embedding_worker(
    store: mdm_db::EmbeddingStore,
    embedder: Arc<mdm_embed::Embedder>,
    settings: &mdm_config::EmbeddingSettings,
) {
    let interval = std::time::Duration::from_secs(settings.worker_interval_secs);
    let batch = settings.batch_size;
    let max_attempts = settings.max_attempts;
    let backoff_base = settings.backoff_base_secs;
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
                        // Ambiguous (count mismatch) or failed batch call: isolate each chunk
                        // so one bad input can't penalise the rest. Recovers automatically
                        // when the provider does; dead-letters only the genuinely poison ones.
                        outcome => {
                            match &outcome {
                                Err(e) => {
                                    tracing::warn!(error = %e, "embedding batch failed; isolating chunks")
                                }
                                Ok(_) => {
                                    tracing::warn!("embedding count mismatch; isolating chunks")
                                }
                            }
                            embed_individually(
                                &store,
                                &embedder,
                                &chunks,
                                max_attempts,
                                backoff_base,
                            )
                            .await;
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

/// Retry a failed batch one chunk at a time: store the ones that embed, and record a
/// backed-off (eventually dead-lettered) failure for the ones that don't.
async fn embed_individually(
    store: &mdm_db::EmbeddingStore,
    embedder: &mdm_embed::Embedder,
    chunks: &[(uuid::Uuid, String)],
    max_attempts: i32,
    backoff_base: i64,
) {
    for (cid, text) in chunks {
        match embedder.embed(std::slice::from_ref(text)).await {
            Ok(vectors) if vectors.len() == 1 => {
                if let Err(e) = store.store(*cid, &vectors[0]).await {
                    tracing::error!(error = %e, "failed to store embedding");
                }
            }
            outcome => {
                let msg = match &outcome {
                    Err(e) => e.to_string(),
                    Ok(_) => "empty embedding response".to_string(),
                };
                match store
                    .mark_failed(&[*cid], &msg, max_attempts, backoff_base)
                    .await
                {
                    Ok(dead) if dead > 0 => {
                        tracing::error!(chunk = %cid, error = %msg, "embedding dead-lettered after repeated failures")
                    }
                    Ok(_) => {}
                    Err(e) => tracing::error!(error = %e, "failed to record embedding failure"),
                }
            }
        }
    }
}
