-- "All my organizations" connector tokens. When `all_orgs = true`, the bound `org_id` is only a
-- placeholder (the user's home org); the agent selects the org per call (an `org` argument on the
-- MCP tools), validated against the user's live membership each request. Existing rows default to
-- false (single-org — unchanged behavior).

ALTER TABLE oauth_auth_codes     ADD COLUMN all_orgs boolean NOT NULL DEFAULT false;
ALTER TABLE oauth_access_tokens  ADD COLUMN all_orgs boolean NOT NULL DEFAULT false;
ALTER TABLE oauth_refresh_tokens ADD COLUMN all_orgs boolean NOT NULL DEFAULT false;
