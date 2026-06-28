# md-manager web (Next.js)

The human web UI. A **BFF**: the browser never holds the API key — Next.js stores it in an
httpOnly cookie and proxies to the Rust API server-side. (Logto OAuth login swaps in later;
only the sign-in step changes.)

> ⚠️ **Not built in the authoring environment.** The npm registry was unreachable there, so
> `npm install` / `next build` were not run. Build it on a machine with npm access — the code
> targets Next.js 15 (App Router) + React 19 + Tailwind v4.

## Run

```bash
# 1) start the Rust API (from the repo root) — see ../CLAUDE.md
cargo run -p mdm-api            # http://127.0.0.1:8080

# 2) the web app — the upstream API host is fixed server-side (MDM_API_URL)
cd frontend
npm install
MDM_API_URL=http://127.0.0.1:8080 npm run dev    # http://localhost:3000
```

Sign in with an API key (`mk_…`) from `mdm bootstrap` / the API-keys page. **Security:** the
client supplies only the key; the API host comes from `MDM_API_URL` on the Next server, so a
user can't make the BFF fetch arbitrary hosts (no SSRF). Defaults to `http://127.0.0.1:8080`.

## Verify (headless, without a browser)

```bash
npm run build                   # type-checks + compiles every route
npm run start &                 # serve the production build

# login route sets the httpOnly cookie, then SSR pages render with data:
curl -s -c /tmp/jar -X POST http://localhost:3000/login --data-urlencode "apiKey=mk_…"
curl -s -b /tmp/jar http://localhost:3000/projects | grep -o "Projects"
```

## What's here

| Route | Purpose |
|---|---|
| `/login` | enter API key → httpOnly session cookie |
| `/projects` | list + create projects |
| `/projects/[slug]` | a project's documents + create |
| `/documents/[id]` | CodeMirror-free markdown editor (edit/preview), **conflict-aware save**, version history + restore, delete |
| `/search` | keyword full-text search |
| `/settings/keys` | mint (shown once) / revoke API keys |

- BFF plumbing: `lib/session.ts` (cookie), `lib/api.ts` (server-side API client), `lib/actions.ts` (server actions), `middleware.ts` (auth guard).
- The editor (`components/editor.tsx`) sends `expected_version`; on a 409 it shows the current version and offers **Load current** or **Overwrite with mine** — surfacing the API's 3-way-merge data.

## Follow-ups
- Swap the API-key login for the Logto OAuth BFF flow (see `../docs/oauth-logto.md`).
- Upgrade the textarea editor to CodeMirror 6; add tags/categories UI, org/project switcher, share links.
