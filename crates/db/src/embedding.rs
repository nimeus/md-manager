//! Vector embeddings: schema management, the background-worker store, and
//! semantic + hybrid search. The embedding column dimension comes from config (env),
//! so it's created at runtime rather than in a static migration.
//!
//! The worker runs as the **owner** role and `doc_chunks` is `NO FORCE ROW LEVEL SECURITY`,
//! so the owner can read/write chunks across orgs (a trusted system process). The app role
//! (`md_app`, non-owner) is still RLS-scoped, so user-facing search stays tenant-isolated.

use anyhow::Context;
use mdm_core::model::{AuthContext, OrgRole, SearchHit};
use mdm_core::{Result, rbac};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use crate::{Db, map_db};

/// Format an f32 vector as a pgvector text literal: `[1,2,3]`.
pub fn to_pgvector_literal(v: &[f32]) -> String {
    let mut s = String::with_capacity(v.len() * 8 + 2);
    s.push('[');
    for (i, x) in v.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&x.to_string());
    }
    s.push(']');
    s
}

/// Owner-role handle for the embedding background worker (bypasses doc_chunks RLS).
pub struct EmbeddingStore {
    pool: PgPool,
}

impl EmbeddingStore {
    /// Connect as the owner role and ensure the embedding schema (column + index) at `dims`.
    pub async fn connect(owner_url: &str, dims: i32) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .connect(owner_url)
            .await
            .context("connecting as owner for the embedding worker")?;
        ensure_schema(&pool, dims).await?;
        Ok(Self { pool })
    }

    /// Fetch up to `limit` chunks lacking an embedding (text = heading breadcrumb + content).
    pub async fn pending(&self, limit: i64) -> anyhow::Result<Vec<(Uuid, String)>> {
        let rows: Vec<(Uuid, String)> = sqlx::query_as(
            "SELECT id, (coalesce(heading_path,'') || ' ' || content)
             FROM doc_chunks WHERE embedding IS NULL ORDER BY id LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("fetching unembedded chunks")?;
        Ok(rows)
    }

    pub async fn store(&self, chunk_id: Uuid, vector: &[f32]) -> anyhow::Result<()> {
        sqlx::query("UPDATE doc_chunks SET embedding = $1::vector WHERE id = $2")
            .bind(to_pgvector_literal(vector))
            .bind(chunk_id)
            .execute(&self.pool)
            .await
            .context("storing chunk embedding")?;
        Ok(())
    }

    pub async fn count_unembedded(&self) -> anyhow::Result<i64> {
        Ok(
            sqlx::query_scalar("SELECT count(*) FROM doc_chunks WHERE embedding IS NULL")
                .fetch_one(&self.pool)
                .await?,
        )
    }

    /// Copy an existing embedding onto any unembedded chunk with identical content
    /// (same `content_hash`), so duplicate content is never re-embedded. Returns the count
    /// copied. Run before [`EmbeddingStore::pending`].
    pub async fn dedup_by_content_hash(&self) -> anyhow::Result<u64> {
        let affected = sqlx::query(
            "UPDATE doc_chunks c SET embedding = d.embedding
             FROM doc_chunks d
             WHERE c.embedding IS NULL AND d.embedding IS NOT NULL
               AND c.content_hash <> '' AND d.content_hash = c.content_hash AND c.id <> d.id",
        )
        .execute(&self.pool)
        .await?
        .rows_affected();
        Ok(affected)
    }

    /// Count of embedded chunks for a document (used in tests).
    pub async fn embedded_count(&self, document_id: Uuid) -> anyhow::Result<i64> {
        Ok(sqlx::query_scalar(
            "SELECT count(*) FROM doc_chunks WHERE document_id = $1 AND embedding IS NOT NULL",
        )
        .bind(document_id)
        .fetch_one(&self.pool)
        .await?)
    }
}

async fn ensure_schema(pool: &PgPool, dims: i32) -> anyhow::Result<()> {
    anyhow::ensure!(dims > 0, "embedding dimensions must be > 0");
    let has_ext: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_extension WHERE extname = 'vector')")
            .fetch_one(pool)
            .await?;
    anyhow::ensure!(
        has_ext,
        "pgvector not installed — run `CREATE EXTENSION vector;` as a superuser in the app database (see docs/embeddings.md)"
    );
    // dims is a validated integer; safe to inline.
    sqlx::query(&format!(
        "ALTER TABLE doc_chunks ADD COLUMN IF NOT EXISTS embedding vector({dims})"
    ))
    .execute(pool)
    .await
    .context("adding embedding column")?;
    // Let the owner-run worker see chunks across orgs (md_app stays RLS-scoped).
    sqlx::query("ALTER TABLE doc_chunks NO FORCE ROW LEVEL SECURITY")
        .execute(pool)
        .await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS doc_chunks_embedding_hnsw
         ON doc_chunks USING hnsw (embedding vector_cosine_ops)",
    )
    .execute(pool)
    .await
    .context("creating HNSW index")?;
    Ok(())
}

