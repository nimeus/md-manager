//! Web SaaS identity: Google-backed JIT user provisioning, multi-org listing/sessions, org
//! creation, and email invitations. The API layer verifies the Google ID token and signs the
//! session JWT; this module owns the database side (users/orgs/members/invitations) and keeps
//! the Rust+Postgres backend the single source of truth for who-belongs-to-what.

use mdm_core::model::{
    ActorType, AuthContext, Invitation, InvitationCreated, OrgMember, OrgRole, Organization,
    ProvisionedUser, UserOrg,
};
use mdm_core::{Error, Result, crypto, ids, rbac, validate};
use uuid::Uuid;

use crate::rows::{InvitationRow, OrgRow};
use crate::{Db, audit, map_db};

/// A DNS-safe, unique-ish slug for a user's auto-created personal org: the email local part
/// (alphanumerics only) plus a short hex suffix from the user id.
fn personal_org_slug(user_id: Uuid, email: &str) -> String {
    let local = email.split('@').next().unwrap_or("user");
    let base: String = local
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .take(40)
        .collect();
    let base = if base.is_empty() { "org".into() } else { base };
    let id_hex = user_id.simple().to_string();
    format!("{base}-{}", &id_hex[..6])
}

impl Db {
    /// Resolve a verified Google identity to a user + their orgs, creating what's missing:
    /// link/create the user (by `google_sub`, falling back to email), accept any pending
    /// invitations for the email, and — if they'd otherwise have no org — create a personal
    /// org they own. Idempotent across logins.
    pub async fn provision_google_user(
        &self,
        google_sub: &str,
        email: &str,
        name: &str,
    ) -> Result<ProvisionedUser> {
        let email = email.trim();
        if email.is_empty() {
            return Err(Error::invalid("google account has no email"));
        }
        let display = if name.trim().is_empty() {
            email.split('@').next().unwrap_or("user").to_string()
        } else {
            name.trim().to_string()
        };

        // 1) find-or-create the user. The Google `sub` is the authoritative identity, so we
        //    match on it FIRST. Only if no row owns this sub do we fall back to email — and we
        //    REFUSE to take over an account already linked to a different Google account
        //    (prevents takeover when an email is reused, e.g. a reassigned Workspace address).
        //    Email→sub linking happens only for an as-yet-unlinked row (e.g. a bootstrap-created
        //    user claiming their account); safe because Google asserted `email_verified`.
        let by_sub: Option<(Uuid, String)> =
            sqlx::query_as("SELECT id, email FROM users WHERE google_sub = $1 LIMIT 1")
                .bind(google_sub)
                .fetch_optional(self.pool())
                .await
                .map_err(map_db)?;

        let (user_id, user_email) = if let Some((id, em)) = by_sub {
            sqlx::query(
                "UPDATE users SET
                   display_name = CASE WHEN display_name = '' THEN $1 ELSE display_name END
                 WHERE id = $2",
            )
            .bind(&display)
            .bind(id)
            .execute(self.pool())
            .await
            .map_err(map_db)?;
            (id, em)
        } else {
            let by_email: Option<(Uuid, String, Option<String>)> = sqlx::query_as(
                "SELECT id, email, google_sub FROM users WHERE lower(email) = lower($1) LIMIT 1",
            )
            .bind(email)
            .fetch_optional(self.pool())
            .await
            .map_err(map_db)?;
            match by_email {
                // Email already belongs to a different Google account — refuse silently linking.
                Some((_, _, Some(_))) => {
                    tracing::warn!(%email, "google sign-in refused: email linked to another account");
                    return Err(Error::Unauthorized);
                }
                // Unlinked existing account (e.g. bootstrap-created): link this Google sub.
                Some((id, em, None)) => {
                    sqlx::query(
                        "UPDATE users SET google_sub = $1,
                           display_name = CASE WHEN display_name = '' THEN $2 ELSE display_name END
                         WHERE id = $3",
                    )
                    .bind(google_sub)
                    .bind(&display)
                    .bind(id)
                    .execute(self.pool())
                    .await
                    .map_err(map_db)?;
                    (id, em)
                }
                None => {
                    let id = ids::new_id();
                    sqlx::query(
                        "INSERT INTO users (id, email, display_name, google_sub)
                         VALUES ($1,$2,$3,$4)",
                    )
                    .bind(id)
                    .bind(email)
                    .bind(&display)
                    .bind(google_sub)
                    .execute(self.pool())
                    .await
                    .map_err(map_db)?;
                    (id, email.to_string())
                }
            }
        };

        // 2) accept any pending invitations addressed to this email.
        self.accept_invitations_for(user_id, &user_email).await?;

        // 3) ensure the user lands in at least one org (personal org on first sign-in).
        let mut orgs = self.list_user_orgs(user_id).await?;
        if orgs.is_empty() {
            let slug = personal_org_slug(user_id, &user_email);
            self.create_org_for(user_id, &slug, &format!("{display}'s Org"))
                .await?;
            orgs = self.list_user_orgs(user_id).await?;
        }

        Ok(ProvisionedUser {
            user_id,
            email: user_email,
            display_name: display,
            orgs,
        })
    }

