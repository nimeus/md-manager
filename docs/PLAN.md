# md-manager — Implementation Plan

> Source-of-truth plan. Keep in sync with [TODO.md](../TODO.md) and [CLAUDE.md](../CLAUDE.md).
> Approved 2026-06-27.

## Context

Markdown/text files have become the working memory and "helper docs" for AI agents (Claude, Gemini, GPT). For a solo dev they're scattered; for a team they're nearly impossible to share, keep current, and let agents read/write safely. **md-manager** is a multi-tenant SaaS where these docs live **only in Postgres** (never as files) and are first-class for **both humans and AI agents**: humans edit in a web UI, agents read/write through an **MCP server** and a **CLI** — all enforcing identical rules so nobody clobbers anyone.

### Decisions locked

| Topic | Decision |
|---|---|
| Deployment | **Multi-tenant SaaS**; organization = hard tenant boundary, enforced by Postgres RLS |
| Tenancy shape | A user belongs to **many orgs**; each org has **teams** and **projects**; **tags + categories** cross projects within an org |
| MVP focus | **Agent surface first** (backend + MCP + CLI); full human editor UI is fast-follow |
| Agent clients at launch | **Terminal/IDE agents AND hosted web connectors** (Claude.ai + ChatGPT) → OAuth 2.1 is in the MVP |
| Login/auth server | **Self-hosted Logto** (open-source, runs in our infra) as the OAuth 2.1 Authorization Server |
| Conflicts | **Detect + 3-way merge** (optimistic concurrency; structured 409). No realtime CRDT in v1 |
| Doc identity | Canonical **immutable UUID** + **mutable human path/slug** (unique within a project) |
| Search | **Postgres full-text (keyword) first**; pgvector semantic/hybrid is a later phase |

---

## 1. Architecture

A single **cargo workspace** with a shared, DB-agnostic core behind repository traits, three thin binaries, plus a Next.js app and self-hosted Logto. Postgres is the single source of truth for all document text.

```
                         ┌───────────────────────────────────────────────┐
                         │                 Postgres 18                    │
                         │  orgs · members · teams · projects · documents │
                         │  document_versions · tags · categories ·       │
                         │  doc_chunks(tsvector) · api_keys · audit_log    │
                         └───────────────────────────────────────────────┘
                                  ▲ md_app role: NON-OWNER + NOBYPASSRLS
              ┌───────────────────┼────────────────────┐
        ┌───────────┐       ┌───────────┐         ┌───────────┐
        │   api     │ Axum  │   mcp     │ rmcp    │  cli (mdm)│ clap
        └─────┬─────┘       └─────┬─────┘         └─────┬─────┘
              └───────────┬───────┴──────────┬──────────┘
                          ▼                  ▼
                 ┌──────────────┐   ┌──────────────┐
                 │  core (lib)  │◄──│   db (lib)   │   + config (lib)
                 │ domain,      │   │ PgPool,SQLx, │
                 │ services,    │   │ repo impls,  │
                 │ repo traits  │   │ migrations,  │
                 │ RBAC, errors │   │ TenantDb     │
                 └──────────────┘   └──────────────┘

 Browser ──HttpOnly cookie──► Next.js 15 (BFF) ──confidential OAuth client──► Logto (AS)
 Web AI connectors (Claude.ai / ChatGPT) ──OAuth2.1 aud-bound JWT──► mcp (resource server)
 Terminal agents (Claude Code/Gemini CLI/Codex) ──API key (Bearer)──► mcp (stdio) / api
 Logto issues all tokens; api + mcp only VALIDATE them. All paths resolve to one (user, org, scopes) → one RBAC layer.
```

**Why the shared `core`:** `core` holds framework-agnostic domain models, **repository traits** (`DocRepository`, `OrgRepository`, …), the RBAC resolver, and `thiserror` errors. `db` implements those traits with SQLx. The three binaries contain only transport wiring, so the API, MCP, and CLI enforce **identical** validation, versioning, RBAC, and concurrency rules.

**Workspace crates**

