# CLAUDE.md — md-manager

Guidance for Claude Code working in this repo. Keep this file current as the project evolves.

## What this is
Multi-tenant SaaS for managing & sharing **markdown/text docs that live ONLY in Postgres** (never as files), used as memory/helper docs by **both humans and AI agents**. Humans edit via a web UI; agents read/write via an **MCP server** and a **CLI** — all enforcing identical rules.

- **Full plan:** [docs/PLAN.md](docs/PLAN.md) · **Roadmap/tracker:** [TODO.md](TODO.md)
- **Current phase:** Phase 0 (scaffolding). Phase 1 = agent-surface MVP (backend + MCP + CLI).

## Stack
Rust backend (cargo workspace) · Next.js 15 frontend (fast-follow) · Postgres 18 · self-hosted Logto (OAuth 2.1 auth server).

## Workspace layout
```
crates/core    (mdm-core)   domain models, repo TRAITS, RBAC resolver, errors  — no framework/DB deps
crates/db      (mdm-db)     PgPool, SQLx repo impls, migrations, TenantDb (RLS)
crates/config  (mdm-config) figment config, secrecy, tracing
apps/api       (mdm-api)    Axum HTTP API (web BFF target + REST for CLI)
apps/mcp       (mdm-mcp)    rmcp MCP server: stdio + Streamable HTTP
apps/cli       (mdm-cli)    clap CLI, binary `mdm`, HTTP-only
frontend/                   Next.js app (Phase 3)
migrations/                 sqlx migrations
```

## Non-negotiable conventions (enforce in every change)
1. **Business rules live in `core`.** `api`/`mcp`/`cli` are thin transport wiring over the same `core` services + repo traits — never duplicate validation/RBAC/versioning logic in a binary.
2. **Tenant isolation = Postgres RLS.** App connects as `md_app` (non-owner, `NOBYPASSRLS`). Every tenant table has `org_id NOT NULL` + `FORCE ROW LEVEL SECURITY`. All DB access goes through **`TenantDb`** (a txn that `set_config('app.current_org_id', …, true)`); repos take `&mut TenantDb`, never a bare pool conn. An unset GUC must yield zero rows (fail closed).
3. **Docs are addressed by immutable UUID + mutable path/slug** (unique per project). UUID is canonical for links/MCP/agent refs.
4. **Concurrency = optimistic.** `update_doc` needs `expected_version`; stale → 409 with `current_version` + `current_content` + `base_content`. `append_to_doc` is an atomic DB-side concat under row lock. No CRDT until Phase 5.
5. **Versioning = full snapshots** with `version_kind` (checkpoint vs autosave) + debounce. Max doc size capped (~1 MB).
6. **Secrets** via `secrecy::SecretString` + figment; never log them. API keys/share tokens hashed with HMAC-SHA256 + server pepper, constant-time compare.
7. **Logto issues tokens; `api`/`mcp` only validate** (JWKS + `aud`/`iss`/`exp`). MCP server never forwards a client token upstream.
8. **CLI and MCP tool surfaces stay symmetric.**
9. Commit the **`.sqlx` offline cache** so CI builds without a live DB.

## Common commands
```bash
cargo build                 # build workspace
cargo test                  # run tests
cargo clippy --all-targets  # lint
cargo run -p mdm-api        # run API
cargo run -p mdm-mcp        # run MCP server
cargo run -p mdm-cli -- --help   # CLI (binary `mdm`)
# DB (needs Docker — not yet installed locally):
docker compose up -d postgres logto
sqlx migrate run            # apply migrations (owner role)
cargo sqlx prepare --workspace   # refresh .sqlx offline cache
```

## Environment notes
- Toolchain: Rust 1.96, Node 25. **Docker is NOT installed** — install it (or Postgres/Logto another way) before running the DB-backed pieces.
- Postgres connects two roles: an **owner/migrator** (runs migrations) and **`md_app`** (app runtime, NOBYPASSRLS).

## How to work here
Track progress in [TODO.md](TODO.md) (update checkboxes) and keep [docs/PLAN.md](docs/PLAN.md) authoritative for decisions. When a decision changes, update PLAN.md + this file in the same change.
