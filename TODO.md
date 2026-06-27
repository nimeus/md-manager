# md-manager — TODO / Roadmap Tracker

> Living checklist. Update statuses as work lands. Full rationale in [docs/PLAN.md](docs/PLAN.md).
> Legend: `[ ]` todo · `[~]` in progress · `[x]` done

**Current status (2026-06-27):** Phase 0 — scaffolding the cargo workspace.
**Blockers:** Docker not installed locally (needed to run Postgres + Logto; not needed to build crates).

---

## Phase 0 — Scaffolding
- [~] git init + `.gitignore` + `rust-toolchain.toml`
- [~] Cargo workspace: `crates/{core,db,config}` + `apps/{api,mcp,cli}`
- [ ] `cargo build` green on the skeleton
- [ ] `core`: domain models (org, team, project, document, version, tag, category, api_key)
- [ ] `core`: repository **traits** + `Actor`/`AuthContext` + typed `thiserror` errors
- [ ] `core`: RBAC resolver (lattice none<viewer<commenter<editor<admin, deny veto, viewer ceiling)
- [ ] `core`: header-aware markdown chunker (respect headings, never split fenced code)
- [ ] `db`: `PgPool` + `TenantDb` (txn + `set_config` GUCs) + repo accepts only `&mut TenantDb`
- [ ] `db`: migration 0001 — users, orgs, members, teams, projects, grants, documents, document_versions
- [ ] `db`: RLS policies + `FORCE ROW LEVEL SECURITY` + `md_app` (NOBYPASSRLS) + owner roles
- [ ] `db`: startup assertion `md_app.rolbypassrls = false`
- [ ] `db`: commit `.sqlx` offline cache
- [ ] `config`: figment (TOML+env+profiles) + `secrecy` + tracing init
- [ ] `docker-compose.yml` (Postgres 18 + Logto) + `.env.example`
- [ ] CI: build offline + run migrations on a throwaway DB + `cargo test`/`clippy`

## Phase 1 — Agent-surface MVP
- [ ] Migration: tags, document_tags, categories, document_categories, api_keys, doc_chunks, audit_log
- [ ] Org/team/project/membership services + REST handlers (`api`)
- [ ] Documents CRUD: UUID + mutable path, content_hash, current_version
- [ ] Versioning: full snapshots + `version_kind` + ~30s debounce/coalesce + retention job
- [ ] Optimistic concurrency: `If-Match`/`expected_version` → 409 with base+current content
- [ ] Server-side 3-way merge helper (`merge_strategy = fail|three_way|append_only`)
- [ ] Atomic `append_to_doc` (`UPDATE … content = content || $1 … RETURNING`)
- [ ] Soft delete + undelete + restore_version
- [ ] Tags + categories services + endpoints
- [ ] FTS: header-aware chunks on write + GIN tsvector + doc-level aggregation
- [ ] API keys: HMAC-SHA256+pepper, prefix lookup, mint/revoke, lifecycle (min-perm, auto-disable)
- [ ] Audit log: writes + auth/permission-denied + key-usage events
- [ ] Rate limits / quotas (`tower-governor`) + max-doc-size cap
- [ ] **MCP stdio** server (rmcp): full tool surface w/ `expected_version`
- [ ] **CLI `mdm`**: auth/config/org/proj/doc/tag/cat/search, raw-markdown-to-stdout
- [ ] Integration tests: tenant isolation, concurrency, RBAC lattice, CLI/MCP parity

## Phase 2 — Web connectors (completes launch)
- [ ] Self-hosted Logto configured (orgs, scopes, resource indicator, DCR/CIMD)
- [ ] `mcp` Streamable HTTP transport
- [ ] OAuth resource server: PRM endpoint, JWKS cache, `aud`/`iss`/`exp` validation, 401+`WWW-Authenticate`
- [ ] Spike: real Claude.ai + ChatGPT connector end-to-end
- [ ] Minimal BFF OAuth callback + cookie session

## Phase 3 — Human web app (Next.js)
- [ ] App shell + Logto BFF auth + org/project switcher
- [ ] Doc tree (virtualized) + CodeMirror 6 editor (inline preview)
- [ ] Version history + restore UI + conflict/merge UI
- [ ] Tag/category management, search UI + `cmdk` palette
- [ ] API-keys screen, share links (default-deny, audited)

## Phase 4 — Semantic search
- [ ] pgvector + `embedding halfvec(1024)` column + HNSW
- [ ] Async embedding worker (queue + dedup + backoff/dead-letter + FTS fallback)
- [ ] `search_docs` `mode=semantic|hybrid` (RRF)

## Phase 5 — Realtime + scale
- [ ] Yjs/CRDT over Rust websocket (agent as CRDT peer)
- [ ] OpenTelemetry export + alerting (conflict rate, embed-queue depth)
- [ ] Version compaction; optional SSO via Logto
