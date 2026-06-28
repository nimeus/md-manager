//! `mdm-db` — SQLx-backed service layer, embedded migrations, and the **`TenantDb`** RLS
//! session wrapper.
//!
//! All tenant-scoped database access goes through a transaction whose `app.current_org_id`
//! GUC is set first (see [`Db::begin_scoped`]), so Postgres row-level security constrains
//! every query to one organization. The service methods (in the sibling modules) call into
//! `mdm-core` for validation, RBAC, hashing, chunking, and merge so the API, MCP, and CLI
//! surfaces all enforce identical rules.
//!
//! Queries are runtime-checked (no `query!` macros) so the build never needs a live DB.

use anyhow::Context;
use mdm_core::{AuthContext, model::ActorType};
use sqlx::{Postgres, Transaction, postgres::PgPoolOptions};
use uuid::Uuid;

mod access;
mod apikey;
mod auth;
mod category;
mod document;
mod org;
mod rows;
mod search;
mod tag;

pub use document::UpdateOutcome;

/// Embedded migrations (run as the owner/migrator role).
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

/// The application database handle: a connection pool plus runtime settings.
#[derive(Clone)]
pub struct Db {
    pool: sqlx::PgPool,
    pepper: String,
    max_doc_bytes: i64,
    autosave_debounce_secs: i64,
    max_docs_per_project: i64,
}

impl Db {
    /// Connect as the app runtime role (`md_app`).
    pub async fn connect(
        database_url: &str,
        max_connections: u32,
        pepper: String,
        max_doc_bytes: i64,
        autosave_debounce_secs: i64,
        max_docs_per_project: i64,
    ) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(database_url)
            .await
            .context("connecting to the database as the app role")?;
        Ok(Self {
            pool,
            pepper,
            max_doc_bytes,
            autosave_debounce_secs,
            max_docs_per_project,
        })
    }

    pub fn pool(&self) -> &sqlx::PgPool {
        &self.pool
    }

    /// Run pending migrations using a temporary pool on the owner/migrator URL.
    pub async fn run_migrations(owner_database_url: &str) -> anyhow::Result<()> {
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .connect(owner_database_url)
            .await
            .context("connecting as the owner role to run migrations")?;
        MIGRATOR.run(&pool).await.context("running migrations")?;
        pool.close().await;
        Ok(())
    }

    /// Safety check: the app role must NOT be able to bypass RLS. Call at startup.
    pub async fn assert_app_role_not_bypassrls(&self) -> anyhow::Result<()> {
        let bypass: bool =
            sqlx::query_scalar("SELECT rolbypassrls FROM pg_roles WHERE rolname = current_user")
                .fetch_one(&self.pool)
                .await
                .context("checking rolbypassrls")?;
        anyhow::ensure!(
            !bypass,
            "SECURITY: app role can bypass RLS — connect as a NOBYPASSRLS, non-owner role"
        );
        Ok(())
    }

    /// Begin a transaction scoped to `org_id` (sets the RLS + actor GUCs).
    async fn begin_scoped(
        &self,
        org_id: Uuid,
        actor_id: Uuid,
        actor_type: ActorType,
    ) -> sqlx::Result<Transaction<'_, Postgres>> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "SELECT set_config('app.current_org_id', $1, true),
                    set_config('app.current_actor_id', $2, true),
                    set_config('app.current_actor_type', $3, true)",
        )
        .bind(org_id.to_string())
        .bind(actor_id.to_string())
        .bind(actor_type.as_str())
        .execute(&mut *tx)
        .await?;
        Ok(tx)
    }

    /// Begin a transaction scoped to the caller's context.
    async fn begin_ctx(&self, ctx: &AuthContext) -> sqlx::Result<Transaction<'_, Postgres>> {
        self.begin_scoped(ctx.org_id, ctx.user_id, ctx.actor_type)
            .await
    }
}

/// Insert one audit-log row inside an existing tenant transaction.
async fn audit(
    tx: &mut Transaction<'_, Postgres>,
    ctx: &AuthContext,
    action: &str,
    target: Option<&str>,
    metadata: serde_json::Value,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO audit_log (id, org_id, actor_type, actor_id, action, target, metadata)
         VALUES ($1, $2, $3, $4, $5, $6, $7::jsonb)",
    )
    .bind(Uuid::now_v7())
    .bind(ctx.org_id)
    .bind(ctx.actor_type.as_str())
    .bind(ctx.user_id)
    .bind(action)
    .bind(target)
    .bind(metadata.to_string())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Map a SQLx error onto a domain error.
pub(crate) fn map_db(e: sqlx::Error) -> mdm_core::Error {
    use mdm_core::Error;
    match &e {
        sqlx::Error::RowNotFound => Error::NotFound,
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            Error::AlreadyExists(db.constraint().unwrap_or("resource").to_string())
        }
        sqlx::Error::Database(db) if db.is_foreign_key_violation() => {
            Error::invalid("referenced resource does not exist")
        }
        _ => Error::Internal(e.to_string()),
    }
}
