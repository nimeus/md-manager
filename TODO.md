# md-manager — TODO / Roadmap Tracker

> Living checklist. Update statuses as work lands. Full rationale in [docs/PLAN.md](docs/PLAN.md).
> Legend: `[ ]` todo · `[~]` in progress · `[x]` done

**Current status (2026-06-28):** ✅ Phase 1 MVP + ✅ Phase 2 resource server (remote MCP over HTTP +
OAuth 2.1 token validation) + ✅ **Phase 3 web app BUILT & verified** (user ran `npm run build` +
`./smoke-test.sh` clean on their Mac, 2026-06-28). The full product is live: Rust agent backend
(API + `mdm` CLI + MCP stdio + MCP over HTTP, 20 tools) **and** the Next.js web UI, both over one
RLS/RBAC-enforced Postgres. Backend re-verified end-to-end against the running server (health, auth/401,
docs CRUD, 409+merge, FTS, MCP get_doc).

**Local dev:** Postgres 17 via Homebrew (no Docker). `bash scripts/db-setup.sh`, then `cargo run -p mdm-api`;
web app: `cd frontend && cp .env.local.example .env.local && npm install && npm run dev` (see [frontend/README.md](frontend/README.md)).

**Remaining for web connectors (external):** run self-hosted Logto + expose over public HTTPS, then
the live Claude.ai/ChatGPT connector spike. See [docs/oauth-logto.md](docs/oauth-logto.md). (Until Logto
is configured, `/.well-known/oauth-protected-resource` correctly returns 404 — nothing to advertise.)

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
- [x] CI workflow (`.github/workflows/ci.yml`: Postgres service, fmt --check, clippy -D warnings, build, test) — codebase is fmt + clippy clean

## Phase 1 — Agent-surface MVP ✅
- [x] Org/project/membership + RLS/RBAC (owner/admin/member/viewer)
- [x] Documents CRUD: UUID + mutable path, content_hash, current_version
- [x] Full-snapshot versioning + `version_kind` (checkpoint/autosave) + ~30s autosave coalesce
- [x] Optimistic concurrency: stale → 409 with `current` + `base` content for 3-way merge
- [x] Atomic `append` (FOR UPDATE serialised), restore, soft delete/undelete, move, history
- [x] Keyword FTS (generated tsvector + GIN, doc-level aggregation, snippet highlights)
- [x] Tags (org-scoped) + document tagging + **list documents by tag** (deny-filtered reverse lookup: db/REST `GET /v1/tags/{name}/documents`/CLI `mdm tag docs`/integration test)
- [x] API keys: HMAC+pepper, prefix lookup, mint/list/revoke, creator-role lifecycle binding
- [x] Audit log (writes + key events)
- [x] **HTTP API** (Axum): all endpoints + auth extractor + bootstrap + error mapping
- [x] **MCP server** (stdio JSON-RPC): 20 tools, raw-markdown reads, conflict-aware updates
- [x] MCP/CLI parity: retrieve documents by tag (`list_docs_by_tag`) and by category (`list_docs_by_category`) — added to both MCP surfaces (stdio + HTTP); verified live over `/mcp` (tools/list = 20; both return the doc)
- [x] **CLI `mdm`**: auth/whoami/org/proj/doc/search/tag/keys; raw-markdown to stdout; stdin/-m/--file body
- [x] `mdm-client`: shared async HTTP client (used by MCP + CLI)
- [x] CLI shell completions (`mdm completions <bash|zsh|fish|powershell|elvish>`, via `clap_complete`) — verified (zsh `#compdef`, bash script, bad shell rejected)
- [x] Integration tests vs Postgres: tenant isolation, concurrency, RBAC, search, key revoke
- [x] End-to-end verified: CLI + MCP agent loops, cross-surface consistency
- [x] **Categories** (org-scoped, hierarchical, cross-project) + document_categories — migration 0003, db, REST, CLI (`mdm cat`), MCP, integration tests
- [x] **Teams + per-project/per-doc grants + full RBAC lattice** (deny-veto, most-permissive, owner override, viewer-ceiling) — migration 0004, `mdm_core::rbac::resolve_doc_role`, per-doc authorization, db/REST/CLI (`mdm team`/`mdm grant`); verified live (member deny, owner override, team-deny-vetoes-grant)
- [x] Rate limits (per-user, `governor`) + per-project document quota — config-driven, verified live (429s) + quota integration test
- [x] Hide per-doc-denied docs from list/search results (not just on access)
- [x] **Public share links** (migration 0005) — read-only, expiring, revocable; HMAC+pepper token (shown once); db/REST/CLI (`mdm share`); PUBLIC `GET /v1/shared/{token}` (no auth); verified live incl. public fetch + revoke→404 + integration test
- [x] **Audit query** — admin-only `GET /v1/audit` (+ `mdm audit`, filter by target/action), reads the who/what/when written on every action; RLS-scoped; integration test (entries present, action filter, non-admin Forbidden)

