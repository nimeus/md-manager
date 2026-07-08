//! Public, read-only, expiring share links for documents.

use mdm_core::model::{
    ActorType, AuthContext, Role, ShareLinkCreated, ShareLinkInfo, SharedDocument,
};
use mdm_core::{Error, Result, crypto, ids, rbac};
use serde_json::json;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::rows::{ShareLinkAuthRow, ShareLinkInfoRow, SharedDocRow};
use crate::{Db, audit, map_db};

const SHARE_INFO_COLS: &str =
    "id, document_id, token_prefix, audience, created_at, expires_at, revoked_at";

impl Db {
    /// Mint a read-only share link for a document. The caller must be able to edit it.
    /// The token is returned once.
    pub async fn create_share_link(
        &self,
        ctx: &AuthContext,
        doc_id: Uuid,
        audience: &str,
        recipients: &[String],
        expires_in_days: Option<i64>,
    ) -> Result<ShareLinkCreated> {
        if !matches!(audience, "public" | "members" | "emails") {
            return Err(Error::invalid(
                "audience must be public, members, or emails",
            ));
        }
        let emails: Vec<String> = if audience == "emails" {
            let e: Vec<String> = recipients
                .iter()
                .map(|s| s.trim().to_lowercase())
                .filter(|s| s.contains('@'))
                .collect();
            if e.is_empty() {
                return Err(Error::invalid("add at least one recipient email"));
            }
            e
        } else {
            Vec::new()
        };

        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let project_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT project_id FROM documents WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(doc_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db)?;
        let project_id = project_id.ok_or(Error::NotFound)?;
        self.authorize_doc(&mut tx, ctx, doc_id, project_id, Role::Editor)
            .await?;

        let token = crypto::generate_share_token();
        let hash = crypto::hash_token(&self.pepper, &token.secret);
        let id = ids::new_id();
        let expires_at = expires_in_days
            .filter(|d| *d > 0)
            .map(|d| OffsetDateTime::now_utc() + Duration::days(d));

        let row = sqlx::query_as::<_, ShareLinkInfoRow>(&format!(
            "INSERT INTO share_links
               (id, org_id, document_id, token_prefix, token_hash, audience, created_by, expires_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
             RETURNING {SHARE_INFO_COLS}"
        ))
        .bind(id)
        .bind(ctx.org_id)
        .bind(doc_id)
        .bind(&token.prefix)
        .bind(&hash)
        .bind(audience)
        .bind(ctx.user_id)
        .bind(expires_at)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db)?;

        for email in &emails {
            sqlx::query(
                "INSERT INTO share_link_recipients (share_link_id, email) VALUES ($1, $2)
                 ON CONFLICT DO NOTHING",
            )
            .bind(id)
            .bind(email)
            .execute(&mut *tx)
            .await
            .map_err(map_db)?;
        }

        audit(
            &mut tx,
            ctx,
            "share.create",
            Some(&id.to_string()),
            json!({ "document_id": doc_id, "audience": audience }),
        )
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;

        Ok(ShareLinkCreated {
            info: row.into(),
            token: token.secret,
        })
    }

