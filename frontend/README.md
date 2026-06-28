# md-manager web (Next.js)

The human web UI. A **BFF**: the browser never holds the API key — Next.js stores it in an
httpOnly cookie and proxies to the Rust API server-side. (Logto OAuth login swaps in later;
only the sign-in step changes.)

Targets **Next.js 15** (App Router) + **React 19** + **Tailwind v4**. The code has been
statically audited against the API contract (every page's fields match the Rust responses;
Next 15 async `params`/`searchParams` are handled; all data pages are dynamic via `cookies()`
so `next build` does **not** need the API running). It just needs `npm` to be reachable —
which it wasn't in the authoring sandbox, so install/build must run on your machine.

## Setup (exact steps)

```bash
# ── Terminal 1: the Rust API (from the repo root) ──────────────────────────────
bash scripts/db-setup.sh                       # one-time: Postgres roles + dev DB
MDM_ADMIN_BOOTSTRAP_TOKEN=dev-bootstrap-token \
  cargo run -p mdm-api                          # listens on http://127.0.0.1:8080

# ── Terminal 2: the web app (from frontend/) ───────────────────────────────────
cd frontend
cp .env.local.example .env.local               # sets MDM_API_URL=http://127.0.0.1:8080
npm install                                    # if peer-dep errors: npm install --legacy-peer-deps
npm run dev                                     # http://localhost:3000
```

Open http://localhost:3000 → you'll be redirected to `/login`. Get a key to paste:

```bash
# from the repo root, mint an admin key (prints `api_key.secret`: mk_…)
cargo run -p mdm-cli -- bootstrap \
  --email you@example.com --name You --org-slug acme --org-name Acme \
  --token dev-bootstrap-token
```

Paste the `mk_…` key on the login page. **Security:** the client supplies only the key; the
API host comes from `MDM_API_URL` on the Next server, so a user can't point the BFF at an
arbitrary host (no SSRF).

## Verify headlessly (one command)

With both servers up (use `npm run build && npm run start` for a production build, or just
`npm run dev`), run the end-to-end smoke test — it bootstraps a tenant, seeds a project, mints
the exact session cookie the app uses, and confirms the BFF renders API data:

```bash
./smoke-test.sh
# or point it somewhere else:
API_URL=http://127.0.0.1:8080 WEB_URL=http://localhost:3000 \
  BOOTSTRAP_TOKEN=dev-bootstrap-token ./smoke-test.sh
```

> Note: `/login` is a React **server action**, not a plain form POST — you can't log in with a
> raw `curl -X POST /login`. The smoke test instead mints the `mdm_session` cookie directly
> (it's just `base64({"apiKey":"…"})`, exactly what `lib/session.ts` writes) and requests
> `/projects` with it, which exercises the real server-side BFF→API path.

## What's here

| Route | Purpose |
|---|---|
| `/login` | enter API key → httpOnly session cookie |
| `/projects` | list + create projects |
| `/projects/[slug]` | a project's documents + create |
| `/documents/[id]` | markdown editor (edit/preview), **conflict-aware save**, version history + restore, delete |
| `/search` | keyword full-text search |
| `/settings/keys` | mint (shown once) / revoke API keys |

- BFF plumbing: `lib/session.ts` (cookie), `lib/api.ts` (server-side API client), `lib/actions.ts` (server actions), `middleware.ts` (auth guard).
- The editor (`components/editor.tsx`) sends `expected_version`; on a 409 it shows the current version and offers **Load current** or **Overwrite with mine** — surfacing the API's 3-way-merge data.

## Follow-ups
- Swap the API-key login for the Logto OAuth BFF flow (see `../docs/oauth-logto.md`).
- Upgrade the textarea editor to CodeMirror 6; add tags/categories UI, org/project switcher, share links.