    /// Every org the user is a member of, with their role — for the org switcher. Uses the
    /// user-scoped RLS path (sees only the caller's own memberships; zero document rows).
    pub async fn list_user_orgs(&self, user_id: Uuid) -> Result<Vec<UserOrg>> {
        let mut tx = self.begin_user_scoped(user_id).await.map_err(map_db)?;
        let rows: Vec<(Uuid, String, String, String)> = sqlx::query_as(
            "SELECT o.id, o.slug, o.name, m.role
             FROM organizations o JOIN organization_members m ON m.org_id = o.id
             WHERE m.user_id = current_user_id() AND o.deleted_at IS NULL
             ORDER BY o.created_at",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        rows.into_iter()
            .map(|(id, slug, name, role)| {
                Ok(UserOrg {
                    id,
                    slug,
                    name,
                    role: OrgRole::from_db(&role)?,
                })
            })
            .collect()
    }

    /// Resolve a web session (already-verified user id) + a chosen org into an [`AuthContext`].
    /// `org_override` (the `X-Org-Id` selection) must be one the user belongs to; otherwise the
    /// first org is used. Errors if the user has no orgs.
    pub async fn authenticate_session(
        &self,
        user_id: Uuid,
        org_override: Option<Uuid>,
    ) -> Result<AuthContext> {
        let orgs = self.list_user_orgs(user_id).await?;
        let target = match org_override {
            Some(o) => orgs.iter().find(|x| x.id == o).ok_or(Error::Forbidden)?,
            None => orgs.first().ok_or(Error::Forbidden)?,
        };
        Ok(AuthContext {
            org_id: target.id,
            user_id,
            actor_type: ActorType::User,
            org_role: target.role,
        })
    }

    /// Create a new organization, making the caller its owner. Any authenticated user may do
    /// this (they own what they create).
    pub async fn create_org(
        &self,
        ctx: &AuthContext,
        slug: &str,
        name: &str,
    ) -> Result<Organization> {
        self.create_org_for(ctx.user_id, slug, name).await
    }

    async fn create_org_for(&self, user_id: Uuid, slug: &str, name: &str) -> Result<Organization> {
        validate::validate_slug(slug)?;
        if name.trim().is_empty() {
            return Err(Error::invalid("organization name is required"));
        }
        let org_id = ids::new_id();
        // Scope to the NEW org so the RLS WITH CHECK (org_id = current_org_id) passes.
        let mut tx = self
            .begin_scoped(org_id, user_id, ActorType::User)
            .await
            .map_err(map_db)?;
        let row = sqlx::query_as::<_, OrgRow>(
            "INSERT INTO organizations (id, slug, name) VALUES ($1,$2,$3)
             RETURNING id, slug, name, created_at",
        )
        .bind(org_id)
        .bind(slug)
        .bind(name.trim())
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db)?;
        sqlx::query(
            "INSERT INTO organization_members (org_id, user_id, role) VALUES ($1,$2,'owner')",
        )
        .bind(org_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(row.into())
    }

    /// Invite a teammate by email (owner/admin only). Returns the invitation plus a one-time
    /// token for the accept link; the token is never stored or shown again.
    pub async fn create_invitation(
        &self,
        ctx: &AuthContext,
        email: &str,
        role: OrgRole,
    ) -> Result<InvitationCreated> {
        rbac::require_admin(ctx)?;
        let email = email.trim();
        if email.is_empty() {
            return Err(Error::invalid("email is required"));
        }
        if matches!(role, OrgRole::Owner) {
            return Err(Error::invalid("cannot invite someone as owner"));
        }
        let id = ids::new_id();
        let token = crypto::generate_token("inv");
        let token_hash = crypto::hash_token(&self.pepper, &token.secret);
        let row = sqlx::query_as::<_, InvitationRow>(
            "INSERT INTO org_invitations
               (id, org_id, email, role, token_hash, token_prefix, invited_by, expires_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7, now() + interval '14 days')
             RETURNING id, org_id, email, role, created_at, expires_at",
        )
        .bind(id)
        .bind(ctx.org_id)
        .bind(email)
        .bind(role.as_str())
        .bind(&token_hash)
        .bind(&token.prefix)
        .bind(ctx.user_id)
        .fetch_one(self.pool())
        .await
        .map_err(map_db)?;
        Ok(InvitationCreated {
            invitation: row.into_core()?,
            token: token.secret,
        })
    }

    /// Pending (unaccepted, unrevoked) invitations for the caller's org (owner/admin only).
    pub async fn list_invitations(&self, ctx: &AuthContext) -> Result<Vec<Invitation>> {
        rbac::require_admin(ctx)?;
        let rows = sqlx::query_as::<_, InvitationRow>(
            "SELECT id, org_id, email, role, created_at, expires_at FROM org_invitations
             WHERE org_id = $1 AND accepted_at IS NULL AND revoked_at IS NULL
             ORDER BY created_at DESC",
        )
        .bind(ctx.org_id)
        .fetch_all(self.pool())
        .await
        .map_err(map_db)?;
        rows.into_iter().map(InvitationRow::into_core).collect()
    }

    /// Revoke a pending invitation in the caller's org (owner/admin only).
    pub async fn revoke_invitation(&self, ctx: &AuthContext, invite_id: Uuid) -> Result<()> {
        rbac::require_admin(ctx)?;
        let n = sqlx::query(
            "UPDATE org_invitations SET revoked_at = now()
             WHERE id = $1 AND org_id = $2 AND revoked_at IS NULL AND accepted_at IS NULL",
        )
        .bind(invite_id)
        .bind(ctx.org_id)
        .execute(self.pool())
        .await
        .map_err(map_db)?
        .rows_affected();
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    /// Add the user to every org with a live invitation for their (verified) email, then mark
    /// those invitations accepted. Internal to provisioning.
    async fn accept_invitations_for(&self, user_id: Uuid, email: &str) -> Result<()> {
        let invites: Vec<(Uuid, Uuid, String)> = sqlx::query_as(
            "SELECT id, org_id, role FROM org_invitations
             WHERE lower(email) = lower($1) AND accepted_at IS NULL AND revoked_at IS NULL
               AND (expires_at IS NULL OR expires_at > now())",
        )
        .bind(email)
        .fetch_all(self.pool())
        .await
        .map_err(map_db)?;

        for (inv_id, org_id, role) in invites {
            // Add membership under the invite's org scope (RLS WITH CHECK).
            let mut tx = self
                .begin_scoped(org_id, user_id, ActorType::User)
                .await
                .map_err(map_db)?;
            sqlx::query(
                "INSERT INTO organization_members (org_id, user_id, role) VALUES ($1,$2,$3)
                 ON CONFLICT (org_id, user_id) DO NOTHING",
            )
            .bind(org_id)
            .bind(user_id)
            .bind(&role)
            .execute(&mut *tx)
            .await
            .map_err(map_db)?;
            tx.commit().await.map_err(map_db)?;

            sqlx::query("UPDATE org_invitations SET accepted_at = now() WHERE id = $1")
                .bind(inv_id)
                .execute(self.pool())
                .await
                .map_err(map_db)?;
        }
        Ok(())
    }

    /// Accept an invitation via its one-time link token (the link IS the authorization — any
    /// signed-in user who opens it joins the org with the invited role). Single-use. Returns
    /// the org so the caller can switch into it.
    pub async fn accept_invitation_by_token(
        &self,
        user_id: Uuid,
        token: &str,
    ) -> Result<Organization> {
        let prefix = crypto::token_prefix("inv", token).ok_or(Error::NotFound)?;
        let found: Option<(Uuid, Uuid, String, String)> = sqlx::query_as(
            "SELECT id, org_id, role, token_hash FROM org_invitations
             WHERE token_prefix = $1 AND accepted_at IS NULL AND revoked_at IS NULL
               AND (expires_at IS NULL OR expires_at > now())",
        )
        .bind(&prefix)
        .fetch_optional(self.pool())
        .await
        .map_err(map_db)?;
        let (inv_id, org_id, role, hash) = found.ok_or(Error::NotFound)?;
        if !crypto::verify_token(&self.pepper, token, &hash) {
            return Err(Error::NotFound);
        }

        let mut tx = self
            .begin_scoped(org_id, user_id, ActorType::User)
            .await
            .map_err(map_db)?;
        sqlx::query(
            "INSERT INTO organization_members (org_id, user_id, role) VALUES ($1, $2, $3)
             ON CONFLICT (org_id, user_id) DO NOTHING",
        )
        .bind(org_id)
        .bind(user_id)
        .bind(&role)
        .execute(&mut *tx)
        .await
        .map_err(map_db)?;
        let org = sqlx::query_as::<_, OrgRow>(
            "SELECT id, slug, name, created_at FROM organizations WHERE id = $1",
        )
        .bind(org_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;

        sqlx::query("UPDATE org_invitations SET accepted_at = now() WHERE id = $1")
            .bind(inv_id)
            .execute(self.pool())
            .await
            .map_err(map_db)?;
        Ok(org.into())
    }

    /// List the members of the caller's org (any member may view the roster).
    pub async fn list_members(&self, ctx: &AuthContext) -> Result<Vec<OrgMember>> {
        rbac::require_read(ctx)?;
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let rows: Vec<(Uuid, String, String, String)> = sqlx::query_as(
            "SELECT u.id, u.email, u.display_name, m.role
             FROM organization_members m JOIN users u ON u.id = m.user_id
             WHERE m.org_id = $1
             ORDER BY CASE m.role WHEN 'owner' THEN 0 WHEN 'admin' THEN 1 WHEN 'member' THEN 2
                                  ELSE 3 END, lower(u.display_name)",
        )
        .bind(ctx.org_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        rows.into_iter()
            .map(|(user_id, email, display_name, role)| {
                Ok(OrgMember {
                    user_id,
                    email,
                    display_name,
                    role: OrgRole::from_db(&role)?,
                })
            })
            .collect()
    }

    /// Change a member's role (owner/admin only). Only an owner may grant/modify the `owner`
    /// role, and the org must keep at least one owner.
    pub async fn update_member_role(
        &self,
        ctx: &AuthContext,
        target: Uuid,
        new_role: OrgRole,
    ) -> Result<()> {
        rbac::require_admin(ctx)?;
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let current = self.member_role(&mut tx, ctx.org_id, target).await?;
        // Only owners may touch owner rows or hand out the owner role.
        if (matches!(current, OrgRole::Owner) || matches!(new_role, OrgRole::Owner))
            && !matches!(ctx.org_role, OrgRole::Owner)
        {
            return Err(Error::Forbidden);
        }
        // Never drop to zero owners.
        if matches!(current, OrgRole::Owner) && !matches!(new_role, OrgRole::Owner) {
            self.assert_not_last_owner(&mut tx, ctx.org_id).await?;
        }
        if current != new_role {
            sqlx::query(
                "UPDATE organization_members SET role = $1 WHERE org_id = $2 AND user_id = $3",
            )
            .bind(new_role.as_str())
            .bind(ctx.org_id)
            .bind(target)
            .execute(&mut *tx)
            .await
            .map_err(map_db)?;
            audit(
                &mut tx,
                ctx,
                "member.role",
                Some(&target.to_string()),
                serde_json::json!({ "role": new_role.as_str() }),
            )
            .await
            .map_err(map_db)?;
        }
        tx.commit().await.map_err(map_db)?;
        Ok(())
    }

    /// Remove a member (owner/admin only). Only an owner may remove an owner, and never the
    /// last one. Removing a member instantly disables their API keys + connectors here (auth
    /// re-checks membership on every request).
    pub async fn remove_member(&self, ctx: &AuthContext, target: Uuid) -> Result<()> {
        rbac::require_admin(ctx)?;
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let current = self.member_role(&mut tx, ctx.org_id, target).await?;
        if matches!(current, OrgRole::Owner) {
            if !matches!(ctx.org_role, OrgRole::Owner) {
                return Err(Error::Forbidden);
            }
            self.assert_not_last_owner(&mut tx, ctx.org_id).await?;
        }
        sqlx::query("DELETE FROM organization_members WHERE org_id = $1 AND user_id = $2")
            .bind(ctx.org_id)
            .bind(target)
            .execute(&mut *tx)
            .await
            .map_err(map_db)?;
        audit(&mut tx, ctx, "member.remove", Some(&target.to_string()), serde_json::json!({}))
            .await
            .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(())
    }

    async fn member_role(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        org_id: Uuid,
        user_id: Uuid,
    ) -> Result<OrgRole> {
        let role: Option<String> = sqlx::query_scalar(
            "SELECT role FROM organization_members WHERE org_id = $1 AND user_id = $2",
        )
        .bind(org_id)
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(map_db)?;
        OrgRole::from_db(&role.ok_or(Error::NotFound)?)
    }

    async fn assert_not_last_owner(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        org_id: Uuid,
    ) -> Result<()> {
        let owners: i64 =
            sqlx::query_scalar("SELECT count(*) FROM organization_members WHERE org_id = $1 AND role = 'owner'")
                .bind(org_id)
                .fetch_one(&mut **tx)
                .await
                .map_err(map_db)?;
        if owners <= 1 {
            return Err(Error::invalid("the organization must keep at least one owner"));
        }
        Ok(())
    }
}
