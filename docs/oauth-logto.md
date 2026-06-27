# Web connectors via OAuth 2.1 + self-hosted Logto

How Claude.ai and ChatGPT connectors authenticate to md-manager's **remote MCP** endpoint.

## Architecture

```
Claude.ai / ChatGPT ──(1) POST /mcp (no token)──► md-manager API  ──401 + WWW-Authenticate──►
                    ──(2) GET /.well-known/oauth-protected-resource──► { authorization_servers: [Logto] }
                    ──(3) OAuth 2.1 (PKCE, DCR) with Logto────────────► Logto issues JWT (aud = our /mcp URL)
                    ──(4) POST /mcp  Authorization: Bearer <JWT>─────► API validates (JWKS, iss, exp, aud) → tools
```

- **md-manager API = OAuth 2.1 _resource server_ only.** It validates tokens; it never issues them. (Built & tested: `apps/api/src/oauth.rs`, `apps/api/src/mcp.rs`.)
- **Logto = the _authorization server_.** It owns login, consent, Dynamic Client Registration, and JWKS. (Self-hosted; you run it.)

**What's built and verified here:** the Streamable-HTTP `/mcp` transport, the protected-resource-metadata endpoint, the `WWW-Authenticate` challenge, JWKS-based RS256 validation with `iss`/`exp`/**`aud`** checks (unit-tested in `oauth.rs`), and dual auth (API key *or* JWT).

**What's external (this doc):** running Logto, exposing the API over public HTTPS, and the live connector handshake.

## 1. Run Logto

```bash
docker compose -f docker-compose.logto.yml up -d
open http://localhost:3002        # admin console; create the first admin account
```
(Logto needs Docker. If Docker isn't available, deploy Logto elsewhere — the API only needs its issuer + JWKS URLs.)

## 2. Configure Logto

1. **API resource** → create one whose **identifier** is the *canonical resource URI* of your MCP endpoint, e.g. `https://md.example.com` (must match `MDM_OAUTH_AUDIENCE` and the `resource` in discovery **byte-for-byte**).
2. **Organizations** → enable; map each Logto organization to an md-manager `organizations.id`. Configure the access token to include an **`org` claim** carrying that org id (the API reads `MDM_OAUTH_ORG_CLAIM`, default `org`).
3. **Third-party / connector apps** → ensure **Dynamic Client Registration** is enabled (Claude/ChatGPT register themselves) and the consent screen is on.
4. Note the OIDC endpoints: issuer `http://localhost:3001/oidc`, JWKS `http://localhost:3001/oidc/jwks` (swap in your public Logto host for production).

## 3. Point md-manager at Logto

```bash
MDM_PUBLIC_URL=https://md.example.com
MDM_OAUTH_ISSUER=https://logto.example.com/oidc
MDM_OAUTH_JWKS_URL=https://logto.example.com/oidc/jwks
MDM_OAUTH_AUDIENCE=https://md.example.com     # == the Logto API resource identifier
MDM_OAUTH_ORG_CLAIM=org
```
When all three `MDM_OAUTH_*` are set, the API enables JWT auth on `/mcp` and serves discovery. (Unset ⇒ API-key-only; `/.well-known/oauth-protected-resource` returns 404.)

## 4. Link identities

A validated token's `sub` (Logto user) must map to a local user, and its `org` claim to an org the user belongs to:
- Set `users.logto_sub` (helper: `Db::link_logto_sub`; a self-serve account-linking flow is a TODO).
- The user must be a current member of the claimed org (enforced by `Db::authenticate_oauth`).
- **JIT provisioning** of brand-new Logto users/orgs is deferred until the Logto org model is wired — until then, link existing users.

## 5. Expose over HTTPS + connect

Connectors require **HTTPS**. Use a tunnel for testing (`cloudflared tunnel --url http://localhost:8080`) or deploy. Then in Claude.ai / ChatGPT, add the connector URL `https://md.example.com/mcp`.

### Launch gotchas (verify before going live)
- **Allowlist callback URLs**: Claude `https://claude.ai/api/mcp/auth_callback` **and** `https://claude.com/api/mcp/auth_callback`; ChatGPT `https://chatgpt.com/connector/oauth/<id>`.
- **Audience byte-match**: the `resource` in discovery, the RFC 8707 `resource` param, and the API's `MDM_OAUTH_AUDIENCE` must be identical. (Mismatch ⇒ silent 401.)
- **DCR responses** must not contain empty-string/null URL fields (breaks Claude's strict schema).
- **Refresh errors** must use RFC 6749 codes (`invalid_grant`), not custom ones.
- **SLAs**: discovery/register/token ≤ 10s, refresh ≤ 30s — keep JWKS cached/warm.
- **WAF/proxy** must permit the `Authorization` header and Anthropic/OpenAI outbound.

## Local verification without a connector

- `cargo test -p mdm-api` exercises RS256 validation (valid accept, wrong-`aud` reject, expired reject).
- With OAuth env set, `GET /.well-known/oauth-protected-resource` returns the metadata, and `POST /mcp` without a token returns `401` + `WWW-Authenticate`.
- The full tool surface is reachable over `/mcp` with an **API key** today (no Logto needed) — see `README` / `CLAUDE.md`.