## Phase 2 — Web connectors (resource server ✅; Logto go-live external)
- [x] Streamable HTTP `/mcp` transport (served by the API) — 20 tools, verified via curl
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
- [x] Static audit vs API contract: every page's fields match the Rust responses (whoami/projects/docs/history/search/keys), Next 15 async `params`/`searchParams` handled, all data pages dynamic via `cookies()` so `next build` needs no live API — no code fixes required
- [x] First-run kit: `.env.local.example`, `.gitignore`, corrected README (the old `curl POST /login` verify was wrong — login is a server action), and `frontend/smoke-test.sh` (bootstraps + seeds + mints the real session cookie + checks the BFF renders API data); API-side of the smoke test validated live
- [x] **Built & verified**: user ran `npm install && npm run build && npm run start` + `./smoke-test.sh` clean on their Mac (2026-06-28) — the app compiles and the BFF→API path renders live data. (npm was unreachable in the authoring sandbox; built on the user's machine.)
## Phase 3.5 — Real SaaS sign-in (Google) — backend ✅, frontend next
> User chose: **"Sign in with Google" via in-app Auth.js (no Docker)**; Rust+Postgres stays the source of truth for users/orgs/invites. (Supersedes the API-key-paste login + the Logto plan for human auth.)
- [x] **Backend (migration 0008, built + tested + live-verified):** `users.google_sub`; server-side Google ID-token verification (`apps/api/src/google.rs`, reuses JWKS/RS256, requires verified email); web session tokens (`mss_…`, HS256, `apps/api/src/session.rs`); `POST /v1/auth/google` JIT-provisions a user + personal org + auto-accepts email invites and returns a session token; multi-org **without weakening isolation** (inert-by-default `current_user_id` RLS policies + `begin_user_scoped`); org switcher via `X-Org-Id`; `POST /v1/orgs`, `GET /v1/me/orgs`, invitations (`/v1/invitations` create/list/revoke). 34 tests; integration test proves a stranger still can't see/enter another's org.
- [x] **Frontend (authored; user builds with npm):** "Sign in with Google" via the OAuth authorization-code flow **directly in the BFF** (no Auth.js dep, no Docker): `lib/google-oauth.ts` + `/auth/google` (start, CSRF state) + `/auth/callback` (exchange code → `/v1/auth/google` → store backend `mss_` session token in the httpOnly cookie) + `/auth/switch` (org switcher). Login page = Google button; `lib/session.ts` holds token+user+currentOrg; `lib/api.ts` sends `Authorization: Bearer` + `X-Org-Id`; onboarding (create org), org-switcher dropdown in nav, members/invites UI (`/settings/members`). Static-audited (no stale refs, imports resolve, no client→server-only imports); README has Google Cloud steps; smoke-test updated. **Needs `npm install && npm run build` + a Google OAuth client (user action).**
- [ ] CodeMirror 6 editor; tags/categories UI; `cmdk`; share links UI; member-role management

## Phase 4 — Semantic search ✅
- [x] pgvector `vector(N)` column (env-dim) + HNSW + async embedding worker (owner role, off the write path)
- [x] OpenAI-compatible embeddings client (`mdm-embed`), all env-driven, **OpenRouter default** ([docs/embeddings.md](docs/embeddings.md))
- [x] `search` `mode=keyword|semantic|hybrid` (RRF) across REST/CLI/MCP; tenant + deny-grant respected
- [x] Verified: pgvector semantic+hybrid (deterministic vectors) + live OpenRouter wiring (needs a real key to embed)
- [x] Embedding-cost dedup: per-chunk `content_hash` (migration 0006) — reindex preserves unchanged chunks' embeddings (diff, not delete-all), worker copies embeddings for identical content; verified (editing one section keeps the other's embedding)
- [x] Embedding backoff + dead-letter (migration 0007): per-chunk `embed_attempts`/`embed_next_attempt_at`/`embed_failed`/`embed_last_error`; `pending()` skips backed-off/dead chunks; worker isolates a failed batch one-chunk-at-a-time (poison input can't block batch-mates); env-driven `BACKOFF_BASE_SECS`/`MAX_ATTEMPTS`; verified (backoff hides chunk, 3rd failure dead-letters, store clears it)
- [x] Embedding model/dimension change: `ensure_schema` detects a width change (pgvector `atttypmod`) and drops+recreates the `embedding` column, clearing vectors and resetting worker bookkeeping (incl. dead-letters) so all chunks re-embed under the new model — instead of silently keeping the old width; verified (reconnect at new dim → column rebuilt, embeddings cleared, dead-letters re-queued)

## Phase 5 — Realtime + scale
- [ ] Yjs/CRDT over Rust websocket; OTel + alerting; version compaction; SSO
