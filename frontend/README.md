# md-manager web (Next.js)

The human web UI. A **BFF**: the browser never holds the backend token — Next.js keeps it in an
httpOnly cookie and proxies to the Rust API server-side.

**Sign-in is "Sign in with Google"** (OAuth authorization-code flow, implemented directly in the
BFF — no Auth.js dependency, no Docker, nothing extra to run). On first sign-in the backend
creates the user + a personal organization automatically; you can then create more orgs, switch
between them, and invite teammates by email. Targets Next.js 15 + React 19 + Tailwind v4.

## 1. Create a Google OAuth client (one-time, ~5 min)

[Google Cloud Console](https://console.cloud.google.com/) → **APIs & Services → Credentials →
Create credentials → OAuth client ID**:

- Application type: **Web application**
- **Authorized redirect URIs**: add `http://localhost:3000/auth/callback`
  (and your production `https://your-domain/auth/callback` later)
- Create, then copy the **Client ID** and **Client secret**.

(If prompted to configure the OAuth consent screen, set it to **External**, add your email as a
test user, and the `email`/`profile`/`openid` scopes — all default.)

## 2. Configure + run

```bash
# ── Terminal 1: the Rust API (repo root) ───────────────────────────────────────
bash scripts/db-setup.sh                       # one-time: Postgres roles + dev DB
MDM_GOOGLE_CLIENT_ID=<your-client-id> \
MDM_SESSION_SECRET=$(openssl rand -hex 32) \
  cargo run -p mdm-api                          # http://127.0.0.1:8080

# ── Terminal 2: the web app (frontend/) ────────────────────────────────────────
cd frontend
cp .env.local.example .env.local               # then fill in the three values below
#   MDM_API_URL=http://127.0.0.1:8080
#   MDM_GOOGLE_CLIENT_ID=<same client id as the API>
#   MDM_GOOGLE_CLIENT_SECRET=<your client secret>
npm install                                    # if peer-dep errors: npm install --legacy-peer-deps
npm run dev                                    # http://localhost:3000
```

> The **Client ID must match** on both sides (the API verifies Google's token against it; the web
> app starts the login with it). The **secret lives only on the Next.js server**. Use the **same**
> `MDM_SESSION_SECRET` whenever you restart the API, or existing sessions are invalidated.

Open http://localhost:3000 → **Sign in with Google**. You land in your auto-created org; create
projects/docs, invite teammates (**Members**), and switch orgs from the sidebar dropdown.

## 3. Verify headlessly (optional)

`./smoke-test.sh` checks the API is up and the web `/login` renders the Google button. The full
signed-in render can't be exercised by curl (login goes through Google), but if you export the
API's `MDM_SESSION_SECRET` the script will mint a session cookie and verify the BFF renders org
data too:

```bash
MDM_SESSION_SECRET=<same as the API> ./smoke-test.sh
```

## What's here

| Route | Purpose |
|---|---|
| `/login` | "Sign in with Google" |
| `/auth/google`, `/auth/callback` | OAuth start + callback (BFF); `/auth/switch` flips the active org |
| `/onboarding` | create an organization |
| `/projects`, `/projects/[slug]`, `/documents/[id]` | docs: list/create, editor (conflict-aware save), history/restore |
| `/search` | keyword search |
| `/settings/members` | invite teammates / revoke invites (owner/admin) |
| `/settings/keys` | mint (shown once) / revoke API keys for the CLI + agents |

- Auth plumbing: `lib/google-oauth.ts` (Google code flow), `lib/session.ts` (httpOnly cookie holds
  the backend `mss_` session token + current org), `lib/api.ts` (server-side client; sends
  `Authorization: Bearer` + `X-Org-Id`), `middleware.ts` (guard).
- The editor sends `expected_version`; on a 409 it offers **Load current** / **Overwrite with mine**.

## Follow-ups
- Add GitHub (or other) social logins — same shape as Google.
- CodeMirror 6 editor; tags/categories UI; `cmdk`; share-links UI; member-role management.
