-- Web SaaS auth: Google sign-in (JIT users), multi-org sessions, and org invitations.

-- 1) Link a user to their Google account. The API verifies a Google ID token (RS256 against
--    Google's JWKS, aud = our client id) and stores the subject here; `users` is RLS-exempt.
ALTER TABLE users ADD COLUMN IF NOT EXISTS google_sub text UNIQUE;

-- 2) Multi-org listing WITHOUT weakening data isolation.
--    The web app must list the orgs a user belongs to (for the org switcher) — a cross-org
--    read the single-`current_org_id` model can't express. Add SELECT-only policies keyed on
--    a NEW `app.current_user_id` GUC, set only by `begin_user_scoped`. During normal
--    org-scoped requests that GUC is unset, so `current_user_id()` is NULL and these policies
--    match nothing (inert) — existing isolation is unchanged. Crucially, data tables
--    (documents, versions, chunks, …) get NO new policy, so a user-scoped session (org GUC
--    unset) still reads ZERO documents.
CREATE OR REPLACE FUNCTION current_user_id() RETURNS uuid
    LANGUAGE sql STABLE
    AS $$ SELECT NULLIF(current_setting('app.current_user_id', true), '')::uuid $$;

CREATE POLICY member_self_read ON organization_members FOR SELECT
    USING (user_id = current_user_id());
CREATE POLICY org_member_read ON organizations FOR SELECT
    USING (EXISTS (SELECT 1 FROM organization_members m
                   WHERE m.org_id = organizations.id AND m.user_id = current_user_id()));

-- 3) Org invitations: invite a teammate by email; they join on Google sign-in (verified-email
--    match) or via the invite link. RLS-exempt like api_keys/share_links — resolved cross-org
--    by email/token before the invitee is a member; management queries filter by org_id and
--    authorize via the caller's role. Token stored as HMAC-SHA256(pepper)+prefix (shown once).
CREATE TABLE org_invitations (
    id           uuid PRIMARY KEY,
    org_id       uuid NOT NULL REFERENCES organizations(id),
    email        text NOT NULL,
    role         text NOT NULL DEFAULT 'member' CHECK (role IN ('admin','member','viewer')),
    token_hash   text NOT NULL,
    token_prefix text NOT NULL,
    invited_by   uuid NOT NULL REFERENCES users(id),
    created_at   timestamptz NOT NULL DEFAULT now(),
    expires_at   timestamptz,
    accepted_at  timestamptz,
    revoked_at   timestamptz
);
CREATE INDEX org_invitations_email ON org_invitations (lower(email));
CREATE INDEX org_invitations_org_id ON org_invitations (org_id);
CREATE UNIQUE INDEX org_invitations_prefix ON org_invitations (token_prefix);
GRANT SELECT, INSERT, UPDATE, DELETE ON org_invitations TO md_app;