| Crate | Type | Responsibility |
|---|---|---|
| `core` (`mdm-core`) | lib | domain models, services, repo **traits**, RBAC resolver, typed errors |
| `db` (`mdm-db`) | lib | `PgPool`, SQLx repo impls, embedded migrations, **`TenantDb`** (RLS session wrapper) |
| `config` (`mdm-config`) | lib | figment config, `secrecy` secrets, tracing init |
| `api` (`mdm-api`) | bin | Axum HTTP API (web app BFF target + REST for CLI) |
| `mcp` (`mdm-mcp`) | bin | rmcp MCP server: **stdio** (terminal agents) + **Streamable HTTP** (web connectors) |
| `cli` (`mdm-cli`, binary `mdm`) | bin | clap CLI — **HTTP client over the API only** |

---

## 2. Tech stack (opinionated, current as of 2026)

**Backend (Rust)**
- Web: `axum` 0.8 + `tower-http` (trace, CORS, request-id, compression) + `tower-governor` (rate limiting)
- DB: `sqlx` 0.9 (Postgres, compile-time-checked SQL, `PgPool`, embedded migrations). **Commit the `.sqlx` offline cache**.
- MCP: `rmcp` (features: `server`, `macros`, `transport-io`, `transport-streamable-http-server`)
- CLI: `clap` (derive) + `reqwest` (rustls) + `comfy-table`
- Auth/crypto: `jsonwebtoken` (validate Logto JWTs) + JWKS cache; `hmac`+`sha2` (API-key/share-token hashing with a server pepper); `subtle` (constant-time compare)
- Config/secrets: `figment` + `dotenvy` + `secrecy::SecretString`
- Errors: `thiserror` in libs, `anyhow` in bins, an api error type impl `IntoResponse`
- IDs: **UUIDv7**
- Observability: `tracing` + `tracing-subscriber` + `tower_http::trace`; OTLP export deferred

**Frontend (Next.js)** — fast-follow after the agent surface
- Next.js 15 App Router, React 19, TS; Tailwind v4 + shadcn/ui; TanStack Query v5
- **Editor: CodeMirror 6** (raw markdown is the source of truth). **Reject WYSIWYG** — its serialization corrupts line-based diffs/merges against agent edits.
- BFF: Next.js is a confidential OAuth client against Logto; browser holds only `HttpOnly; Secure; SameSite` cookies.

**Auth server:** self-hosted **Logto** (container + its own Postgres). Hosted login + consent, Organizations model, DCR, PKCE, JWKS, resource-indicator → `aud` binding.

**Database:** Postgres 18. pgvector added only in the semantic-search phase.

---

## 3. Data model + multi-tenant isolation

UUIDv7 PKs everywhere. Soft-delete via `deleted_at` with **partial unique indexes** (`WHERE deleted_at IS NULL`). Every tenant table carries a `NOT NULL org_id` and is protected by RLS.

