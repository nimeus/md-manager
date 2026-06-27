# md-manager

A multi-tenant manager for **markdown/text docs that live only in Postgres** — built so that **both humans and AI agents** can read, write, search, and share them safely.

Markdown files have become the working memory and helper docs for AI agents (Claude, Gemini, GPT). md-manager stores them in a database (no loose files), gives teams shared organizations / projects / tags, and lets agents work with docs through an **MCP server** and a **CLI** — under the same permission and versioning rules as the web UI.

## Highlights
- **Postgres-only storage** — every doc + version lives in the DB; the database is the single source of truth.
- **Built for agents** — MCP server (stdio for terminal agents, OAuth 2.1 Streamable HTTP for Claude.ai/ChatGPT connectors) + a `mdm` CLI.
- **Multi-tenant** — organizations are hard tenant boundaries (Postgres row-level security); teams, projects, tags, and categories on top.
- **Safe co-editing** — optimistic concurrency with structured conflict responses + 3-way merge; full version history & restore.
- **Keyword search now, semantic later** — Postgres full-text first; pgvector hybrid search in a later phase.

## Stack
Rust (Axum · rmcp · SQLx) · Next.js 15 · Postgres 18 · self-hosted Logto (auth).

## Status
Early development — **Phase 0 (scaffolding)**. See [TODO.md](TODO.md) for the roadmap and [docs/PLAN.md](docs/PLAN.md) for the full design.

## Development
Requires Rust 1.96+, Node 20+, and Docker (for Postgres + Logto). See [CLAUDE.md](CLAUDE.md) for layout, conventions, and commands.

## License
MIT
