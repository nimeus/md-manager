//! Organization and project operations.

use mdm_core::model::{AuthContext, Organization, Project};
use mdm_core::{Error, Result, ids, rbac, validate};
use serde_json::json;
use uuid::Uuid;

use crate::rows::{OrgRow, ProjectRow};
use crate::{Db, audit, map_db};

impl Db {
    /// The caller's current organization.
    pub async fn get_current_org(&self, ctx: &AuthContext) -> Result<Organization> {
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let row = sqlx::query_as::<_, OrgRow>(
            "SELECT id, slug, name, created_at FROM organizations
             WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(ctx.org_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        row.map(Into::into).ok_or(Error::NotFound)
    }

    /// MVP: an agent's key is bound to one org, so this returns just the current org.
    pub async fn list_orgs(&self, ctx: &AuthContext) -> Result<Vec<Organization>> {
        Ok(vec![self.get_current_org(ctx).await?])
    }

    pub async fn create_project(
        &self,
        ctx: &AuthContext,
        slug: &str,
        name: &str,
    ) -> Result<Project> {
        rbac::require_admin(ctx)?;
        validate::validate_slug(slug)?;
        let id = ids::new_id();
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let row = sqlx::query_as::<_, ProjectRow>(
            "INSERT INTO projects (id, org_id, slug, name) VALUES ($1, $2, $3, $4)
             RETURNING id, org_id, slug, name, created_at",
        )
        .bind(id)
        .bind(ctx.org_id)
        .bind(slug)
        .bind(name)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db)?;
        audit(
            &mut tx,
            ctx,
            "project.create",
            Some(&id.to_string()),
            json!({ "slug": slug }),
        )
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(row.into())
    }

    pub async fn list_projects(&self, ctx: &AuthContext) -> Result<Vec<Project>> {
        rbac::require_read(ctx)?;
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let rows = sqlx::query_as::<_, ProjectRow>(
            "SELECT id, org_id, slug, name, created_at FROM projects
             WHERE deleted_at IS NULL ORDER BY slug",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn get_project_by_slug(&self, ctx: &AuthContext, slug: &str) -> Result<Project> {
        rbac::require_read(ctx)?;
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let row = sqlx::query_as::<_, ProjectRow>(
            "SELECT id, org_id, slug, name, created_at FROM projects
             WHERE slug = $1 AND deleted_at IS NULL",
        )
        .bind(slug)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        row.map(Into::into).ok_or(Error::NotFound)
    }

    /// Resolve a project id, verifying it belongs to the caller's org (via RLS).
    pub(crate) async fn project_exists(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        project_id: Uuid,
    ) -> Result<bool> {
        let found: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM projects WHERE id = $1 AND deleted_at IS NULL")
                .bind(project_id)
                .fetch_optional(&mut **tx)
                .await
                .map_err(map_db)?;
        Ok(found.is_some())
    }
}