| Table | Key columns | Notes |
|---|---|---|
| `users` | `id`, `email` (unique), `display_name`, `logto_sub` | identity; mirrors the Logto subject |
| `organizations` | `id`, `slug` (unique), `name`, `deleted_at` | **tenant boundary** (maps to a Logto Organization) |
| `organization_members` | `org_id`, `user_id`, `role` (owner/admin/member/viewer) | PK (org_id,user_id); a user is in **many** orgs |
| `teams` | `id`, `org_id`, `name`, `slug` | grouping of members for bulk grants |
| `team_members` | `team_id`, `user_id` | |
| `projects` | `id`, `org_id`, `slug`, `name`, `deleted_at` | document container; unique (org_id, slug) where not deleted |
| `project_grants` | `id`, `org_id`, `project_id`, `subject_type`, `subject_id`, `role` | grant a user or team a role on a project |
| `documents` | `id` (**UUID**), `org_id`, `project_id`, `path` (**mutable slug**), `title`, **`content` TEXT**, `content_hash`, `current_version`, `created_by`, `updated_by`, ts, `deleted_at` | text lives **here only**; unique (project_id, path) where not deleted; `CHECK (octet_length(content) <= max)` |
| `document_versions` | `id`, `org_id`, `document_id`, `version`, **`content` TEXT snapshot**, `content_hash`, **`version_kind`** (checkpoint/autosave), `actor_type`, `actor_id`, `created_at` | **full snapshots** → O(1) restore |
| `document_grants` | `id`, `org_id`, `document_id`, `subject_type`, `subject_id`, `role` (incl. **`none` = deny**) | per-doc ACL overlay |
| `tags` | `id`, `org_id`, `name` | **org-scoped**, reusable across projects; unique (org_id, name) |
| `document_tags` | `document_id`, `tag_id` | many-to-many |
| `categories` | `id`, `org_id`, `parent_id`, `name`, `slug` | org-scoped hierarchical taxonomy, crosses projects |
| `document_categories` | `document_id`, `category_id` | |
| `api_keys` | `id`, `org_id`, `name`, `key_prefix`, `key_hash` (**HMAC-SHA256+pepper**), `role`, `scopes`, `created_by`, `last_used_at`, `revoked_at`, `expires_at` | agent auth; see §5 lifecycle |
| `share_links` | `id`, `org_id`, `document_id`, `token_prefix`, `token_hash`, `role`, `expires_at`, `revoked_at` | later phase; default-deny, audited |
| `doc_chunks` | `id`, `org_id`, `document_id`, `chunk_index`, `content`, `heading_path`, `tsv` (**GENERATED tsvector**, GIN) | header-aware; FTS in MVP. Embedding col added later (§6) |
| `audit_log` | `id`, `org_id`, `actor_type`, `actor_id`, `action`, `target`, `metadata` jsonb, `created_at` | **MVP** (writes + auth events) |

