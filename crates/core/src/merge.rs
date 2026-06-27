//! 3-way merge for concurrent edits.
//!
//! When an `update_doc` arrives with a stale `expected_version`, the surface returns the
//! base (the snapshot the caller started from) and the current content so a client can
//! merge. This helper offers a server-side merge for callers that ask for it.

/// Outcome of a 3-way merge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutcome {
    /// A clean merge with no overlapping changes.
    Clean(String),
    /// Conflicting changes; the string contains Git-style conflict markers.
    Conflict(String),
}

/// Merge `ours` and `theirs` against their common ancestor `base`.
pub fn three_way(base: &str, ours: &str, theirs: &str) -> MergeOutcome {
    match diffy::merge(base, ours, theirs) {
        Ok(merged) => MergeOutcome::Clean(merged),
        Err(conflicted) => MergeOutcome::Conflict(conflicted),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_overlapping_changes_merge_clean() {
        let base = "line1\nline2\nline3\n";
        let ours = "line1 edited\nline2\nline3\n";
        let theirs = "line1\nline2\nline3 edited\n";
        match three_way(base, ours, theirs) {
            MergeOutcome::Clean(s) => {
                assert!(s.contains("line1 edited"));
                assert!(s.contains("line3 edited"));
            }
            MergeOutcome::Conflict(_) => panic!("expected a clean merge"),
        }
    }

    #[test]
    fn overlapping_changes_conflict() {
        let base = "the quick brown fox\n";
        let ours = "the slow brown fox\n";
        let theirs = "the fast brown fox\n";
        assert!(matches!(
            three_way(base, ours, theirs),
            MergeOutcome::Conflict(_)
        ));
    }
}
