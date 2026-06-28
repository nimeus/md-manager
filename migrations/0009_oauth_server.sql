-- Built-in OAuth 2.1 Authorization Server (replaces the external-Logto plan).
--
-- These tables back the AS that lets Claude.ai / ChatGPT add the /mcp endpoint as a native
-- connector. Like `api_keys` / `share_links` they are intentionally NOT under RLS: OAuth
-- clients and pending authorization requests exist *before* any org is chosen, and tokens are
-- looked up by prefix (cross-org) before an org context exists. All secrets are stored as
-- HMAC-SHA256(pepper, secret) + a lookup prefix (shown once); the org/user context for codes
-- and tokens is bound explicitly into the row and the membership is re-checked on every use.

-- 1) Registered OAuth clients. Dynamic Client Registration (RFC 7591) creates these
--    anonymously; public PKCE clients (Claude/ChatGPT) carry a NULL secret. No org_id — a
--    client is just a label until a user consents and picks an org.
CREATE TABLE oauth_clients (
    id                   uuid PRIMARY KEY,
    client_id            text NOT NULL UNIQUE,
    client_secret_prefix text,            -- NULL for public (PKCE-only) clients
    client_secret_hash   text,            -- NULL for public clients
    name                 text NOT NULL,
    redirect_uris        text[] NOT NULL,
    grant_types          text[] NOT NULL DEFAULT ARRAY['authorization_code','refresh_token'],
    scopes               text NOT NULL DEFAULT 'mcp',
    is_dcr               boolean NOT NULL DEFAULT true,
    created_at           timestamptz NOT NULL DEFAULT now(),
    revoked_at           timestamptz
);
CREATE INDEX oauth_clients_client_id_idx ON oauth_clients (client_id);

-- 2) Pending authorization requests: created (validated) at /oauth/authorize, consumed when
--    the user approves consent. No user/org yet — those are decided at consent time.
CREATE TABLE oauth_authorization_requests (
    id                    uuid PRIMARY KEY,
    client_id             uuid NOT NULL REFERENCES oauth_clients(id),
    redirect_uri          text NOT NULL,
    code_challenge        text NOT NULL,
    code_challenge_method text NOT NULL,
    resource              text NOT NULL,
    scope                 text NOT NULL,
    state                 text,
    created_at            timestamptz NOT NULL DEFAULT now(),
    expires_at            timestamptz NOT NULL,
    consumed_at           timestamptz
);

-- 3) Authorization codes: single-use, short-lived, bound to (client,user,org,redirect,pkce).
CREATE TABLE oauth_auth_codes (
    id                    uuid PRIMARY KEY,
    code_prefix           text NOT NULL,
    code_hash             text NOT NULL,
    client_id             uuid NOT NULL REFERENCES oauth_clients(id),
    user_id               uuid NOT NULL REFERENCES users(id),
    org_id                uuid NOT NULL REFERENCES organizations(id),
    redirect_uri          text NOT NULL,
    code_challenge        text NOT NULL,
    code_challenge_method text NOT NULL,
    resource              text NOT NULL,
    scope                 text NOT NULL,
    created_at            timestamptz NOT NULL DEFAULT now(),
    expires_at            timestamptz NOT NULL,
    consumed_at           timestamptz
);
CREATE INDEX oauth_auth_codes_prefix_idx ON oauth_auth_codes (code_prefix);

-- 4) Access tokens: opaque (mo_…), validated by the /mcp resource server via prefix + hash.
CREATE TABLE oauth_access_tokens (
    id           uuid PRIMARY KEY,
    token_prefix text NOT NULL,
    token_hash   text NOT NULL,
    client_id    uuid NOT NULL REFERENCES oauth_clients(id),
    user_id      uuid NOT NULL REFERENCES users(id),
    org_id       uuid NOT NULL REFERENCES organizations(id),
    scope        text NOT NULL,
    resource     text NOT NULL,
    created_at   timestamptz NOT NULL DEFAULT now(),
    expires_at   timestamptz NOT NULL,
    last_used_at timestamptz,
    revoked_at   timestamptz
);
CREATE INDEX oauth_access_tokens_prefix_idx ON oauth_access_tokens (token_prefix);

-- 5) Refresh tokens: rotate on use; presenting a rotated/revoked one revokes the whole family
--    (theft response). `rotated_to` chains a token to its successor.
CREATE TABLE oauth_refresh_tokens (
    id              uuid PRIMARY KEY,
    token_prefix    text NOT NULL,
    token_hash      text NOT NULL,
    client_id       uuid NOT NULL REFERENCES oauth_clients(id),
    user_id         uuid NOT NULL REFERENCES users(id),
    org_id          uuid NOT NULL REFERENCES organizations(id),
    scope           text NOT NULL,
    resource        text NOT NULL,
    access_token_id uuid REFERENCES oauth_access_tokens(id),
    created_at      timestamptz NOT NULL DEFAULT now(),
    expires_at      timestamptz NOT NULL,
    revoked_at      timestamptz,
    rotated_to      uuid REFERENCES oauth_refresh_tokens(id)
);
CREATE INDEX oauth_refresh_tokens_prefix_idx ON oauth_refresh_tokens (token_prefix);
CREATE INDEX oauth_refresh_tokens_family_idx ON oauth_refresh_tokens (client_id, user_id, org_id);

GRANT SELECT, INSERT, UPDATE, DELETE ON
    oauth_clients, oauth_authorization_requests, oauth_auth_codes,
    oauth_access_tokens, oauth_refresh_tokens
    TO md_app;
