-- Public, read-only, expiring share links for a single document.
--
-- Like `api_keys`, this is intentionally NOT under RLS: resolving a link looks the token up
-- by prefix *before* an org is known (the token determines the org + document). The service
-- scopes management queries (list/revoke) by org_id explicitly; the public resolve path
-- reads the linked document under that org's RLS scope. Tokens are stored as
-- HMAC-SHA256(pepper, token) + a lookup prefix, shown once at creation. Read-only (viewer).

CREATE TABLE share_links (
    id           uuid PRIMARY KEY,
    org_id       uuid NOT NULL REFERENCES organizations(id),
    document_id  uuid NOT NULL REFERENCES documents(id),
    token_prefix text NOT NULL,
    token_hash   text NOT NULL,
    created_by   uuid NOT NULL REFERENCES users(id),
    created_at   timestamptz NOT NULL DEFAULT now(),
    expires_at   timestamptz,
    revoked_at   timestamptz
);
CREATE INDEX share_links_prefix_idx ON share_links (token_prefix);
CREATE INDEX share_links_document_idx ON share_links (document_id);
CREATE INDEX share_links_org_idx ON share_links (org_id);

GRANT SELECT, INSERT, UPDATE, DELETE ON share_links TO md_app;
