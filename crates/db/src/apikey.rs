//! API-key management (mint / list / revoke).

use mdm_core::model::{ApiKeyCreated, ApiKeyInfo, AuthContext, OrgRole};
use mdm_core::{Error, Result, crypto, ids, rbac};
use serde_json::json;
use uuid::Uuid;

use crate::{Db, audit, map_db};

impl Db {
    /// Mint a new API key. The key's role is clamped to the caller's own role.
    /// The secret is returned exactly once.
    pub async fn create_api_key(
        &self,
        ctx: &AuthContext,
        name: &str,
        role: OrgRole,
    ) -> Result<ApiKeyCreated> {
        rbac::require_admin(ctx)?;
        let role = role.min(ctx.org_role);
        let key = crypto::generate_api_key();
        let key_hash = crypto::hash_api_key(&self.pepper, &key.secret);

        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let row = sqlx::query_as::<_, crate::rows::ApiKeyInfoRow>(
            "INSERT INTO api_keys (id, org_id, name, key_prefix, key_hash, role, created_by)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             RETURNING id, name, key_prefix, role, created_at, last_used_at, revoked_at",
        )
        .bind(ids::new_id())
        .bind(ctx.org_id)
        .bind(name)
        .bind(&key.prefix)
        .bind(&key_hash)
        .bind(role.as_str())
        .bind(ctx.user_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db)?;
        audit(&mut tx, ctx, "apikey.create", Some(&row.id.to_string()), json!({"name": name}))
            .await
            .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;

        Ok(ApiKeyCreated {
            info: row.into_core()?,
            secret: key.secret,
        })
    }

    pub async fn list_api_keys(&self, ctx: &AuthContext) -> Result<Vec<ApiKeyInfo>> {
        rbac::require_admin(ctx)?;
        // api_keys is RLS-exempt; scope explicitly by org.
        let rows = sqlx::query_as::<_, crate::rows::ApiKeyInfoRow>(
            "SELECT id, name, key_prefix, role, created_at, last_used_at, revoked_at
             FROM api_keys WHERE org_id = $1 ORDER BY created_at DESC",
        )
        .bind(ctx.org_id)
        .fetch_all(self.pool())
        .await
        .map_err(map_db)?;
        rows.into_iter().map(|r| r.into_core()).collect()
    }

    pub async fn revoke_api_key(&self, ctx: &AuthContext, key_id: Uuid) -> Result<()> {
        rbac::require_admin(ctx)?;
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let affected = sqlx::query(
            "UPDATE api_keys SET revoked_at = now()
             WHERE id = $1 AND org_id = $2 AND revoked_at IS NULL",
        )
        .bind(key_id)
        .bind(ctx.org_id)
        .execute(&mut *tx)
        .await
        .map_err(map_db)?
        .rows_affected();
        if affected == 0 {
            return Err(Error::NotFound);
        }
        audit(&mut tx, ctx, "apikey.revoke", Some(&key_id.to_string()), json!({}))
            .await
            .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(())
    }
}
