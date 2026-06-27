//! md-manager CLI — binary `mdm`. Phase 0 placeholder.
//!
//! HTTP client over the API only (never touches the DB directly, so RLS/RBAC always apply).
//! Command tree (auth/config/org/proj/doc/tag/cat/search) lands in Phase 1; `mdm doc get`
//! prints raw markdown to stdout so agents can pipe it straight into context.

fn main() {
    println!("mdm: scaffolding only — commands land in Phase 1 (see TODO.md).");
}
