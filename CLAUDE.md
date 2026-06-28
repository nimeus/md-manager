# CLAUDE.md — md-manager

Guidance for Claude Code working in this repo. Keep current as the project evolves.

## What this is
Multi-tenant SaaS for managing & sharing **markdown/text docs that live ONLY in Postgres** (never as files), usable by both humans (web UI, later) and AI agents (MCP server + CLI) under identical rules.

- **Plan:** [docs/PLAN.md](docs/PLAN.md) · **Tracker:** [TODO.md](TODO.md)
- **Status:** ✅ Phase 1 MVP · ✅ Phase 2 remote MCP + OAuth JWT · ✅ Phase 3 web app code (`frontend/`, Next.js BFF — **not built here**: npm unreachable) · ✅ Phase 4 semantic search (pgvector + env-driven embeddings, **OpenRouter default** — [docs/embeddings.md](docs/embeddings.md); needs a real key + `CREATE EXTENSION vector`). Web-connector go-live needs Logto — [docs/oauth-logto.md](docs/oauth-logto.md).

## Stack
Rust cargo workspace · Postgres 17 (local via Homebrew; no Docker) · Next.js (Phase 3) · self-hosted Logto (Phase 2).

## Workspace layout
```
crates/core    (mdm-core)   domain models, role lattice + RBAC, 3-way merge, md chunker, validation, crypto — pure, no DB/framework
crates/db      (mdm-db)     SQLx service + migrations + TenantDb (RLS session). Runtime-checked queries (no query! macros)
crates/config  (mdm-config) figment config, Secret newtype, tracing
crates/client  (mdm-client) async reqwest client for the API, shared by mcp + cli
crates/embed   (mdm-embed)  OpenAI-compatible embeddings client (OpenRouter default), all env-driven
apps/api       (mdm-api)    Axum HTTP API: REST (v1) + remote MCP at POST /mcp + OAuth discovery + JWT validation (oauth.rs, mcp.rs). Bin: mdm-api
apps/mcp       (mdm-mcp)    stdio MCP server — JSON-RPC 2.0, dispatches via HTTP client. Bin: mdm-mcp
                            (tool schemas shared via mdm_core::mcp; api/mcp dispatch to db, stdio dispatches to the API)
apps/cli       (mdm-cli)    clap CLI. Bin: `mdm`
migrations/                 sqlx migrations (0001_init … 0008_web_auth: Google sign-in, multi-org sessions, invitations)
frontend/                   Next.js 15 web app (BFF over the API; httpOnly cookie holds the key). Build with npm.
```

**Network note:** in the authoring sandbox, cargo/crates.io + GitHub work but **npm registry is unreachable** — so the Rust side is fully built+tested here; `frontend/` is authored but must be `npm install`ed elsewhere.
Layering: `core` (pure) ← `db` (SQL/txn, calls core) ← `api` (HTTP). `cli`/`mcp` → `mdm-client` → HTTP API.
All business rules live in `db`'s service (used by `api`) — `cli`/`mcp` go through the API, so every surface enforces the same rules.

