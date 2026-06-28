//! Audit-log queries (admin-only). The `audit` free function in `lib.rs` writes entries;
//! this reads them back, RLS-scoped to the caller's org.

use mdm_core::model::{AuditEntry, AuthContext};
use mdm_core::{Result, rbac};

use crate::rows::AuditRow;
use crate::{Db, map_db};

impl Db {
    /// List recent audit entries for the caller's org, newest first. Optionally filter by
    /// `target` (e.g. a document id) and/or an `action` prefix (e.g. `doc.` or `share.`).
    pub async fn list_audit(
        &self,
        ctx: &AuthContext,
        target: Option<&str>,
        action_prefix: Option<&str>,
        limit: i64,
    ) -> Result<Vec<AuditEntry>> {
        rbac::require_admin(ctx)?;
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let rows = sqlx::query_as::<_, AuditRow>(
            "SELECT id, actor_type, actor_id, action, target, metadata::text AS metadata, created_at
             FROM audit_log
             WHERE ($1::text IS NULL OR target = $1)
               AND ($2::text IS NULL OR action LIKE $2 || '%')
             ORDER BY created_at DESC LIMIT $3",
        )
        .bind(target)
        .bind(action_prefix)
        .bind(limit)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        rows.into_iter().map(|r| r.into_core()).collect()
    }
}