    pub async fn list_share_links(
        &self,
        ctx: &AuthContext,
        doc_id: Uuid,
    ) -> Result<Vec<ShareLinkInfo>> {
        rbac::require_read(ctx)?;
        // share_links is RLS-exempt; scope explicitly by org.
        let rows = sqlx::query_as::<_, ShareLinkInfoRow>(&format!(
            "SELECT {SHARE_INFO_COLS} FROM share_links
             WHERE document_id = $1 AND org_id = $2 ORDER BY created_at DESC"
        ))
        .bind(doc_id)
        .bind(ctx.org_id)
        .fetch_all(self.pool())
        .await
        .map_err(map_db)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn revoke_share_link(&self, ctx: &AuthContext, link_id: Uuid) -> Result<()> {
        let doc_id: Option<Uuid> =
            sqlx::query_scalar("SELECT document_id FROM share_links WHERE id = $1 AND org_id = $2")
                .bind(link_id)
                .bind(ctx.org_id)
                .fetch_optional(self.pool())
                .await
                .map_err(map_db)?;
        let doc_id = doc_id.ok_or(Error::NotFound)?;

        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let project_id: Option<Uuid> =
            sqlx::query_scalar("SELECT project_id FROM documents WHERE id = $1")
                .bind(doc_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(map_db)?;
        let project_id = project_id.ok_or(Error::NotFound)?;
        self.authorize_doc(&mut tx, ctx, doc_id, project_id, Role::Editor)
            .await?;

        let affected = sqlx::query(
            "UPDATE share_links SET revoked_at = now()
             WHERE id = $1 AND org_id = $2 AND revoked_at IS NULL",
        )
        .bind(link_id)
        .bind(ctx.org_id)
        .execute(&mut *tx)
        .await
        .map_err(map_db)?
        .rows_affected();
        if affected == 0 {
            return Err(Error::NotFound);
        }
        audit(
            &mut tx,
            ctx,
            "share.revoke",
            Some(&link_id.to_string()),
            json!({}),
        )
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(())
    }

    /// Resolve a share token to a read-only document view, enforcing its audience:
    /// `public` (anyone), `members` (a signed-in member of the doc's org), or `emails` (a
    /// signed-in allow-listed recipient). `viewer` is the signed-in user id (None if anonymous).
    /// Invalid/expired/revoked → `NotFound`; needs sign-in → `Unauthorized`; signed-in but not
    /// allowed → `Forbidden`.
    pub async fn resolve_share_link(
        &self,
        token: &str,
        viewer: Option<Uuid>,
    ) -> Result<SharedDocument> {
        let prefix = crypto::token_prefix("sl", token).ok_or(Error::NotFound)?;
        let candidates = sqlx::query_as::<_, ShareLinkAuthRow>(
            "SELECT id, org_id, document_id, token_hash, audience, expires_at, revoked_at
             FROM share_links WHERE token_prefix = $1",
        )
        .bind(&prefix)
        .fetch_all(self.pool())
        .await
        .map_err(map_db)?;

        let now = OffsetDateTime::now_utc();
        let link = candidates
            .into_iter()
            .find(|l| crypto::verify_token(&self.pepper, token, &l.token_hash))
            .ok_or(Error::NotFound)?;
        if link.revoked_at.is_some() {
            return Err(Error::NotFound);
        }
        if let Some(exp) = link.expires_at
            && exp <= now
        {
            return Err(Error::NotFound);
        }

        // Audience gate.
        match link.audience.as_str() {
            "public" => {}
            "members" => {
                let uid = viewer.ok_or(Error::Unauthorized)?;
                let mut tx = self
                    .begin_scoped(link.org_id, uid, ActorType::User)
                    .await
                    .map_err(map_db)?;
                let role: Option<String> = sqlx::query_scalar(
                    "SELECT role FROM organization_members WHERE org_id = $1 AND user_id = $2",
                )
                .bind(link.org_id)
                .bind(uid)
                .fetch_optional(&mut *tx)
                .await
                .map_err(map_db)?;
                tx.commit().await.map_err(map_db)?;
                if role.is_none() {
                    return Err(Error::Forbidden);
                }
            }
            "emails" => {
                let uid = viewer.ok_or(Error::Unauthorized)?;
                let email: Option<String> =
                    sqlx::query_scalar("SELECT email FROM users WHERE id = $1")
                        .bind(uid)
                        .fetch_optional(self.pool())
                        .await
                        .map_err(map_db)?;
                let email = email.ok_or(Error::Unauthorized)?.to_lowercase();
                let allowed: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM share_link_recipients
                                   WHERE share_link_id = $1 AND lower(email) = $2)",
                )
                .bind(link.id)
                .bind(&email)
                .fetch_one(self.pool())
                .await
                .map_err(map_db)?;
                if !allowed {
                    return Err(Error::Forbidden);
                }
            }
            _ => return Err(Error::NotFound),
        }

        // Read the linked document scoped to its org.
        let mut tx = self
            .begin_scoped(
                link.org_id,
                viewer.unwrap_or_else(Uuid::nil),
                ActorType::User,
            )
            .await
            .map_err(map_db)?;
        let row = sqlx::query_as::<_, SharedDocRow>(
            "SELECT id AS document_id, path, title, content, updated_at
             FROM documents WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(link.document_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        row.map(Into::into).ok_or(Error::NotFound)
    }
}