## Non-negotiable conventions
1. **Rules live in `core` + `db`'s service.** `api` handlers are thin; `cli`/`mcp` call the API. Never duplicate validation/RBAC/versioning.
2. **Tenant isolation = Postgres RLS.** App connects as `md_app` (non-owner, `NOBYPASSRLS`). Tenant tables have `org_id` + `FORCE ROW LEVEL SECURITY` keyed on `current_org_id()`. All tenant access goes through `Db::begin_ctx`/`begin_scoped` (sets `app.current_org_id` GUC, `is_local`). Unset GUC ⇒ zero rows. `users`, `api_keys`, and `share_links` are intentionally RLS-exempt (global identity / cross-org token lookup before an org is known) — scope them by explicit `org_id` filter in management code; the public share resolve reads the linked doc under the link's org scope.
3. **Docs = immutable UUID + mutable path** (unique per project). UUID is canonical.
4. **Concurrency = optimistic.** `update_document` needs `expected_version`; stale ⇒ `UpdateOutcome::Conflict { current_version, current_content, base_content }` (HTTP 409). `append` is FOR-UPDATE serialised. No CRDT yet.
4b. **Doc authorization = the RBAC lattice.** Document ops authorize against `Db::effective_doc_role` (org base + project/team/per-doc grants), resolved by `mdm_core::rbac::resolve_doc_role`: per-doc deny (`role='none'`) vetoes unless org owner/admin; positive grants accumulate most-permissive; org viewer is a hard ceiling. `users`/`api_keys` stay RLS-exempt; grant tables are RLS-scoped. (List/search still filter at org level — per-doc deny is enforced on access, not yet hidden from listings.)
5. **Versioning = full snapshots** with `version_kind` (checkpoint vs autosave) + ~30s autosave coalesce. Max doc size capped.
6. **Secrets** via `mdm_config::Secret`; never log. API keys/share tokens hashed HMAC-SHA256 + server pepper, constant-time compare. Keys shown once.
7. **CLI and MCP tool surfaces stay symmetric.**
8. SQLx uses **runtime-checked** queries (no `query!`), so the build never needs a live DB.

## Local dev setup
```bash
# 1) Postgres 17 (Homebrew) + roles + dev/test DBs (idempotent)
brew services start postgresql@17
bash scripts/db-setup.sh

# 2) run the API (loads MDM_* env; see .env.example)
set -a; source .env.example; set +a       # or export your own
cargo run -p mdm-api                        # listens on MDM_API_ADDR (default 127.0.0.1:8080)

# 3) bootstrap a tenant + key, then use the CLI
cargo run -p mdm-cli -- bootstrap --email me@x.com --name Me --org-slug acme --org-name Acme --token "$MDM_ADMIN_BOOTSTRAP_TOKEN" --save
cargo run -p mdm-cli -- whoami
cargo run -p mdm-cli -- doc create --project <slug> --path notes/x --title X -m "# X"
cargo run -p mdm-cli -- search hello

# 4) MCP server (for an agent host): MDM_API_URL + MDM_API_KEY env, speaks stdio
MDM_API_KEY=mk_... cargo run -p mdm-mcp
```
Two DB roles: `md_owner` (owns tables, runs migrations) and `md_app` (runtime, NOBYPASSRLS). The API runs migrations as `md_owner` on startup, then serves as `md_app`.

## Commands
```bash
cargo build                       # build all
cargo test                        # unit (core) + integration (db, needs Postgres running)
cargo clippy --all-targets
cargo run -p mdm-api              # API server
cargo run -p mdm-mcp              # MCP stdio server
cargo run -p mdm-cli -- --help    # CLI (binary `mdm`)
```

## Env vars
API (`mdm-api`): `MDM_DATABASE_URL`, `MDM_MIGRATION_DATABASE_URL`, `MDM_API_KEY_PEPPER`, `MDM_ADMIN_BOOTSTRAP_TOKEN`, `MDM_API_ADDR`, `MDM_LOG_FORMAT`, `MDM_DB_MAX_CONNECTIONS`, `MDM_MAX_DOC_BYTES`, `MDM_AUTOSAVE_DEBOUNCE_SECS`. Web sign-in: `MDM_GOOGLE_CLIENT_ID` (enables `POST /v1/auth/google`), `MDM_SESSION_SECRET`, `MDM_SESSION_TTL_SECS`.
CLI/MCP: `MDM_API_URL`, `MDM_API_KEY`, `MDM_BOOTSTRAP_TOKEN`. CLI also reads `~/.config/md-manager/config.json` (`mdm auth login`).

## Notes / known follow-ups
- MCP is a hand-rolled stdio JSON-RPC server (robust, spec-compliant). Phase 2 adds Streamable HTTP + OAuth; could adopt `rmcp` then.
- Categories (migration 0003) and Teams + per-project/per-doc grants + the full RBAC lattice (migration 0004) are shipped across db/REST/CLI (grants not in MCP — agents don't manage ACLs). Rate limiting deferred; per-doc-deny not yet hidden from list/search; CI workflow not yet added.
- When a decision changes, update docs/PLAN.md + this file in the same change.
