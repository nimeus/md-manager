//! API-key authentication and the dev bootstrap.

use mdm_core::model::{ActorType, ApiKeyCreated, AuthContext, Organization, OrgRole, User};
use mdm_core::{Error, Result, crypto, ids, validate};
use time::OffsetDateTime;

use crate::rows::{ApiKeyAuthRow, ApiKeyInfoRow, OrgRow, UserRow};
use crate::{Db, map_db};

impl Db {
    /// Authenticate a presented API key (`mk_…`) and resolve the request context.
    ///
    /// The key is looked up by prefix across orgs (api_keys is RLS-exempt), its hash is
    /// verified in constant time, and the effective role is `min(key role, creator's
    /// CURRENT org role)` — so a key dies if its creator is demoted or removed.
    pub async fn authenticate_api_key(&self, secret: &str) -> Result<AuthContext> {
        let prefix = crypto::key_prefix(secret).ok_or(Error::Unauthorized)?;

        let candidates = sqlx::query_as::<_, ApiKeyAuthRow>(
            "SELECT id, org_id, key_hash, role, created_by, revoked_at, expires_at
             FROM api_keys WHERE key_prefix = $1",
        )
        .bind(&prefix)
        .fetch_all(self.pool())
        .await
        .map_err(map_db)?;

        let now = OffsetDateTime::now_utc();
        let key = candidates
            .into_iter()
            .find(|row| crypto::verify_api_key(&self.pepper, secret, &row.key_hash))
            .ok_or(Error::Unauthorized)?;

        if key.revoked_at.is_some() {
            return Err(Error::Unauthorized);
        }
        if let Some(exp) = key.expires_at {
            if exp <= now {
                return Err(Error::Unauthorized);
            }
        }
        let key_role = key.role()?;

        // Re-check the creator's current membership/role under org scope, and touch last_used.
        let mut tx = self
            .begin_scoped(key.org_id, key.created_by, ActorType::Agent)
            .await
            .map_err(map_db)?;
        let creator_role: Option<String> = sqlx::query_scalar(
            "SELECT role FROM organization_members WHERE org_id = $1 AND user_id = $2",
        )
        .bind(key.org_id)
        .bind(key.created_by)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db)?;
        let _ = sqlx::query("UPDATE api_keys SET last_used_at = now() WHERE id = $1")
            .bind(key.id)
            .execute(&mut *tx)
            .await;
        tx.commit().await.map_err(map_db)?;

        let creator_role = OrgRole::from_db(&creator_role.ok_or(Error::Unauthorized)?)?;
        let effective = key_role.min(creator_role);

        Ok(AuthContext {
            org_id: key.org_id,
            user_id: key.created_by,
            actor_type: ActorType::Agent,
            org_role: effective,
        })
    }

    /// Bootstrap a tenant: create (or reuse) a user, a new org with the user as owner, and
    /// an initial admin API key. The key secret is returned once. Gated by the caller
    /// (the API checks the bootstrap token before invoking this).
    pub async fn bootstrap(
        &self,
        email: &str,
        display_name: &str,
        org_slug: &str,
        org_name: &str,
        key_name: &str,
    ) -> Result<(Organization, User, ApiKeyCreated)> {
        validate::validate_slug(org_slug)?;
        if email.trim().is_empty() {
            return Err(Error::invalid("email is required"));
        }

        let user_id = ids::new_id();
        let org_id = ids::new_id();
        let key = crypto::generate_api_key();
        let key_hash = crypto::hash_api_key(&self.pepper, &key.secret);

        let mut tx = self
            .begin_scoped(org_id, user_id, ActorType::User)
            .await
            .map_err(map_db)?;

        let user = sqlx::query_as::<_, UserRow>(
            "INSERT INTO users (id, email, display_name) VALUES ($1, $2, $3)
             ON CONFLICT (email) DO UPDATE SET display_name = EXCLUDED.display_name
             RETURNING id, email, display_name",
        )
        .bind(user_id)
        .bind(email)
        .bind(display_name)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db)?;
        let actual_user_id = user.id;

        let org = sqlx::query_as::<_, OrgRow>(
            "INSERT INTO organizations (id, slug, name) VALUES ($1, $2, $3)
             RETURNING id, slug, name, created_at",
        )
        .bind(org_id)
        .bind(org_slug)
        .bind(org_name)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db)?;

        sqlx::query(
            "INSERT INTO organization_members (org_id, user_id, role) VALUES ($1, $2, 'owner')",
        )
        .bind(org_id)
        .bind(actual_user_id)
        .execute(&mut *tx)
        .await
        .map_err(map_db)?;

        let info_row = sqlx::query_as::<_, ApiKeyInfoRow>(
            "INSERT INTO api_keys (id, org_id, name, key_prefix, key_hash, role, created_by)
             VALUES ($1, $2, $3, $4, $5, 'admin', $6)
             RETURNING id, name, key_prefix, role, created_at, last_used_at, revoked_at",
        )
        .bind(ids::new_id())
        .bind(org_id)
        .bind(key_name)
        .bind(&key.prefix)
        .bind(&key_hash)
        .bind(actual_user_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db)?;

        tx.commit().await.map_err(map_db)?;

        Ok((
            org.into(),
            user.into(),
            ApiKeyCreated {
                info: info_row.into_core()?,
                secret: key.secret,
            },
        ))
    }
}