impl Db {
    /// Semantic (vector) search within the caller's org, deny-filtered, doc-aggregated.
    pub async fn semantic_search(
        &self,
        ctx: &AuthContext,
        project_id: Option<Uuid>,
        query_vector: &[f32],
        limit: i64,
    ) -> Result<Vec<SearchHit>> {
        rbac::require_read(ctx)?;
        let privileged = matches!(ctx.org_role, OrgRole::Owner | OrgRole::Admin);
        let literal = to_pgvector_literal(query_vector);
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let rows = sqlx::query_as::<_, crate::rows::SearchRow>(
            "SELECT document_id, project_id, path, title, heading_path, snippet, rank
             FROM (
               SELECT DISTINCT ON (d.id)
                 d.id AS document_id, d.project_id, d.path, d.title, c.heading_path,
                 left(c.content, 200) AS snippet,
                 (1 - (c.embedding <=> $1::vector))::real AS rank
               FROM doc_chunks c JOIN documents d ON d.id = c.document_id
               WHERE d.deleted_at IS NULL AND c.embedding IS NOT NULL
                 AND ($2::uuid IS NULL OR d.project_id = $2)
                 AND ($4 OR NOT EXISTS (
                   SELECT 1 FROM document_grants g
                   WHERE g.document_id = d.id AND g.role = 'none'
                     AND ((g.subject_type = 'user' AND g.subject_id = $5)
                       OR (g.subject_type = 'team' AND g.subject_id IN
                           (SELECT team_id FROM team_members WHERE user_id = $5)))))
               ORDER BY d.id, c.embedding <=> $1::vector
             ) hits
             ORDER BY rank DESC LIMIT $3",
        )
        .bind(&literal)
        .bind(project_id)
        .bind(limit)
        .bind(privileged)
        .bind(ctx.user_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Hybrid search: Reciprocal Rank Fusion of keyword (FTS) and vector rankings.
    pub async fn hybrid_search(
        &self,
        ctx: &AuthContext,
        project_id: Option<Uuid>,
        query: &str,
        query_vector: &[f32],
        limit: i64,
    ) -> Result<Vec<SearchHit>> {
        rbac::require_read(ctx)?;
        let privileged = matches!(ctx.org_role, OrgRole::Owner | OrgRole::Admin);
        let literal = to_pgvector_literal(query_vector);
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        // RRF with k=60. $1 query text, $2 vector, $3 project, $4 limit, $5 privileged, $6 user.
        let rows = sqlx::query_as::<_, crate::rows::SearchRow>(
            "WITH q AS (SELECT websearch_to_tsquery('english', $1) AS tsq, $2::vector AS vec),
             kw AS (
               SELECT d.id AS doc, max(ts_rank_cd(c.tsv, q.tsq)) AS s
               FROM doc_chunks c JOIN documents d ON d.id = c.document_id, q
               WHERE d.deleted_at IS NULL AND c.tsv @@ q.tsq
                 AND ($3::uuid IS NULL OR d.project_id = $3)
                 AND ($5 OR NOT EXISTS (SELECT 1 FROM document_grants g
                   WHERE g.document_id = d.id AND g.role='none'
                     AND ((g.subject_type='user' AND g.subject_id=$6)
                       OR (g.subject_type='team' AND g.subject_id IN
                           (SELECT team_id FROM team_members WHERE user_id=$6)))))
               GROUP BY d.id
             ),
             kw_ranked AS (SELECT doc, row_number() OVER (ORDER BY s DESC) AS rk FROM kw),
             vec AS (
               SELECT d.id AS doc, min(c.embedding <=> q.vec) AS dist
               FROM doc_chunks c JOIN documents d ON d.id = c.document_id, q
               WHERE d.deleted_at IS NULL AND c.embedding IS NOT NULL
                 AND ($3::uuid IS NULL OR d.project_id = $3)
                 AND ($5 OR NOT EXISTS (SELECT 1 FROM document_grants g
                   WHERE g.document_id = d.id AND g.role='none'
                     AND ((g.subject_type='user' AND g.subject_id=$6)
                       OR (g.subject_type='team' AND g.subject_id IN
                           (SELECT team_id FROM team_members WHERE user_id=$6)))))
               GROUP BY d.id
             ),
             vec_ranked AS (SELECT doc, row_number() OVER (ORDER BY dist ASC) AS rk FROM vec),
             fused AS (
               SELECT coalesce(kw_ranked.doc, vec_ranked.doc) AS doc,
                      coalesce(1.0/(60+kw_ranked.rk),0) + coalesce(1.0/(60+vec_ranked.rk),0) AS rrf
               FROM kw_ranked FULL OUTER JOIN vec_ranked ON kw_ranked.doc = vec_ranked.doc
             )
             SELECT d.id AS document_id, d.project_id, d.path, d.title,
                    ''::text AS heading_path, left(d.content, 200) AS snippet, f.rrf::real AS rank
             FROM fused f JOIN documents d ON d.id = f.doc
             ORDER BY f.rrf DESC LIMIT $4",
        )
        .bind(query)
        .bind(&literal)
        .bind(project_id)
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
