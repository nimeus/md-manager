//! Org-scoped hierarchical categories and document categorization.

use mdm_core::model::{AuthContext, Category};
use mdm_core::{Error, Result, ids, rbac, validate};
use serde_json::json;
use uuid::Uuid;

use crate::rows::CategoryRow;
use crate::{Db, audit, map_db};

const CAT_COLS: &str = "id, parent_id, slug, name, created_at";

impl Db {
    pub async fn create_category(
        &self,
        ctx: &AuthContext,
        parent_id: Option<Uuid>,
        slug: &str,
        name: &str,
    ) -> Result<Category> {
        rbac::require_write(ctx)?;
        validate::validate_slug(slug)?;
        validate::validate_title(name)?;

        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        if let Some(parent) = parent_id {
            // RLS scopes this to the caller's org, so a foreign parent reads as absent.
            let found: Option<Uuid> = sqlx::query_scalar("SELECT id FROM categories WHERE id = $1")
                .bind(parent)
                .fetch_optional(&mut *tx)
                .await
                .map_err(map_db)?;
            if found.is_none() {
                return Err(Error::invalid(
                    "parent category not found in this organization",
                ));
            }
        }

        let id = ids::new_id();
        let row = sqlx::query_as::<_, CategoryRow>(&format!(
            "INSERT INTO categories (id, org_id, parent_id, slug, name) VALUES ($1,$2,$3,$4,$5)
             RETURNING {CAT_COLS}"
        ))
        .bind(id)
        .bind(ctx.org_id)
        .bind(parent_id)
        .bind(slug)
        .bind(name)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db)?;
        audit(
            &mut tx,
            ctx,
            "category.create",
            Some(&id.to_string()),
            json!({ "slug": slug }),
        )
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(row.into())
    }

    pub async fn list_categories(&self, ctx: &AuthContext) -> Result<Vec<Category>> {
        rbac::require_read(ctx)?;
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let rows = sqlx::query_as::<_, CategoryRow>(&format!(
            "SELECT {CAT_COLS} FROM categories ORDER BY slug"
        ))
        .fetch_all(&mut *tx)
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// File a document under a category (both must be in the caller's org).
    pub async fn categorize_document(
        &self,
        ctx: &AuthContext,
        doc_id: Uuid,
        category_id: Uuid,
    ) -> Result<()> {
        rbac::require_write(ctx)?;
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;

        // RLS-scoped existence checks prevent linking to another org's doc/category.
        let doc_ok: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM documents WHERE id = $1 AND deleted_at IS NULL")
                .bind(doc_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(map_db)?;
        if doc_ok.is_none() {
            return Err(Error::NotFound);
        }
        let cat_ok: Option<Uuid> = sqlx::query_scalar("SELECT id FROM categories WHERE id = $1")
            .bind(category_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_db)?;
        if cat_ok.is_none() {
            return Err(Error::NotFound);
        }

        sqlx::query(
            "INSERT INTO document_categories (org_id, document_id, category_id)
             VALUES ($1,$2,$3) ON CONFLICT DO NOTHING",
        )
        .bind(ctx.org_id)
        .bind(doc_id)
        .bind(category_id)
        .execute(&mut *tx)
        .await
        .map_err(map_db)?;
        audit(
            &mut tx,
            ctx,
            "doc.categorize",
            Some(&doc_id.to_string()),
            json!({ "category_id": category_id }),
        )
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(())
    }

    pub async fn list_document_categories(
        &self,
        ctx: &AuthContext,
        doc_id: Uuid,
    ) -> Result<Vec<Category>> {
        rbac::require_read(ctx)?;
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let rows = sqlx::query_as::<_, CategoryRow>(
            "SELECT c.id, c.parent_id, c.slug, c.name, c.created_at FROM categories c
             JOIN document_categories dc ON dc.category_id = c.id
             WHERE dc.document_id = $1 ORDER BY c.slug",
        )
        .bind(doc_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// List documents filed under a category.
    pub async fn list_documents_in_category(
        &self,
        ctx: &AuthContext,
        category_id: Uuid,
    ) -> Result<Vec<mdm_core::model::DocumentSummary>> {
        rbac::require_read(ctx)?;
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let rows = sqlx::query_as::<_, crate::rows::DocSummaryRow>(
            "SELECT d.id, d.project_id, d.path, d.title, d.current_version, d.updated_at
             FROM documents d JOIN document_categories dc ON dc.document_id = d.id
             WHERE dc.category_id = $1 AND d.deleted_at IS NULL ORDER BY d.path",
        )
        .bind(category_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
}
