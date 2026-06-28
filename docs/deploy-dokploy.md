# Deploying md-manager on a VPS with Dokploy

A complete, do-it-once walkthrough. You'll run three things on your server, all managed by
[Dokploy](https://dokploy.com):

```
                 ┌──────────────┐
   docs.you.com  │   web app    │  Next.js (this is what people open)
        ──────►  │  (port 3000) │
                 └──────┬───────┘
                        │  server-side calls (BFF)
                 ┌──────▼───────┐
    api.you.com  │   API        │  Rust/Axum — also what the CLI, MCP, and
        ──────►  │  (port 8080) │  web connectors talk to
                 └──────┬───────┘
                 ┌──────▼───────┐
                 │  Postgres    │  your documents live here (the only copy)
                 └──────────────┘
```

You provide two subdomains (e.g. `docs.you.com` for the app and `api.you.com` for the API),
and a Google OAuth client. Dokploy handles Docker builds, HTTPS (Let's Encrypt), and env vars.

---

## 0. Prerequisites

- A VPS with **Dokploy installed** (their one-line installer:
  `curl -sSL https://dokploy.com/install.sh | sh`). Give it **≥ 2 GB RAM** — the Rust API
  compiles on the server. (Tight on RAM? See *Build elsewhere* at the bottom.)
- A **domain** with two A-records pointing at the VPS IP: `docs.you.com` and `api.you.com`.
- This repository pushed to **GitHub/GitLab/Gitea** (Dokploy builds from Git). Connect your
  Git provider under Dokploy → *Settings → Git*.
- A **Google OAuth client** — [Google Cloud Console](https://console.cloud.google.com) →
  *APIs & Services → Credentials → OAuth client ID → Web application*. Leave the redirect URI
  for [step 5](#5-point-google-at-your-domain); copy the **Client ID** and **Client secret**.

Generate three secrets now (keep them somewhere safe):

```bash
openssl rand -hex 32   # → MDM_API_KEY_PEPPER
openssl rand -hex 32   # → MDM_SESSION_SECRET
openssl rand -hex 16   # → MDM_ADMIN_BOOTSTRAP_TOKEN
```

---

## 1. Create the database

In Dokploy: **Create → Database → Postgres**. Set the **database name to `md_manager`**, image
`postgres:17` (or `pgvector/pgvector:pg17` for semantic search). Deploy it.

That's all — **no SQL to run.** md-manager needs two roles (`md_owner` to own the tables and
`md_app`, a non-owner `NOBYPASSRLS` runtime role — that second one is what makes row-level
security actually isolate tenants, since a superuser would bypass it). The API **creates both
roles itself on first boot** when you give it the superuser URL (next step).

From the database page, note three things Dokploy shows: the **internal host** (e.g.
`md-db-xxxx`), and the superuser **user** and **password** you set. You'll also pick passwords
for the two app roles — use the ones generated above, or your own.

> Don't want the API holding a superuser URL? Skip `MDM_SETUP_DATABASE_URL` below and create the
> roles yourself once — paste [`scripts/db-roles.sql`](../scripts/db-roles.sql), or run a single
> line over SSH:
> ```bash
> docker exec <pg-container> psql -U <superuser> -d md_manager -c "CREATE ROLE md_owner LOGIN PASSWORD 'OWNER_PW'; CREATE ROLE md_app LOGIN NOBYPASSRLS PASSWORD 'APP_PW'; ALTER SCHEMA public OWNER TO md_owner; GRANT USAGE ON SCHEMA public TO md_app;"
> ```

---

## 2. Deploy the API

**Create → Application** (e.g. `md-api`), source = this Git repo/branch.

**Build settings** (Dokploy → the app → *Build*):
- Build type: **Dockerfile**
- Dockerfile path: `Dockerfile.api`
- Build context / path: `.` (repository root)

**Environment** (*Environment* tab) — replace hosts/passwords/domain:

```bash
# Superuser URL (Dokploy's user/pass) — used ONCE at boot to create the two app roles below.
MDM_SETUP_DATABASE_URL=postgres://DOKPLOY_USER:DOKPLOY_PW@INTERNAL_DB_HOST:5432/md_manager
# The two app roles the API will create (pick passwords; OWNER_PW ≠ APP_PW):
MDM_MIGRATION_DATABASE_URL=postgres://md_owner:OWNER_PW@INTERNAL_DB_HOST:5432/md_manager
MDM_DATABASE_URL=postgres://md_app:APP_PW@INTERNAL_DB_HOST:5432/md_manager
MDM_API_KEY_PEPPER=<openssl rand -hex 32>
MDM_ADMIN_BOOTSTRAP_TOKEN=<openssl rand -hex 16>
MDM_SESSION_SECRET=<openssl rand -hex 32>
MDM_GOOGLE_CLIENT_ID=<your-google-client-id>.apps.googleusercontent.com
MDM_PUBLIC_URL=https://api.you.com
# MDM_API_ADDR is already 0.0.0.0:8080 in the image.
```

**Domain** (*Domains* tab): add `api.you.com`, container port **8080**, enable HTTPS.

Click **Deploy**. The first build compiles the whole Rust workspace — give it a few minutes. On
boot the API uses `MDM_SETUP_DATABASE_URL` to create `md_owner` / `md_app` (idempotently), runs
the migrations as `md_owner`, then serves as `md_app`. When it's up,
`https://api.you.com/healthz` returns `ok`. (Created the roles yourself? Just omit
`MDM_SETUP_DATABASE_URL`.)

> Optional — **semantic search**: use the `pgvector/pgvector:pg17` DB image, run
> `CREATE EXTENSION vector;` once (superuser), and add `MDM_EMBEDDING_ENABLED=true` plus the
> `MDM_EMBEDDING_*` vars from [embeddings.md](embeddings.md).

---

## 3. Deploy the web app

**Create → Application** (e.g. `md-web`), same Git repo/branch.

**Build settings**:
- Build type: **Dockerfile**
- Dockerfile path: `frontend/Dockerfile`
- Build context / path: `frontend`

**Environment**:

```bash
MDM_API_URL=https://api.you.com
MDM_GOOGLE_CLIENT_ID=<same google client id as the API>
MDM_GOOGLE_CLIENT_SECRET=<your-google-client-secret>
# NODE_ENV=production is already set in the image.
```

**Domain**: add `docs.you.com`, container port **3000**, enable HTTPS. **Deploy.**

---

## 4. Sanity check

```bash
curl https://api.you.com/healthz                       # → ok
curl -s https://api.you.com/v1/auth/google \
  -H 'content-type: application/json' -d '{"id_token":"x"}'   # → 401 (Google sign-in is enabled)
open https://docs.you.com                              # → the landing page
```

---

## 5. Point Google at your domain

In your Google OAuth client, add the **Authorized redirect URI** (exactly):

```
https://docs.you.com/auth/callback
```

If the OAuth consent screen is in *Testing*, add your account under *Test users*.

---

## 6. First sign-in

Open `https://docs.you.com` → **Sign in with Google**. Your user and a personal organization
are created automatically; invite teammates from **Settings → Members**, and mint agent keys
from **Settings → API Keys** for the CLI / MCP server (full instructions live in the app at
`https://docs.you.com/docs/cli` and `/docs/mcp`).

Prefer an admin via the API instead? With your bootstrap token:

```bash
curl -X POST https://api.you.com/v1/bootstrap \
  -H 'content-type: application/json' \
  -H 'x-bootstrap-token: <MDM_ADMIN_BOOTSTRAP_TOKEN>' \
  -d '{"email":"you@you.com","display_name":"You","org_slug":"acme","org_name":"Acme","key_name":"admin"}'
```

---

## Operating it

- **HTTPS** is automatic via Dokploy's Traefik + Let's Encrypt once DNS resolves to the VPS.
- **Backups — important.** Postgres is the *only* copy of every document. Enable Dokploy's
  scheduled database backups (and test a restore). There is no file fallback by design.
- **Updating.** Push to your branch and hit **Deploy** (or enable auto-deploy webhooks). The API
  re-runs any new migrations on startup.
- **Secrets.** Set them only in Dokploy's Environment tab. If you rotate `MDM_SESSION_SECRET`,
  everyone is signed out; if you rotate `MDM_API_KEY_PEPPER`, existing API keys stop working.
- **Web connectors** (Claude.ai / ChatGPT). The API already serves remote MCP at
  `POST https://api.you.com/mcp`. To let hosted assistants connect, stand up self-hosted Logto
  and set the `MDM_OAUTH_*` vars — see [oauth-logto.md](oauth-logto.md) and the in-app
  `/docs/connectors`.

## Troubleshooting

- **API build runs out of memory** → use a bigger VPS, add swap, or *build elsewhere* (below).
- **`startup assertion: md_app can bypass RLS`** → `md_app` was created without `NOBYPASSRLS`
  (or is a superuser). Re-run `ALTER ROLE md_app NOBYPASSRLS;` and redeploy.
- **`redirect_uri_mismatch` on login** → the Google redirect URI must be exactly
  `https://docs.you.com/auth/callback`.
- **Login fails / "not configured"** → `MDM_GOOGLE_CLIENT_ID` must be set on **both** apps and
  match; the secret goes on the web app only.
- **Web app can't reach the API** → confirm `MDM_API_URL` is the API's public HTTPS URL and that
  `https://api.you.com/healthz` works.

### Build elsewhere (low-RAM VPS)

Build the images on your laptop or CI and push to a registry, then point Dokploy at the images
instead of the Git build:

```bash
# API (from repo root)
docker build -f Dockerfile.api -t YOUR_REGISTRY/md-api:latest .
# web (from frontend/)
docker build -f frontend/Dockerfile -t YOUR_REGISTRY/md-web:latest frontend
docker push YOUR_REGISTRY/md-api:latest && docker push YOUR_REGISTRY/md-web:latest
```

In Dokploy, set each Application's source to **Docker image** instead of Git, keep the same
Environment and Domain settings, and deploy.
