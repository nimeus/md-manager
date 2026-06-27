//! RBAC: the permission checks every surface enforces.
//!
//! MVP scope: effective document capability is derived from the caller's **org role**
//! (the org `Viewer` ceiling is inherent in the mapping). The schema also carries
//! project/team/per-doc grants (see `docs/PLAN.md` §3); when those are wired in, only
//! [`effective_role`] changes — the `require_*` helpers stay the same.

use crate::error::{Error, Result};
use crate::model::{AuthContext, Role};

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
}
