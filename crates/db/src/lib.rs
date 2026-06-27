//! `mdm-db` — SQLx-backed implementations of the `mdm-core` repository traits,
//! embedded migrations, and the **`TenantDb`** RLS session wrapper.
//!
//! Hard rule (see `CLAUDE.md`): all database access goes through `TenantDb`, which
//! opens a transaction and sets `app.current_org_id` (+ actor) via `set_config(_, _, true)`
//! so Postgres row-level security scopes every query to one organization. Repositories
//! accept `&mut TenantDb`, never a bare pool connection.
//!
//! Phase 0 skeleton — `PgPool`, `TenantDb`, and the first migration land per `TODO.md`.

// Intentionally empty until the database layer is implemented.
