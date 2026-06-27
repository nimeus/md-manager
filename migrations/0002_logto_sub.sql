-- Link local users to Logto (OAuth) identities.
-- `logto_sub` is the Logto subject (the `sub` claim of an access token). Nullable so
-- API-key-only users (no web/OAuth login) don't need it; unique when present.

ALTER TABLE users ADD COLUMN logto_sub text;
CREATE UNIQUE INDEX users_logto_sub_uniq ON users (logto_sub) WHERE logto_sub IS NOT NULL;
