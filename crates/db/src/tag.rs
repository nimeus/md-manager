//! Org-scoped tags and document tagging.

use mdm_core::model::{AuthContext, DocumentSummary, OrgRole, Tag};
use mdm_core::{Result, ids, rbac};
use serde_json::json;
use uuid::Uuid;

use crate::{Db, audit, map_db};

impl Db {
    pub async fn list_tags(&self, ctx: &AuthContext) -> Result<Vec<Tag>> {
        rbac::require_read(ctx)?;
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let rows =
            sqlx::query_as::<_, crate::rows::TagRow>("SELECT id, name FROM tags ORDER BY name")
                .fetch_all(&mut *tx)
                .await
                .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Attach a tag (creating it if needed) to a document.
    pub async fn add_document_tag(
        &self,
        ctx: &AuthContext,
        doc_id: Uuid,
        name: &str,
    ) -> Result<Tag> {
        rbac::require_write(ctx)?;
        let name = name.trim();
        if name.is_empty() {
            return Err(mdm_core::Error::invalid("tag name is required"));
        }
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let tag = sqlx::query_as::<_, crate::rows::TagRow>(
            "INSERT INTO tags (id, org_id, name) VALUES ($1, $2, $3)
             ON CONFLICT (org_id, name) DO UPDATE SET name = EXCLUDED.name
             RETURNING id, name",
        )
        .bind(ids::new_id())
        .bind(ctx.org_id)
        .bind(name)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db)?;

        sqlx::query(
            "INSERT INTO document_tags (org_id, document_id, tag_id) VALUES ($1, $2, $3)
             ON CONFLICT DO NOTHING",
        )
        .bind(ctx.org_id)
        .bind(doc_id)
        .bind(tag.id)
        .execute(&mut *tx)
        .await
        .map_err(map_db)?;

        audit(
            &mut tx,
            ctx,
            "doc.tag",
            Some(&doc_id.to_string()),
            json!({"tag": name}),
        )
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(tag.into())
    }

    pub async fn list_document_tags(&self, ctx: &AuthContext, doc_id: Uuid) -> Result<Vec<Tag>> {
        rbac::require_read(ctx)?;
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let rows = sqlx::query_as::<_, crate::rows::TagRow>(
            "SELECT t.id, t.name FROM tags t
             JOIN document_tags dt ON dt.tag_id = t.id
             WHERE dt.document_id = $1 ORDER BY t.name",
        )
        .bind(doc_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// List documents carrying a given tag (by name), deny-filtered like `list_documents`.
    pub async fn list_documents_with_tag(
        &self,
        ctx: &AuthContext,
        tag_name: &str,
        limit: i64,
    ) -> Result<Vec<DocumentSummary>> {
        rbac::require_read(ctx)?;
        let privileged = matches!(ctx.org_role, OrgRole::Owner | OrgRole::Admin);
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let rows = sqlx::query_as::<_, crate::rows::DocSummaryRow>(
            "SELECT d.id, d.project_id, d.path, d.title, d.current_version, d.updated_at
             FROM documents d
             JOIN document_tags dt ON dt.document_id = d.id
             JOIN tags t ON t.id = dt.tag_id
             WHERE t.name = $1 AND d.deleted_at IS NULL
               AND ($3 OR NOT EXISTS (
                 SELECT 1 FROM document_grants g
                 WHERE g.document_id = d.id AND g.role = 'none'
                   AND ((g.subject_type = 'user' AND g.subject_id = $4)
                     OR (g.subject_type = 'team' AND g.subject_id IN
                         (SELECT team_id FROM team_members WHERE user_id = $4)))))
             ORDER BY d.path LIMIT $2",
        )
        .bind(tag_name)
        .bind(limit)
        .bind(privileged)
        .bind(ctx.user_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
}
