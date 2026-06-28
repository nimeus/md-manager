//! The access-control layer: teams, project/document grants, and the per-document
//! effective-role resolution that document operations authorize against.

use mdm_core::model::{AuthContext, Role, Team};
use mdm_core::rbac::{self, DocAccess};
use mdm_core::{Error, Result, ids, validate};
use serde_json::json;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::rows::TeamRow;
use crate::{Db, audit, map_db};

type Tx<'a> = Transaction<'a, Postgres>;

impl Db {
    // --- effective role resolution (called inside a tenant transaction) ------

    async fn team_ids(&self, tx: &mut Tx<'_>, user_id: Uuid) -> Result<Vec<Uuid>> {
        sqlx::query_scalar("SELECT team_id FROM team_members WHERE user_id = $1")
            .bind(user_id)
            .fetch_all(&mut **tx)
            .await
            .map_err(map_db)
    }

    /// Effective role on a document = org base + project/team/doc grants, with deny-veto
    /// and the org-viewer ceiling (see `mdm_core::rbac::resolve_doc_role`).
    pub(crate) async fn effective_doc_role(
        &self,
        tx: &mut Tx<'_>,
        ctx: &AuthContext,
        doc_id: Uuid,
        project_id: Uuid,
    ) -> Result<Role> {
        let teams = self.team_ids(tx, ctx.user_id).await?;

        let role_strs: Vec<String> = sqlx::query_scalar(
            "SELECT role FROM project_grants
               WHERE project_id = $1 AND role <> 'none'
                 AND ((subject_type='user' AND subject_id = $2)
                   OR (subject_type='team' AND subject_id = ANY($3)))
             UNION ALL
             SELECT role FROM document_grants
               WHERE document_id = $4 AND role <> 'none'
                 AND ((subject_type='user' AND subject_id = $2)
                   OR (subject_type='team' AND subject_id = ANY($3)))",
        )
        .bind(project_id)
        .bind(ctx.user_id)
        .bind(&teams)
        .bind(doc_id)
        .fetch_all(&mut **tx)
        .await
        .map_err(map_db)?;

        let denied: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM document_grants
               WHERE document_id = $1 AND role = 'none'
                 AND ((subject_type='user' AND subject_id = $2)
                   OR (subject_type='team' AND subject_id = ANY($3))))",
        )
        .bind(doc_id)
        .bind(ctx.user_id)
        .bind(&teams)
        .fetch_one(&mut **tx)
        .await
        .map_err(map_db)?;

        let mut grant_roles = Vec::with_capacity(role_strs.len());
        for s in role_strs {
            grant_roles.push(Role::from_db(&s)?);
        }
        Ok(rbac::resolve_doc_role(&DocAccess { org_role: ctx.org_role, grant_roles, denied }))
    }

    /// Effective role at the project level (for creating documents — there's no doc yet).
    pub(crate) async fn effective_project_role(
        &self,
        tx: &mut Tx<'_>,
        ctx: &AuthContext,
        project_id: Uuid,
    ) -> Result<Role> {
        let teams = self.team_ids(tx, ctx.user_id).await?;
        let role_strs: Vec<String> = sqlx::query_scalar(
            "SELECT role FROM project_grants
               WHERE project_id = $1 AND role <> 'none'
                 AND ((subject_type='user' AND subject_id = $2)
                   OR (subject_type='team' AND subject_id = ANY($3)))",
        )
        .bind(project_id)
        .bind(ctx.user_id)
        .bind(&teams)
        .fetch_all(&mut **tx)
        .await
        .map_err(map_db)?;
        let mut grant_roles = Vec::with_capacity(role_strs.len());
        for s in role_strs {
            grant_roles.push(Role::from_db(&s)?);
        }
        Ok(rbac::resolve_doc_role(&DocAccess { org_role: ctx.org_role, grant_roles, denied: false }))
    }

    pub(crate) async fn authorize_doc(
        &self,
        tx: &mut Tx<'_>,
        ctx: &AuthContext,
        doc_id: Uuid,
        project_id: Uuid,
        need: Role,
    ) -> Result<()> {
        if self.effective_doc_role(tx, ctx, doc_id, project_id).await? >= need {
            Ok(())
        } else {
            Err(Error::Forbidden)
        }
    }

    pub(crate) async fn authorize_project(
        &self,
        tx: &mut Tx<'_>,
        ctx: &AuthContext,
        project_id: Uuid,
        need: Role,
    ) -> Result<()> {
        if self.effective_project_role(tx, ctx, project_id).await? >= need {
            Ok(())
        } else {
            Err(Error::Forbidden)
        }
    }

    // --- teams ---------------------------------------------------------------

    pub async fn create_team(&self, ctx: &AuthContext, slug: &str, name: &str) -> Result<Team> {
        rbac::require_admin(ctx)?;
        validate::validate_slug(slug)?;
        validate::validate_title(name)?;
        let id = ids::new_id();
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let row = sqlx::query_as::<_, TeamRow>(
            "INSERT INTO teams (id, org_id, slug, name) VALUES ($1,$2,$3,$4)
             RETURNING id, slug, name, created_at",
        )
        .bind(id)
        .bind(ctx.org_id)
        .bind(slug)
        .bind(name)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db)?;
        audit(&mut tx, ctx, "team.create", Some(&id.to_string()), json!({ "slug": slug }))
            .await
            .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(row.into())
    }

    pub async fn list_teams(&self, ctx: &AuthContext) -> Result<Vec<Team>> {
        rbac::require_read(ctx)?;
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        let rows = sqlx::query_as::<_, TeamRow>(
            "SELECT id, slug, name, created_at FROM teams ORDER BY slug",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn add_team_member(
        &self,
        ctx: &AuthContext,
        team_id: Uuid,
        user_id: Uuid,
    ) -> Result<()> {
        rbac::require_admin(ctx)?;
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        // Team must be in this org (RLS-scoped); user must be an org member.
        let team_ok: Option<Uuid> = sqlx::query_scalar("SELECT id FROM teams WHERE id = $1")
            .bind(team_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_db)?;
        if team_ok.is_none() {
            return Err(Error::NotFound);
        }
        let member_ok: Option<Uuid> = sqlx::query_scalar(
            "SELECT user_id FROM organization_members WHERE org_id = $1 AND user_id = $2",
        )
        .bind(ctx.org_id)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db)?;
        if member_ok.is_none() {
            return Err(Error::invalid("user is not a member of this organization"));
        }
        sqlx::query(
            "INSERT INTO team_members (org_id, team_id, user_id) VALUES ($1,$2,$3)
             ON CONFLICT DO NOTHING",
        )
        .bind(ctx.org_id)
        .bind(team_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(map_db)?;
        audit(&mut tx, ctx, "team.add_member", Some(&team_id.to_string()),
              json!({ "user_id": user_id }))
            .await
            .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(())
    }

    // --- grants --------------------------------------------------------------

    pub async fn grant_project(
        &self,
        ctx: &AuthContext,
        project_id: Uuid,
        subject_type: &str,
        subject_id: Uuid,
        role: Role,
    ) -> Result<()> {
        rbac::require_admin(ctx)?;
        validate_subject_type(subject_type)?;
        if role == Role::None {
            return Err(Error::invalid("project grants cannot be a deny (role 'none')"));
        }
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        sqlx::query(
            "INSERT INTO project_grants (id, org_id, project_id, subject_type, subject_id, role)
             VALUES ($1,$2,$3,$4,$5,$6)
             ON CONFLICT (project_id, subject_type, subject_id) DO UPDATE SET role = EXCLUDED.role",
        )
        .bind(ids::new_id())
        .bind(ctx.org_id)
        .bind(project_id)
        .bind(subject_type)
        .bind(subject_id)
        .bind(role.as_str())
        .execute(&mut *tx)
        .await
        .map_err(map_db)?;
        audit(&mut tx, ctx, "grant.project", Some(&project_id.to_string()),
              json!({ "subject_type": subject_type, "subject_id": subject_id, "role": role.as_str() }))
            .await
            .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(())
    }

    /// Grant (or deny, with `role = None`) a user/team on a document.
    pub async fn grant_document(
        &self,
        ctx: &AuthContext,
        doc_id: Uuid,
        subject_type: &str,
        subject_id: Uuid,
        role: Role,
    ) -> Result<()> {
        rbac::require_admin(ctx)?;
        validate_subject_type(subject_type)?;
        let mut tx = self.begin_ctx(ctx).await.map_err(map_db)?;
        sqlx::query(
            "INSERT INTO document_grants (id, org_id, document_id, subject_type, subject_id, role)
             VALUES ($1,$2,$3,$4,$5,$6)
             ON CONFLICT (document_id, subject_type, subject_id) DO UPDATE SET role = EXCLUDED.role",
        )
        .bind(ids::new_id())
        .bind(ctx.org_id)
        .bind(doc_id)
        .bind(subject_type)
        .bind(subject_id)
        .bind(role.as_str())
        .execute(&mut *tx)
        .await
        .map_err(map_db)?;
        audit(&mut tx, ctx, "grant.document", Some(&doc_id.to_string()),
              json!({ "subject_type": subject_type, "subject_id": subject_id, "role": role.as_str() }))
            .await
            .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(())
    }
}

fn validate_subject_type(s: &str) -> Result<()> {
    match s {
        "user" | "team" => Ok(()),
        _ => Err(Error::invalid("subject_type must be 'user' or 'team'")),
    }
}
