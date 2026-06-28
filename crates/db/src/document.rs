//! Document operations: CRUD, full-snapshot versioning, optimistic concurrency,
//! atomic append, restore, move, soft delete/undelete, and history.

use mdm_core::model::{
    AuthContext, Document, DocumentSummary, DocumentVersion, OrgRole, Role, VersionKind,
    VersionSummary,
};
use mdm_core::{Error, Result, crypto, ids, rbac, validate};
use serde_json::json;
use sqlx::{Postgres, Transaction};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

const DOC_COLS: &str = "id, org_id, project_id, path, title, content, content_hash, \
     current_version, created_by, updated_by, created_at, updated_at";

/// Outcome of an `update_document` call.
pub enum UpdateOutcome {
    Updated(Document),
    /// The caller's `expected_version` was stale. Carries what's needed for a 3-way merge.
    Conflict {
        current_version: i64,
        current_content: String,
        base_content: String,
    },
}

use crate::{Db, audit, map_db};

impl Db {
    pub async fn create_document(
        &self,
        ctx: &AuthContext,
        project_id: Uuid,
        path: &str,
        title: &str,
        content: &str,
    ) -> Result<Document> {
        validate::validate_path(path)?;
        validate::validate_title(title)?;
        validate::validate_content_size(content, self.max_doc_bytes)?;

        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        if !self.project_exists(&mut tx, project_id).await? {
            return Err(Error::invalid("project not found in this organization"));
        }
        self.authorize_project(&mut tx, ctx, project_id, Role::Editor)
            .await?;

        // Quota guard against agent create-loops.
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM documents WHERE project_id = $1 AND deleted_at IS NULL",
        )
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db)?;
        if count >= self.max_docs_per_project {
            return Err(Error::TooManyRequests(format!(
                "project document limit reached ({})",
                self.max_docs_per_project
            )));
        }

        let doc_id = ids::new_id();
        let hash = crypto::content_hash(content);
        let doc = sqlx::query_as::<_, crate::rows::DocumentRow>(&format!(
            "INSERT INTO documents
               (id, org_id, project_id, path, title, content, content_hash,
                current_version, created_by, updated_by)
             VALUES ($1,$2,$3,$4,$5,$6,$7,1,$8,$8)
             RETURNING {DOC_COLS}"
        ))
        .bind(doc_id)
        .bind(ctx.org_id)
        .bind(project_id)
        .bind(path)
        .bind(title)
        .bind(content)
        .bind(&hash)
        .bind(ctx.user_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db)?;

        self.insert_version(
            &mut tx,
            ctx,
            doc_id,
            1,
            content,
            &hash,
            VersionKind::Checkpoint,
        )
        .await?;
        self.reindex_chunks(&mut tx, ctx, doc_id, content).await?;
        audit(
            &mut tx,
            ctx,
            "doc.create",
            Some(&doc_id.to_string()),
            json!({"path": path}),
        )
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(doc.into())
    }

    pub async fn get_document(&self, ctx: &AuthContext, doc_id: Uuid) -> Result<Document> {
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let row = sqlx::query_as::<_, crate::rows::DocumentRow>(&format!(
            "SELECT {DOC_COLS} FROM documents WHERE id = $1 AND deleted_at IS NULL"
        ))
        .bind(doc_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db)?;
        let Some(row) = row else {
            return Err(Error::NotFound);
        };
        self.authorize_doc(&mut tx, ctx, doc_id, row.project_id, Role::Viewer)
            .await?;
        tx.commit().await.map_err(map_db)?;
        Ok(row.into())
    }

    pub async fn get_document_by_path(
        &self,
        ctx: &AuthContext,
        project_id: Uuid,
        path: &str,
    ) -> Result<Document> {
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let row = sqlx::query_as::<_, crate::rows::DocumentRow>(&format!(
            "SELECT {DOC_COLS} FROM documents
             WHERE project_id = $1 AND path = $2 AND deleted_at IS NULL"
        ))
        .bind(project_id)
        .bind(path)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db)?;
        let Some(row) = row else {
            return Err(Error::NotFound);
        };
        self.authorize_doc(&mut tx, ctx, row.id, row.project_id, Role::Viewer)
            .await?;
        tx.commit().await.map_err(map_db)?;
        Ok(row.into())
    }

    pub async fn list_documents(
        &self,
        ctx: &AuthContext,
        project_id: Uuid,
        limit: i64,
    ) -> Result<Vec<DocumentSummary>> {
        rbac::require_read(ctx)?;
        // Owners/admins see everything; others don't see docs they're explicitly denied.
        let privileged = matches!(ctx.org_role, OrgRole::Owner | OrgRole::Admin);
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let rows = sqlx::query_as::<_, crate::rows::DocSummaryRow>(
            "SELECT id, project_id, path, title, current_version, updated_at
             FROM documents d
             WHERE d.project_id = $1 AND d.deleted_at IS NULL
               AND ($4 OR NOT EXISTS (
                 SELECT 1 FROM document_grants g
                 WHERE g.document_id = d.id AND g.role = 'none'
                   AND ((g.subject_type = 'user' AND g.subject_id = $3)
                     OR (g.subject_type = 'team' AND g.subject_id IN
                         (SELECT team_id FROM team_members WHERE user_id = $3)))))
             ORDER BY d.path LIMIT $2",
        )
        .bind(project_id)
        .bind(limit)
        .bind(ctx.user_id)
        .bind(privileged)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Update a document with optimistic concurrency. A stale `expected_version` returns
    /// [`UpdateOutcome::Conflict`] (no write). Consecutive same-actor autosaves within the
    /// debounce window are coalesced into the latest version instead of creating a new one.
    pub async fn update_document(
        &self,
        ctx: &AuthContext,
        doc_id: Uuid,
        content: &str,
        expected_version: i64,
        kind: VersionKind,
    ) -> Result<UpdateOutcome> {
        validate::validate_content_size(content, self.max_doc_bytes)?;

        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let cur = self.lock_document(&mut tx, doc_id).await?;
        self.authorize_doc(&mut tx, ctx, doc_id, cur.project_id, Role::Editor)
            .await?;

        if cur.current_version != expected_version {
            let base: Option<String> = sqlx::query_scalar(
                "SELECT content FROM document_versions WHERE document_id = $1 AND version = $2",
            )
            .bind(doc_id)
            .bind(expected_version)
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_db)?;
            tx.commit().await.map_err(map_db)?;
            return Ok(UpdateOutcome::Conflict {
                current_version: cur.current_version,
                current_content: cur.content,
                base_content: base.unwrap_or_default(),
            });
        }

        let hash = crypto::content_hash(content);
        let coalesced = self
            .try_coalesce(&mut tx, ctx, doc_id, content, &hash, kind)
            .await?;

        let doc = if let Some(doc) = coalesced {
            doc
        } else {
            let new_version = cur.current_version + 1;
            let doc = sqlx::query_as::<_, crate::rows::DocumentRow>(&format!(
                "UPDATE documents SET content = $1, content_hash = $2, current_version = $3,
                     updated_by = $4, updated_at = now()
                 WHERE id = $5 RETURNING {DOC_COLS}"
            ))
            .bind(content)
            .bind(&hash)
            .bind(new_version)
            .bind(ctx.user_id)
            .bind(doc_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_db)?;
            self.insert_version(&mut tx, ctx, doc_id, new_version, content, &hash, kind)
                .await?;
            doc.into()
        };

        self.reindex_chunks(&mut tx, ctx, doc_id, content).await?;
        audit(
            &mut tx,
            ctx,
            "doc.update",
            Some(&doc_id.to_string()),
            json!({"version": doc.current_version}),
        )
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(UpdateOutcome::Updated(doc))
    }

    /// Atomically append to a document (serialised by the row lock); always a new version.
    pub async fn append_to_document(
        &self,
        ctx: &AuthContext,
        doc_id: Uuid,
        addition: &str,
    ) -> Result<Document> {
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let cur = self.lock_document(&mut tx, doc_id).await?;
        self.authorize_doc(&mut tx, ctx, doc_id, cur.project_id, Role::Editor)
            .await?;

        let mut new_content = cur.content.clone();
        if !new_content.is_empty() && !new_content.ends_with('\n') {
            new_content.push('\n');
        }
        new_content.push_str(addition);
        validate::validate_content_size(&new_content, self.max_doc_bytes)?;

        let hash = crypto::content_hash(&new_content);
        let new_version = cur.current_version + 1;
        let doc = sqlx::query_as::<_, crate::rows::DocumentRow>(&format!(
            "UPDATE documents SET content = $1, content_hash = $2, current_version = $3,
                 updated_by = $4, updated_at = now()
             WHERE id = $5 RETURNING {DOC_COLS}"
        ))
        .bind(&new_content)
        .bind(&hash)
        .bind(new_version)
        .bind(ctx.user_id)
        .bind(doc_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db)?;
        self.insert_version(
            &mut tx,
            ctx,
            doc_id,
            new_version,
            &new_content,
            &hash,
            VersionKind::Autosave,
        )
        .await?;
        self.reindex_chunks(&mut tx, ctx, doc_id, &new_content)
            .await?;
        audit(
            &mut tx,
            ctx,
            "doc.append",
            Some(&doc_id.to_string()),
            json!({"version": new_version}),
        )
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(doc.into())
    }

    pub async fn move_document(
        &self,
        ctx: &AuthContext,
        doc_id: Uuid,
        new_path: &str,
    ) -> Result<Document> {
        validate::validate_path(new_path)?;
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let cur = self.lock_document(&mut tx, doc_id).await?;
        self.authorize_doc(&mut tx, ctx, doc_id, cur.project_id, Role::Editor)
            .await?;
        let doc = sqlx::query_as::<_, crate::rows::DocumentRow>(&format!(
            "UPDATE documents SET path = $1, updated_by = $2, updated_at = now()
             WHERE id = $3 AND deleted_at IS NULL RETURNING {DOC_COLS}"
        ))
        .bind(new_path)
        .bind(ctx.user_id)
        .bind(doc_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db)?
        .ok_or(Error::NotFound)?;
        audit(
            &mut tx,
            ctx,
            "doc.move",
            Some(&doc_id.to_string()),
            json!({"path": new_path}),
        )
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(doc.into())
    }

    pub async fn delete_document(&self, ctx: &AuthContext, doc_id: Uuid) -> Result<()> {
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let cur = self.lock_document(&mut tx, doc_id).await?;
        self.authorize_doc(&mut tx, ctx, doc_id, cur.project_id, Role::Editor)
            .await?;
        sqlx::query("UPDATE documents SET deleted_at = now() WHERE id = $1 AND deleted_at IS NULL")
            .bind(doc_id)
            .execute(&mut *tx)
            .await
            .map_err(map_db)?;
        // Drop chunks so a deleted doc no longer appears in search.
        sqlx::query("DELETE FROM doc_chunks WHERE document_id = $1")
            .bind(doc_id)
            .execute(&mut *tx)
            .await
            .map_err(map_db)?;
        audit(
            &mut tx,
            ctx,
            "doc.delete",
            Some(&doc_id.to_string()),
            json!({}),
        )
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(())
    }

    pub async fn undelete_document(&self, ctx: &AuthContext, doc_id: Uuid) -> Result<Document> {
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let project_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT project_id FROM documents WHERE id = $1 AND deleted_at IS NOT NULL",
        )
        .bind(doc_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db)?;
        let project_id = project_id.ok_or(Error::NotFound)?;
        self.authorize_doc(&mut tx, ctx, doc_id, project_id, Role::Editor)
            .await?;
        let doc: Document = sqlx::query_as::<_, crate::rows::DocumentRow>(&format!(
            "UPDATE documents SET deleted_at = NULL, updated_at = now()
             WHERE id = $1 AND deleted_at IS NOT NULL RETURNING {DOC_COLS}"
        ))
        .bind(doc_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db)?
        .ok_or(Error::NotFound)?
        .into();
        self.reindex_chunks(&mut tx, ctx, doc_id, &doc.content)
            .await?;
        audit(
            &mut tx,
            ctx,
            "doc.undelete",
            Some(&doc_id.to_string()),
            json!({}),
        )
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(doc)
    }

    /// Restore a document's content to a prior version (creates a new checkpoint version).
    pub async fn restore_version(
        &self,
        ctx: &AuthContext,
        doc_id: Uuid,
        version: i64,
    ) -> Result<Document> {
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let cur = self.lock_document(&mut tx, doc_id).await?;
        self.authorize_doc(&mut tx, ctx, doc_id, cur.project_id, Role::Editor)
            .await?;
        let snapshot: String = sqlx::query_scalar(
            "SELECT content FROM document_versions WHERE document_id = $1 AND version = $2",
        )
        .bind(doc_id)
        .bind(version)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db)?
        .ok_or(Error::NotFound)?;

        let hash = crypto::content_hash(&snapshot);
        let new_version = cur.current_version + 1;
        let doc = sqlx::query_as::<_, crate::rows::DocumentRow>(&format!(
            "UPDATE documents SET content = $1, content_hash = $2, current_version = $3,
                 updated_by = $4, updated_at = now()
             WHERE id = $5 RETURNING {DOC_COLS}"
        ))
        .bind(&snapshot)
        .bind(&hash)
        .bind(new_version)
        .bind(ctx.user_id)
        .bind(doc_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db)?;
        self.insert_version(
            &mut tx,
            ctx,
            doc_id,
            new_version,
            &snapshot,
            &hash,
            VersionKind::Checkpoint,
        )
        .await?;
        self.reindex_chunks(&mut tx, ctx, doc_id, &snapshot).await?;
        audit(
            &mut tx,
            ctx,
            "doc.restore",
            Some(&doc_id.to_string()),
            json!({"restored_from": version, "new_version": new_version}),
        )
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(doc.into())
    }

    pub async fn get_history(
        &self,
        ctx: &AuthContext,
        doc_id: Uuid,
    ) -> Result<Vec<VersionSummary>> {
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        self.authorize_doc_read(&mut tx, ctx, doc_id).await?;
        let rows = sqlx::query_as::<_, crate::rows::VersionSummaryRow>(
            "SELECT version, version_kind, actor_type, actor_id, content_hash, created_at
             FROM document_versions WHERE document_id = $1 ORDER BY version DESC",
        )
        .bind(doc_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        rows.into_iter().map(|r| r.into_core()).collect()
    }

    pub async fn get_version(
        &self,
        ctx: &AuthContext,
        doc_id: Uuid,
        version: i64,
    ) -> Result<DocumentVersion> {
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        self.authorize_doc_read(&mut tx, ctx, doc_id).await?;
        let row = sqlx::query_as::<_, crate::rows::VersionRow>(
            "SELECT id, document_id, version, content, content_hash, version_kind,
                    actor_type, actor_id, created_at
             FROM document_versions WHERE document_id = $1 AND version = $2",
        )
        .bind(doc_id)
        .bind(version)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        row.ok_or(Error::NotFound)?.into_core()
    }

    // --- internal helpers -------------------------------------------------

    async fn lock_document(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        doc_id: Uuid,
    ) -> Result<Document> {
        let row = sqlx::query_as::<_, crate::rows::DocumentRow>(&format!(
            "SELECT {DOC_COLS} FROM documents WHERE id = $1 AND deleted_at IS NULL FOR UPDATE"
        ))
        .bind(doc_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(map_db)?;
        row.map(Into::into).ok_or(Error::NotFound)
    }

    /// Authorize at least viewer access on a (non-deleted) document by id.
    async fn authorize_doc_read(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        ctx: &AuthContext,
        doc_id: Uuid,
    ) -> Result<()> {
        let project_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT project_id FROM documents WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(doc_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(map_db)?;
        let project_id = project_id.ok_or(Error::NotFound)?;
        self.authorize_doc(tx, ctx, doc_id, project_id, Role::Viewer)
            .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn insert_version(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        ctx: &AuthContext,
        doc_id: Uuid,
        version: i64,
        content: &str,
        hash: &str,
        kind: VersionKind,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO document_versions
               (id, org_id, document_id, version, content, content_hash,
                version_kind, actor_type, actor_id)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        )
        .bind(Uuid::now_v7())
        .bind(ctx.org_id)
        .bind(doc_id)
        .bind(version)
        .bind(content)
        .bind(hash)
        .bind(kind.as_str())
        .bind(ctx.actor_type.as_str())
        .bind(ctx.user_id)
        .execute(&mut **tx)
        .await
        .map_err(map_db)?;
        Ok(())
    }

    /// If the latest version is a recent same-actor autosave, overwrite it in place and
    /// return the updated document; otherwise return `None`.
    async fn try_coalesce(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        ctx: &AuthContext,
        doc_id: Uuid,
        content: &str,
        hash: &str,
        kind: VersionKind,
    ) -> Result<Option<Document>> {
        if kind != VersionKind::Autosave {
            return Ok(None);
        }
        let latest = sqlx::query_as::<_, crate::rows::VersionRow>(
            "SELECT id, document_id, version, content, content_hash, version_kind,
                    actor_type, actor_id, created_at
             FROM document_versions WHERE document_id = $1 ORDER BY version DESC LIMIT 1",
        )
        .bind(doc_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(map_db)?;

        let Some(latest) = latest else {
            return Ok(None);
        };
        let within_window = (OffsetDateTime::now_utc() - latest.created_at)
            < Duration::seconds(self.autosave_debounce_secs);
        let same_actor =
            latest.actor_id == ctx.user_id && latest.actor_type == ctx.actor_type.as_str();
        if !(latest.version_kind == "autosave" && same_actor && within_window) {
            return Ok(None);
        }

        sqlx::query(
            "UPDATE document_versions SET content = $1, content_hash = $2, created_at = now()
             WHERE id = $3",
        )
        .bind(content)
        .bind(hash)
        .bind(latest.id)
        .execute(&mut **tx)
        .await
        .map_err(map_db)?;

        let doc = sqlx::query_as::<_, crate::rows::DocumentRow>(&format!(
            "UPDATE documents SET content = $1, content_hash = $2, updated_by = $3,
                 updated_at = now() WHERE id = $4 RETURNING {DOC_COLS}"
        ))
        .bind(content)
        .bind(hash)
        .bind(ctx.user_id)
        .bind(doc_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(map_db)?;
        Ok(Some(doc.into()))
    }

    async fn reindex_chunks(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        ctx: &AuthContext,
        doc_id: Uuid,
        content: &str,
    ) -> Result<()> {
        sqlx::query("DELETE FROM doc_chunks WHERE document_id = $1")
            .bind(doc_id)
            .execute(&mut **tx)
            .await
            .map_err(map_db)?;
        for chunk in mdm_core::chunk::chunk_markdown(content) {
            sqlx::query(
                "INSERT INTO doc_chunks (id, org_id, document_id, chunk_index, heading_path, content)
                 VALUES ($1,$2,$3,$4,$5,$6)",
            )
            .bind(Uuid::now_v7())
            .bind(ctx.org_id)
            .bind(doc_id)
            .bind(chunk.index)
            .bind(&chunk.heading_path)
            .bind(&chunk.content)
            .execute(&mut **tx)
            .await
            .map_err(map_db)?;
        }
        Ok(())
    }
}
