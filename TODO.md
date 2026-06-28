# md-manager — TODO / Roadmap Tracker

> Living checklist. Update statuses as work lands. Full rationale in [docs/PLAN.md](docs/PLAN.md).
> Legend: `[ ]` todo · `[~]` in progress · `[x]` done

**Current status (2026-06-28):** ✅ Phase 1 MVP + ✅ Phase 2 resource server (remote MCP over HTTP +
OAuth 2.1 token validation) complete & verified. Agents reach md-manager via API, `mdm` CLI, MCP
stdio, **and MCP over HTTP** (with an API key today; with Logto-issued OAuth JWTs once Logto is run).

**Local dev:** Postgres 17 via Homebrew (no Docker). `bash scripts/db-setup.sh`, then `cargo run -p mdm-api`.

**Remaining for web connectors (external):** run self-hosted Logto + expose over public HTTPS, then
the live Claude.ai/ChatGPT connector spike. See [docs/oauth-logto.md](docs/oauth-logto.md).

**Phase 3 web app (`frontend/`):** code complete (Next.js 15 BFF). ⚠️ NOT built in this environment —
the npm registry was unreachable (cargo/crates.io works; npm doesn't). Run `npm install && npm run build`
on a machine with npm access; see [frontend/README.md](frontend/README.md).

---

## Phase 0 — Scaffolding ✅
- [x] git init + `.gitignore` + `rust-toolchain.toml`
- [x] Cargo workspace: `crates/{core,db,config,client}` + `apps/{api,mcp,cli}` (binary `mdm`)
- [x] `cargo build` green + tests
- [x] `core`: models, role lattice + RBAC, 3-way merge, header-aware chunker, validation, crypto (14 tests)
- [x] `db`: `Db` + `TenantDb` (RLS GUC session), runtime-checked SQLx service
- [x] migration 0001 (orgs/members/projects/documents/versions/tags/chunks/api_keys/audit) + RLS + FTS
- [x] startup assertion `md_app` is `NOBYPASSRLS`
- [x] `config` crate (figment + Secret + tracing)
- [x] `scripts/db-setup.sh` (roles + dev/test DBs) + `.env.example`  *(replaces docker-compose; Postgres via brew)*
- [ ] CI workflow (build + run migrations on a throwaway DB + tests)   ← still open

## Phase 1 — Agent-surface MVP ✅
- [x] Org/project/membership + RLS/RBAC (owner/admin/member/viewer)
- [x] Documents CRUD: UUID + mutable path, content_hash, current_version
- [x] Full-snapshot versioning + `version_kind` (checkpoint/autosave) + ~30s autosave coalesce
- [x] Optimistic concurrency: stale → 409 with `current` + `base` content for 3-way merge
- [x] Atomic `append` (FOR UPDATE serialised), restore, soft delete/undelete, move, history
- [x] Keyword FTS (generated tsvector + GIN, doc-level aggregation, snippet highlights)
- [x] Tags (org-scoped) + document tagging
- [x] API keys: HMAC+pepper, prefix lookup, mint/list/revoke, creator-role lifecycle binding
- [x] Audit log (writes + key events)
- [x] **HTTP API** (Axum): all endpoints + auth extractor + bootstrap + error mapping
- [x] **MCP server** (stdio JSON-RPC): 15 tools, raw-markdown reads, conflict-aware updates
- [x] **CLI `mdm`**: auth/whoami/org/proj/doc/search/tag/keys; raw-markdown to stdout; stdin/-m/--file body
- [x] `mdm-client`: shared async HTTP client (used by MCP + CLI)
- [x] Integration tests vs Postgres: tenant isolation, concurrency, RBAC, search, key revoke
- [x] End-to-end verified: CLI + MCP agent loops, cross-surface consistency
- [x] **Categories** (org-scoped, hierarchical, cross-project) + document_categories — migration 0003, db, REST, CLI (`mdm cat`), MCP (18 tools total), integration tests
- [ ] Rate limits / quotas (`tower-governor`, max docs/project)   ← deferred polish
- [ ] Teams + per-project/per-doc grants + full RBAC lattice (deny-veto / viewer-ceiling) — resolver still org-role only

## Phase 2 — Web connectors (resource server ✅; Logto go-live external)
- [x] Streamable HTTP `/mcp` transport (served by the API) — 15 tools, verified via curl
- [x] OAuth resource server: PRM endpoint, JWKS cache, RS256 `aud`/`iss`/`exp` validation (3 unit tests)
- [x] Dual auth (API key OR JWT) + `WWW-Authenticate` challenge
- [x] migration 0002 `users.logto_sub` + `authenticate_oauth(sub, org)`
- [x] Shared MCP tool schemas (`mdm_core::mcp`) across stdio + HTTP surfaces
- [x] Logto docs + compose ([docs/oauth-logto.md](docs/oauth-logto.md), docker-compose.logto.yml)
- [ ] Run self-hosted Logto + public HTTPS, configure resource/orgs/DCR — **external** (needs Docker)
- [ ] Spike: real Claude.ai + ChatGPT connector end-to-end — **external go-live**
- [ ] JIT identity/org provisioning + self-serve account linking
- [ ] BFF OAuth callback + cookie session (lands with the Phase 3 web app)

## Phase 3 — Human web app (Next.js) — code complete, build externally
- [x] Next.js 15 App Router + React 19 + Tailwind v4 scaffold (`frontend/`)
- [x] BFF auth: httpOnly session cookie (API key) + middleware guard + login page/action
- [x] Server API client (`lib/api.ts`) + server actions (`lib/actions.ts`)
- [x] App shell + nav; projects list/create; project → documents list/create
- [x] Markdown editor (edit/preview) with **conflict-aware save** (409 → load current / overwrite)
- [x] Version history + restore; document delete; search page; API-keys (mint shown-once / revoke)
- [ ] **Run `npm install && npm run build`** + headless verify (blocked here: npm unreachable)
- [ ] Swap API-key login → Logto OAuth BFF flow
- [ ] CodeMirror 6 editor; tags/categories UI; org/project switcher; `cmdk`; share links

## Phase 4 — Semantic search
- [ ] pgvector + `embedding halfvec(1024)` + HNSW + async embedding worker
- [ ] `search_docs` `mode=semantic|hybrid` (RRF)

## Phase 5 — Realtime + scale
- [ ] Yjs/CRDT over Rust websocket; OTel + alerting; version compaction; SSO
