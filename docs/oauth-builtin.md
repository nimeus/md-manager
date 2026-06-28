# Native connector — built-in OAuth 2.1 authorization server

md-manager can be added directly in **Claude.ai** and **ChatGPT** as a custom connector. The API
is its own OAuth 2.1 authorization server — **no Logto, no external IdP**. The user signs in with
Google, picks an organization, and the connector receives a revocable token scoped to that org.

## Enable it

On the **API** service:

```
MDM_OAUTH_MODE=builtin
MDM_PUBLIC_URL=https://mdm-api.example.com     # this API's public origin
MDM_WEB_URL=https://mdm.example.com            # the web app (hosts /oauth/consent)
```

The web app already needs `MDM_API_URL` (the API origin) and `MDM_APP_URL` (its own origin), plus
Google sign-in configured (`MDM_GOOGLE_CLIENT_ID` on the API). Token lifetimes are tunable
(`MDM_OAUTH_{ACCESS,REFRESH,CODE,REQUEST}_TTL_SECS`, `MDM_OAUTH_DCR_PER_HOUR`); defaults are
1h / 30d / 60s / 10m / 20-per-hour.

The reverse proxy must forward the `Authorization` header (already required for `/mcp`) and a
correct `X-Forwarded-For` (the Dynamic Client Registration limiter keys on it).

## What the user does

1. In Claude → **Settings → Connectors → Add custom connector** → URL `https://mdm-api.example.com/mcp`.
2. A sign-in window opens → **Continue with Google** → choose the organization → **Allow**.
3. The 20 tools appear, scoped to that org with the user's permissions.

Revoke from **Settings → API Keys** (a connector grant is an `mo_` token).

## How it works

- **Discovery:** `/.well-known/oauth-protected-resource` (RFC 9728) → `/.well-known/oauth-authorization-server` (RFC 8414). The canonical resource is `<MDM_PUBLIC_URL>/mcp`, byte-identical across the metadata, the `resource` parameter, and the token's audience.
- **Registration:** the connector registers itself (RFC 7591 DCR — anonymous, IP-rate-limited; Claude/ChatGPT are public/PKCE clients, so no secret is issued).
- **Authorize → consent:** `/oauth/authorize` validates the client + exact redirect_uri + S256 PKCE, then 302s the browser to the web app's `/oauth/consent`. The signed-in user picks an org and approves; the API mints a **single-use, PKCE-bound code from the verified session user** — never from the request body.
- **Token:** `/oauth/token` exchanges the code (PKCE S256, exact redirect_uri, audience) for an opaque `mo_` access token (DB-backed, hashed with the API-key HMAC+pepper) plus a rotating refresh token. Reusing a rotated/revoked refresh token revokes the entire `(client, user, org)` family.
- **Resource server:** `/mcp` validates the `mo_` token by prefix + constant-time hash compare, checks expiry/revocation and audience, and **re-resolves the user's org membership on every call** (so the token dies if they lose the org). The org is intrinsic to the token — the `X-Org-Id` switcher header is ignored for connector tokens.

## Alternative: external Logto

Leave `MDM_OAUTH_MODE` off (or `logto`) and set `MDM_OAUTH_ISSUER` / `MDM_OAUTH_JWKS_URL` /
`MDM_OAUTH_AUDIENCE` to validate RS256 JWTs from a Logto instance instead. See
[oauth-logto.md](oauth-logto.md).
