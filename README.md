# md-manager

[![CI](https://github.com/nimeus/md-manager/actions/workflows/ci.yml/badge.svg)](https://github.com/nimeus/md-manager/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.96%2B-orange.svg)](rust-toolchain.toml)

A multi-tenant manager for **markdown/text docs that live only in Postgres** — built so that **both humans and AI agents** can read, write, search, and share them safely.

Markdown files have become the working memory and helper docs for AI agents (Claude, Gemini, GPT). md-manager stores them in a database (no loose files), gives teams shared organizations / projects / tags, and lets agents work with docs through an **MCP server** and a **CLI** — under the same permission and versioning rules as the web UI.

## Highlights
- **Postgres-only storage** — every doc + version lives in the DB; the database is the single source of truth.
- **Built for agents** — MCP server (stdio for terminal agents, OAuth 2.1 Streamable HTTP so it plugs into Claude.ai / ChatGPT as a native connector) + a `mdm` CLI.
- **Multi-tenant** — organizations are hard tenant boundaries enforced by Postgres row-level security; teams, projects, tags, categories, and a full RBAC grant lattice on top.
- **Safe co-editing** — optimistic concurrency with structured conflict responses + 3-way merge; full version history & restore.
- **Search** — Postgres full-text out of the box; optional semantic/hybrid search via pgvector with env-driven, OpenAI-compatible embeddings (OpenRouter by default).
- **Sharing** — public/private share links, org invitations, member management.
- **Self-contained auth** — a built-in OAuth 2.1 authorization server (Google sign-in) for the connector flow; an external IdP (Logto) is supported as an alternative, not required.

## Stack
Rust (Axum · SQLx) · Next.js 15 · Postgres 17 (+ pgvector, optional).

## Repository layout
```
crates/core     domain models, RBAC lattice, 3-way merge, validation, crypto (pure)
crates/db       SQLx service layer, migrations, RLS-scoped tenant access
crates/config   configuration + secrets handling
crates/client   HTTP client shared by the CLI and stdio MCP server
crates/embed    OpenAI-compatible embeddings client
apps/api        Axum HTTP API: REST v1, remote MCP endpoint, built-in OAuth 2.1 server
apps/mcp        stdio MCP server (JSON-RPC 2.0)
apps/cli        `mdm` CLI
frontend/       Next.js 15 web app (BFF over the API)
migrations/     SQLx migrations
docs/           design plan, deployment, OAuth + embeddings guides
```

## Quick start
Requires Rust 1.96+ and Postgres 17 (Node 20+ for the web app).

```bash
# 1) Postgres roles + dev database (idempotent)
bash scripts/db-setup.sh

# 2) configure + run the API
cp .env.example .env            # then edit; see docs/ for production settings
set -a; source .env; set +a
cargo run -p mdm-api            # http://127.0.0.1:8080

# 3) bootstrap a tenant + API key, then use the CLI
cargo run -p mdm-cli -- bootstrap --email me@example.com --name Me \
  --org-slug acme --org-name Acme --token "$MDM_ADMIN_BOOTSTRAP_TOKEN" --save
cargo run -p mdm-cli -- doc create --project <slug> --path notes/hello --title Hello -m "# Hello"
cargo run -p mdm-cli -- search hello

# 4) MCP server for an agent host (stdio)
MDM_API_URL=http://127.0.0.1:8080 MDM_API_KEY=mk_... cargo run -p mdm-mcp

# 5) web app
cd frontend && npm install && npm run dev
```

## Documentation
- [docs/PLAN.md](docs/PLAN.md) — full design & architecture
- [docs/oauth-builtin.md](docs/oauth-builtin.md) — native Claude.ai / ChatGPT connector setup
- [docs/embeddings.md](docs/embeddings.md) — semantic search setup
- [docs/deploy-dokploy.md](docs/deploy-dokploy.md) — self-hosted deployment
- [CLAUDE.md](CLAUDE.md) — layout, conventions, and dev commands

## License
[MIT](LICENSE)
