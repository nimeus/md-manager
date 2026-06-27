//! RBAC resolver.
//!
//! The full resolver (see `docs/PLAN.md` §3) computes an effective document role from
//! the org base role, project grants, and per-doc grants, with two overrides: an
//! explicit per-doc `None` grant vetoes (unless the user is org owner/admin), and an
//! org `Viewer` is a hard ceiling. `mdm-db` mirrors this as a single SQL CTE.
//!
//! This Phase 0 stub implements only most-permissive accumulation; the veto and
//! ceiling rules land with the membership/grants schema (see `TODO.md`).

use crate::model::Role;

/// Combine positive grants by most-permissive (lattice `max`).
pub fn effective_role(grants: &[Role]) -> Role {
    grants.iter().copied().max().unwrap_or(Role::None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn most_permissive_grant_wins() {
        assert_eq!(effective_role(&[Role::Viewer, Role::Editor]), Role::Editor);
        assert_eq!(effective_role(&[Role::Commenter, Role::Viewer]), Role::Commenter);
    }

    #[test]
    fn no_grants_is_none() {
        assert_eq!(effective_role(&[]), Role::None);
    }
}
