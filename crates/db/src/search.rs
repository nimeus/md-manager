//! Keyword full-text search over document chunks, aggregated to the document level.

use mdm_core::model::{AuthContext, SearchHit};
use mdm_core::{Result, rbac};
use uuid::Uuid;

use crate::{Db, map_db};

impl Db {
    /// Full-text search within the caller's org (optionally scoped to one project).
    ///
    /// Ranks chunks with `ts_rank_cd`, then keeps the best chunk per document
    /// (`DISTINCT ON`) so results are documents, not raw chunks.
    pub async fn search(
        &self,
        ctx: &AuthContext,
        project_id: Option<Uuid>,
        query: &str,
        limit: i64,
    ) -> Result<Vec<SearchHit>> {
        rbac::require_read(ctx)?;
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let rows = sqlx::query_as::<_, crate::rows::SearchRow>(
            "SELECT document_id, project_id, path, title, heading_path, snippet, rank
             FROM (
               SELECT DISTINCT ON (d.id)
                 d.id          AS document_id,
                 d.project_id  AS project_id,
                 d.path        AS path,
                 d.title       AS title,
                 c.heading_path AS heading_path,
                 ts_headline('english', c.content,
                             websearch_to_tsquery('english', $1),
                             'MaxFragments=1,MinWords=5,MaxWords=20,StartSel=**,StopSel=**') AS snippet,
                 ts_rank_cd(c.tsv, websearch_to_tsquery('english', $1)) AS rank
               FROM doc_chunks c
               JOIN documents d ON d.id = c.document_id
               WHERE d.deleted_at IS NULL
                 AND ($2::uuid IS NULL OR d.project_id = $2)
                 AND c.tsv @@ websearch_to_tsquery('english', $1)
               ORDER BY d.id, ts_rank_cd(c.tsv, websearch_to_tsquery('english', $1)) DESC
             ) hits
             ORDER BY rank DESC
             LIMIT $3",
        )
        .bind(query)
        .bind(project_id)
        .bind(limit)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
}