### Row-Level Security (hard tenant isolation)
- App connects as **`md_app`**: a **non-owner, `NOBYPASSRLS`** role. Table owner/migrator is separate. (Owners/superusers bypass RLS — the #1 footgun.)
- Every tenant table: `ENABLE` + **`FORCE ROW LEVEL SECURITY`**, policy `USING (org_id = current_org_id()) WITH CHECK (org_id = current_org_id())`, where `current_org_id() = NULLIF(current_setting('app.current_org_id', true), '')::uuid` → **unset GUC yields zero rows** (fails closed).
- Per request: txn + `set_config('app.current_org_id', $1, true)` (+ actor id/type) — `is_local=true`, **safe under PgBouncer transaction pooling**.
- Rust **`TenantDb`** owns the txn; **repositories accept only `&mut TenantDb`, never a bare pool connection**. Startup asserts `md_app` is `NOBYPASSRLS`.

### RBAC resolution (effective permission on a document)
Lattice `none < viewer < commenter < editor < admin`:
1. Per-doc grant role `none` (user or their team) → **DENY** (vetoes), unless org **owner/admin** (override).
2. Else **MAX** over org base role, project grants, per-doc grants (positive = most-permissive).
3. Org **viewer** clamped to `viewer` (hard ceiling).
One SQL CTE resolver in `core`/`db`, reused by every surface.

---

## 4. MCP tool surface + CLI command tree

### MCP tools (rmcp; typed `Parameters<T>` structs)
```
list_orgs · list_projects(org) · list_docs(project, cursor?)
search_docs(project|org, query, mode=keyword[, semantic|hybrid later], top_k?)
get_doc(doc_id | project+path) · get_doc_history(doc_id)
create_doc(project, path, title, content[, tags, category])
update_doc(doc_id, content, expected_version)     # optimistic → structured 409
append_to_doc(doc_id, content)                    # atomic server-side concat
move_doc(doc_id, new_path) · delete_doc(doc_id)   # soft delete
restore_version(doc_id, version) · undelete_doc(doc_id)
list_tags(org) · list_categories(org)
```
Stable UUIDs, project-relative paths, ISO-8601 ts, cursor pagination, read-only vs destructive tool hints. **MCP server never forwards the client's token upstream.** CLI and MCP kept symmetric.

### CLI (`mdm`, clap-derive) — HTTP client over the API only
```
Global: --profile --org --project --output(auto|human|json|raw) --json --api-url --api-key --verbose --no-color
mdm auth   {login, logout, status, whoami}
mdm config {init, set, get, list, profile {list, add, use, rm}}
mdm org    {list, use, current}
mdm proj   {list, use, current, create}
mdm doc    {list, get, create, edit, append, mv, rm, restore, history}
mdm tag    {list, add, rm}   mdm cat {list}
mdm search <query> [--mode keyword]
mdm completions <shell>
```
- `mdm doc get` prints **raw markdown to stdout** by default; `--json` wraps with metadata.
- Body input precedence: `--file` → `-m` → stdin → `$EDITOR` (**TTY-only**). Data→stdout, logs→stderr. Exit codes: 0 ok, 2 usage, 3 auth, 4 not-found, 5 network. Respect `NO_COLOR`.
- **HTTP-only** (no direct DB) — a direct-DB mode would bypass RLS/RBAC.

---

## 5. Auth model — three doors, one identity

All resolve to `(user, org, scopes)` → one RBAC layer. **Logto issues every token; `api`/`mcp` only validate.**

**A. Web AI connectors (Claude.ai, ChatGPT) — OAuth 2.1, MVP.** The `mcp` binary is an **OAuth 2.1 resource server**:
- `GET /.well-known/oauth-protected-resource` (RFC 9728) → `{ resource, authorization_servers:[Logto], scopes_supported, bearer_methods_supported:["header"] }`.
- `POST /mcp` (+ GET SSE) = rmcp Streamable HTTP behind auth middleware.
- Middleware: `Authorization: Bearer <jwt>`; missing/invalid → **401 + `WWW-Authenticate: Bearer resource_metadata="…"`**; validate via cached **JWKS**, check `iss`, `exp/nbf`, **`aud` == canonical URI** (RFC 8707 — most common silent breakage). Map `sub`→user, org claim→org, `scope`→scopes.
- Logto handles `/authorize`, `/token`, `/register` (DCR), `/jwks`, hosted login + consent.

**Launch gotchas:** allowlist `claude.ai` AND `claude.com` callbacks + `chatgpt.com/connector/...`; one canonical resource URI used byte-identically; no empty-string/null URL fields in DCR; RFC 6749 `invalid_grant` on bad refresh; SLAs (discovery/register/token ≤10s, refresh ≤30s — keep JWKS warm); WAF allows `Authorization` header + Anthropic/OpenAI outbound.

**B. Terminal/IDE agents — API keys.** `Authorization: Bearer mk_…` over stdio MCP + REST. **HMAC-SHA256(pepper, key)** of a ≥256-bit CSPRNG token, indexed `key_prefix`, constant-time compare. Key bound to creating user; **effective perm = min(key role/scopes, creator's CURRENT org/project role)**, re-evaluated per request; **auto-disabled when creator loses membership**. Shown once.

**C. Web app humans — BFF cookie session.** Next.js = confidential OAuth client against Logto (authz_code + PKCE), tokens server-side, `HttpOnly; Secure; SameSite` cookies.

---

## 6. Concurrency, versioning, search

**Conflicts (detect + 3-way merge).** `update_doc` requires `expected_version` (HTTP `If-Match`). Stale write → **409** returning `current_version`, `current_content`, **and `base_content`** (snapshot at `expected_version`) for 3-way merge; `core` offers a server-side merge helper (`similar`/`diffy`) with `merge_strategy = fail|three_way|append_only`. **`append_to_doc` = atomic `UPDATE … SET content = content || $1 … RETURNING`** under row lock. Realtime CRDT is a later phase; history + restore is the safety net.

**Versioning.** Full snapshots; **`version_kind`** separates human `checkpoint`s from agent `autosave`s; Phase-1 **debounce/coalesce** (≤1 trailing autosave per (document, actor) within ~30s) + retention job. `max-doc-size` cap (~1 MB) via `CHECK` + append guard.

**Search (keyword first).** MVP = Postgres FTS: `doc_chunks` GENERATED `tsvector` + GIN, `websearch_to_tsquery` + `ts_rank_cd`, **aggregated to documents** (group by `document_id`, max rank). Chunking header-aware, **synchronous on write**. Later: `CREATE EXTENSION vector` + `ALTER TABLE doc_chunks ADD COLUMN embedding halfvec(1024)` (metadata-only), async embedding backfill (`FOR UPDATE SKIP LOCKED` + tokio worker, per-chunk `content_hash` dedup, backoff/dead-letter, FTS fallback) + HNSW; `search_docs` gains `mode=semantic|hybrid` (RRF).

---

## 7. Cross-cutting

- **Rate limits & quotas (MVP):** per-key/per-org limits (`tower-governor`), max doc size, max docs/project.
- **Audit log (MVP):** writes + auth/permission-denied + key-usage.
- **Backups/DR (from launch):** Postgres is the only copy — PITR + automated backups + restore tests.
- **Secrets:** dev via env/`.env`; prod via a secrets manager. Pepper + JWT config + Logto creds there.
- **Migrations:** `sqlx::migrate!` forward, expand-contract, `.sqlx` cache in CI, run by owner role (not `md_app`).

---

## 8. Phased roadmap

**Phase 0 — Scaffolding.** git init; cargo workspace; `core` models + traits + RBAC + errors; `db` `PgPool`/`TenantDb` + first migration + RLS + roles; `.sqlx` cache; docker-compose (Postgres + Logto); CI.

**Phase 1 — Agent-surface MVP.** Org/team/project/membership + RLS/RBAC; docs CRUD (UUID+path), versioning (`version_kind`+debounce), 409+merge, atomic append, soft-delete/undelete/restore, audit, rate limits, max-doc cap; tags + categories; FTS; **API keys**; **MCP stdio** + **CLI**.

**Phase 2 — Web connectors.** Logto wired; `mcp` Streamable HTTP + OAuth resource-server endpoints; spike a real Claude.ai + ChatGPT connect; minimal BFF callback + cookie session.

**Phase 3 — Human web app.** Next.js: switchers, doc tree, CodeMirror editor, history/restore + conflict UI, tag/category mgmt, search + `cmdk`, API-keys screen, share links.

**Phase 4 — Semantic search.** pgvector + embedding worker; `search_docs` `semantic`/`hybrid` (RRF).

**Phase 5 — Realtime + scale.** Yjs/CRDT over Rust websocket; OTel + alerting; version compaction; SSO via Logto.

---

## 9. Defaults chosen (change if needed)

- Single shared DB + RLS (not schema/DB-per-org).
- RBAC: most-permissive accumulation, single per-doc `none`=deny that vetoes (org owner/admin override), org viewer ceiling.
- Versioning: full snapshots + ~30s autosave coalesce + keep checkpoints; max doc ~1 MB.
- CLI binary name **`mdm`**.
- Embedding specifics deferred to Phase 4.

---

## 10. Verification

- **Tenant isolation:** unset GUC → zero rows; org A can't read org B; assert `md_app` is `NOBYPASSRLS`; fresh pooled conn has no leaked `app.current_org_id`.
- **Concurrency:** agent v5 + human v6 → 409 with base+current; concurrent `append` loses nothing.
- **RBAC:** table-driven over the lattice.
- **CLI/MCP parity:** same op via both → identical result + perms.
- **Agents (P1):** Claude Code / Gemini CLI over stdio MCP w/ API key — create, search, read, edit-with-conflict, restore.
- **Web connectors (P2):** real Claude.ai + ChatGPT connect via Logto → list/read/write; verify `aud`/`iss`/`exp`, 401+`WWW-Authenticate`, refresh rotation.
- **Search:** FTS returns documents (not raw chunks), ranked; immediate after write.
- `cargo test` + `sqlx` offline build in CI on every change.
