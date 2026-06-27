# md-manager — TODO / Roadmap Tracker

> Living checklist. Update statuses as work lands. Full rationale in [docs/PLAN.md](docs/PLAN.md).
> Legend: `[ ]` todo · `[~]` in progress · `[x]` done

**Current status (2026-06-27):** ✅ **Phase 1 MVP complete & verified** — the agent surface works end to end.
A solo dev or team can manage Postgres-stored markdown via the HTTP API, the `mdm` CLI, and the
MCP server (stdio), with tenant isolation (RLS), full-snapshot versioning, optimistic concurrency
(409 + 3-way merge data), atomic append, keyword FTS, tags, audit log, and API-key auth.

**Local dev:** Postgres 17 via Homebrew (no Docker). Run `bash scripts/db-setup.sh` once, then
`cargo run -p mdm-api` (see `.env.example` / [CLAUDE.md](CLAUDE.md)).

**Next up:** Phase 2 — self-hosted Logto + remote MCP (Streamable HTTP) + OAuth 2.1 resource
server so Claude.ai / ChatGPT connectors work; then Phase 3 web app.

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
- [ ] Rate limits / quotas (`tower-governor`, max docs/project)   ← deferred polish
- [ ] Per-team / per-doc grants + categories (schema-ready; service uses org role for now)

## Phase 2 — Web connectors (completes launch)
- [ ] Self-hosted Logto configured (orgs, scopes, resource indicator, DCR)
- [ ] `mcp` Streamable HTTP transport
- [ ] OAuth resource server: PRM endpoint, JWKS cache, `aud`/`iss`/`exp` validation
- [ ] Spike: real Claude.ai + ChatGPT connector end-to-end
- [ ] Minimal BFF OAuth callback + cookie session

## Phase 3 — Human web app (Next.js)
- [ ] App shell + Logto BFF auth + org/project switcher
- [ ] Doc tree + CodeMirror 6 editor (inline preview)
- [ ] Version history + restore + conflict/merge UI
- [ ] Tag/category management, search UI + `cmdk`, API-keys screen, share links

## Phase 4 — Semantic search
- [ ] pgvector + `embedding halfvec(1024)` + HNSW + async embedding worker
- [ ] `search_docs` `mode=semantic|hybrid` (RRF)

## Phase 5 — Realtime + scale
- [ ] Yjs/CRDT over Rust websocket; OTel + alerting; version compaction; SSO
