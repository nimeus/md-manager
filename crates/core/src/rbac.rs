//! RBAC: the permission checks every surface enforces.
//!
//! MVP scope: effective document capability is derived from the caller's **org role**
//! (the org `Viewer` ceiling is inherent in the mapping). The schema also carries
//! project/team/per-doc grants (see `docs/PLAN.md` §3); when those are wired in, only
//! [`effective_role`] changes — the `require_*` helpers stay the same.

use crate::error::{Error, Result};
use crate::model::{AuthContext, OrgRole, Role};

/// Combine positive grants by most-permissive (lattice `max`).
pub fn most_permissive(grants: &[Role]) -> Role {
    grants.iter().copied().max().unwrap_or(Role::None)
}

/// The caller's effective capability on documents in their current org.
pub fn effective_role(ctx: &AuthContext) -> Role {
    ctx.org_role.capability()
}

fn require(have: Role, need: Role) -> Result<()> {
    if have >= need {
        Ok(())
    } else {
        Err(Error::Forbidden)
    }
}

/// Read documents / list / search.
pub fn require_read(ctx: &AuthContext) -> Result<()> {
    require(effective_role(ctx), Role::Viewer)
}

/// Create / edit / append / delete documents.
pub fn require_write(ctx: &AuthContext) -> Result<()> {
    require(effective_role(ctx), Role::Editor)
}

/// Manage the org: projects, members, API keys.
pub fn require_admin(ctx: &AuthContext) -> Result<()> {
    require(effective_role(ctx), Role::Admin)
}

/// Inputs for resolving a caller's effective role on a specific document, layering
/// project/team/per-doc grants on top of the org base role.
pub struct DocAccess {
    pub org_role: OrgRole,
    /// Positive grant roles (from project + doc grants) applicable to the user or a team
    /// they belong to.
    pub grant_roles: Vec<Role>,
    /// True if any applicable per-doc grant is an explicit deny (role `none`).
    pub denied: bool,
}

/// Resolve the effective document role (see `docs/PLAN.md` §3):
/// 1. an explicit per-doc deny vetoes — unless the caller is org owner/admin;
/// 2. otherwise take the most-permissive of the org base capability and all positive grants;
/// 3. an org `viewer` is a hard ceiling.
pub fn resolve_doc_role(access: &DocAccess) -> Role {
    let privileged = matches!(access.org_role, OrgRole::Owner | OrgRole::Admin);
    if access.denied && !privileged {
        return Role::None;
    }
    let base = access.org_role.capability();
    let mut effective = access.grant_roles.iter().copied().fold(base, Role::max);
    if matches!(access.org_role, OrgRole::Viewer) && effective > Role::Viewer {
        effective = Role::Viewer;
    }
    effective
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ActorType, OrgRole};
    use uuid::Uuid;

    fn ctx(role: OrgRole) -> AuthContext {
        AuthContext {
            org_id: Uuid::nil(),
            user_id: Uuid::nil(),
            actor_type: ActorType::User,
            org_role: role,
        }
    }

    #[test]
    fn most_permissive_wins() {
        assert_eq!(most_permissive(&[Role::Viewer, Role::Editor]), Role::Editor);
        assert_eq!(most_permissive(&[]), Role::None);
    }

    #[test]
    fn viewer_can_read_but_not_write() {
        assert!(require_read(&ctx(OrgRole::Viewer)).is_ok());
        assert!(require_write(&ctx(OrgRole::Viewer)).is_err());
        assert!(require_admin(&ctx(OrgRole::Viewer)).is_err());
    }

    #[test]
    fn member_can_write_but_not_admin() {
        assert!(require_write(&ctx(OrgRole::Member)).is_ok());
        assert!(require_admin(&ctx(OrgRole::Member)).is_err());
    }

    #[test]
    fn admin_and_owner_can_admin() {
        assert!(require_admin(&ctx(OrgRole::Admin)).is_ok());
        assert!(require_admin(&ctx(OrgRole::Owner)).is_ok());
    }

    fn access(org_role: OrgRole, grant_roles: Vec<Role>, denied: bool) -> DocAccess {
        DocAccess { org_role, grant_roles, denied }
    }

    #[test]
    fn member_base_is_editor() {
        assert_eq!(resolve_doc_role(&access(OrgRole::Member, vec![], false)), Role::Editor);
    }

    #[test]
    fn per_doc_deny_locks_out_a_member() {
        assert_eq!(resolve_doc_role(&access(OrgRole::Member, vec![], true)), Role::None);
    }

    #[test]
    fn owner_and_admin_override_deny() {
        assert_eq!(resolve_doc_role(&access(OrgRole::Owner, vec![], true)), Role::Admin);
        assert_eq!(resolve_doc_role(&access(OrgRole::Admin, vec![], true)), Role::Admin);
    }

    #[test]
    fn grants_elevate_most_permissive() {
        assert_eq!(
            resolve_doc_role(&access(OrgRole::Member, vec![Role::Admin], false)),
            Role::Admin
        );
    }

    #[test]
    fn org_viewer_is_a_hard_ceiling() {
        // even a grant of editor cannot lift an org viewer above viewer
        assert_eq!(
            resolve_doc_role(&access(OrgRole::Viewer, vec![Role::Editor], false)),
            Role::Viewer
        );
    }
}
