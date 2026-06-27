-- md-manager initial schema.
--
-- Tenancy: `organizations` is the tenant boundary. Every tenant-data table carries
-- org_id and is protected by row-level security keyed on `current_org_id()`, which reads
-- the `app.current_org_id` GUC set per-transaction by the Rust `TenantDb` wrapper. An
-- unset GUC yields NULL => zero rows (fail closed).
--
-- `users` and `api_keys` are intentionally NOT under RLS: users are global identities,
-- and API-key authentication must look a key up by prefix *before* an org is known
-- (the key determines the org). The service enforces org scoping on api_keys management
-- queries explicitly. See docs/PLAN.md §3 and CLAUDE.md.
--
-- Migrations run as the owner/migrator role (md_owner); the app runs as md_app
-- (non-owner, NOBYPASSRLS), so RLS always applies to the app.

-- Resolve the current tenant from the per-transaction GUC (NULL when unset => no rows).
CREATE OR REPLACE FUNCTION current_org_id() RETURNS uuid
    LANGUAGE sql STABLE
    AS $$ SELECT NULLIF(current_setting('app.current_org_id', true), '')::uuid $$;

-- ---------------------------------------------------------------------------
-- Identity (global; no RLS)
-- ---------------------------------------------------------------------------
CREATE TABLE users (
    id           uuid PRIMARY KEY,
    email        text NOT NULL UNIQUE,
    display_name text NOT NULL,
    created_at   timestamptz NOT NULL DEFAULT now()
);

-- ---------------------------------------------------------------------------
-- Tenant root + membership
-- ---------------------------------------------------------------------------
CREATE TABLE organizations (
    id         uuid PRIMARY KEY,
    slug       text NOT NULL UNIQUE,
    name       text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    deleted_at timestamptz
);

CREATE TABLE organization_members (
    org_id     uuid NOT NULL REFERENCES organizations(id),
    user_id    uuid NOT NULL REFERENCES users(id),
    role       text NOT NULL CHECK (role IN ('owner','admin','member','viewer')),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (org_id, user_id)
);

-- ---------------------------------------------------------------------------
-- Projects (document containers)
-- ---------------------------------------------------------------------------
CREATE TABLE projects (
    id         uuid PRIMARY KEY,
    org_id     uuid NOT NULL REFERENCES organizations(id),
    slug       text NOT NULL,
    name       text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    deleted_at timestamptz
);
CREATE UNIQUE INDEX projects_org_slug_uniq ON projects(org_id, slug) WHERE deleted_at IS NULL;

-- ---------------------------------------------------------------------------
-- Documents + full-snapshot version history
-- ---------------------------------------------------------------------------
CREATE TABLE documents (
    id              uuid PRIMARY KEY,
    org_id          uuid NOT NULL REFERENCES organizations(id),
    project_id      uuid NOT NULL REFERENCES projects(id),
    path            text NOT NULL,
    title           text NOT NULL,
    content         text NOT NULL,
    content_hash    text NOT NULL,
    current_version bigint NOT NULL DEFAULT 1,
    created_by      uuid NOT NULL REFERENCES users(id),
    updated_by      uuid NOT NULL REFERENCES users(id),
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz NOT NULL DEFAULT now(),
    deleted_at      timestamptz,
    CHECK (octet_length(content) <= 10485760)  -- 10MB hard ceiling; app enforces configured max
);
CREATE UNIQUE INDEX documents_project_path_uniq ON documents(project_id, path) WHERE deleted_at IS NULL;
CREATE INDEX documents_project_idx ON documents(project_id) WHERE deleted_at IS NULL;

CREATE TABLE document_versions (
    id           uuid PRIMARY KEY,
    org_id       uuid NOT NULL REFERENCES organizations(id),
    document_id  uuid NOT NULL REFERENCES documents(id),
    version      bigint NOT NULL,
    content      text NOT NULL,
    content_hash text NOT NULL,
    version_kind text NOT NULL CHECK (version_kind IN ('checkpoint','autosave')),
    actor_type   text NOT NULL CHECK (actor_type IN ('user','agent')),
    actor_id     uuid NOT NULL,
    created_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (document_id, version)
);
CREATE INDEX document_versions_doc_idx ON document_versions(document_id, version DESC);

-- ---------------------------------------------------------------------------
-- Tags (org-scoped, cross-project)
-- ---------------------------------------------------------------------------
CREATE TABLE tags (
    id         uuid PRIMARY KEY,
    org_id     uuid NOT NULL REFERENCES organizations(id),
    name       text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (org_id, name)
);
CREATE TABLE document_tags (
    org_id      uuid NOT NULL REFERENCES organizations(id),
    document_id uuid NOT NULL REFERENCES documents(id),
    tag_id      uuid NOT NULL REFERENCES tags(id),
    PRIMARY KEY (document_id, tag_id)
);

-- ---------------------------------------------------------------------------
-- Full-text search chunks (header-aware; keyword FTS in MVP)
-- ---------------------------------------------------------------------------
CREATE TABLE doc_chunks (
    id           uuid PRIMARY KEY,
    org_id       uuid NOT NULL REFERENCES organizations(id),
    document_id  uuid NOT NULL REFERENCES documents(id),
    chunk_index  int NOT NULL,
    heading_path text NOT NULL DEFAULT '',
    content      text NOT NULL,
    tsv          tsvector GENERATED ALWAYS AS
                 (to_tsvector('english', coalesce(heading_path,'') || ' ' || content)) STORED,
    UNIQUE (document_id, chunk_index)
);
CREATE INDEX doc_chunks_tsv_idx ON doc_chunks USING GIN (tsv);
CREATE INDEX doc_chunks_doc_idx ON doc_chunks(document_id);

-- ---------------------------------------------------------------------------
-- API keys (system table; NO RLS — auth lookup is cross-org by design)
-- ---------------------------------------------------------------------------
CREATE TABLE api_keys (
    id           uuid PRIMARY KEY,
    org_id       uuid NOT NULL REFERENCES organizations(id),
    name         text NOT NULL,
    key_prefix   text NOT NULL,
    key_hash     text NOT NULL,
    role         text NOT NULL CHECK (role IN ('owner','admin','member','viewer')),
    created_by   uuid NOT NULL REFERENCES users(id),
    created_at   timestamptz NOT NULL DEFAULT now(),
    last_used_at timestamptz,
    revoked_at   timestamptz,
    expires_at   timestamptz
);
CREATE INDEX api_keys_prefix_idx ON api_keys(key_prefix);
CREATE INDEX api_keys_org_idx ON api_keys(org_id);

-- ---------------------------------------------------------------------------
-- Audit log
-- ---------------------------------------------------------------------------
CREATE TABLE audit_log (
    id         uuid PRIMARY KEY,
    org_id     uuid NOT NULL REFERENCES organizations(id),
    actor_type text NOT NULL,
    actor_id   uuid NOT NULL,
    action     text NOT NULL,
    target     text,
    metadata   jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX audit_log_org_idx ON audit_log(org_id, created_at DESC);

-- ---------------------------------------------------------------------------
-- Row-level security: strict org isolation on every tenant-data table.
-- ---------------------------------------------------------------------------
ALTER TABLE organizations        ENABLE ROW LEVEL SECURITY;
ALTER TABLE organizations        FORCE  ROW LEVEL SECURITY;
CREATE POLICY org_isolation ON organizations
    USING (id = current_org_id()) WITH CHECK (id = current_org_id());

ALTER TABLE organization_members ENABLE ROW LEVEL SECURITY;
ALTER TABLE organization_members FORCE  ROW LEVEL SECURITY;
CREATE POLICY org_isolation ON organization_members
    USING (org_id = current_org_id()) WITH CHECK (org_id = current_org_id());

ALTER TABLE projects             ENABLE ROW LEVEL SECURITY;
ALTER TABLE projects             FORCE  ROW LEVEL SECURITY;
CREATE POLICY org_isolation ON projects
    USING (org_id = current_org_id()) WITH CHECK (org_id = current_org_id());

ALTER TABLE documents            ENABLE ROW LEVEL SECURITY;
ALTER TABLE documents            FORCE  ROW LEVEL SECURITY;
CREATE POLICY org_isolation ON documents
    USING (org_id = current_org_id()) WITH CHECK (org_id = current_org_id());

ALTER TABLE document_versions    ENABLE ROW LEVEL SECURITY;
ALTER TABLE document_versions    FORCE  ROW LEVEL SECURITY;
CREATE POLICY org_isolation ON document_versions
    USING (org_id = current_org_id()) WITH CHECK (org_id = current_org_id());

ALTER TABLE tags                 ENABLE ROW LEVEL SECURITY;
ALTER TABLE tags                 FORCE  ROW LEVEL SECURITY;
CREATE POLICY org_isolation ON tags
    USING (org_id = current_org_id()) WITH CHECK (org_id = current_org_id());

ALTER TABLE document_tags        ENABLE ROW LEVEL SECURITY;
ALTER TABLE document_tags        FORCE  ROW LEVEL SECURITY;
CREATE POLICY org_isolation ON document_tags
    USING (org_id = current_org_id()) WITH CHECK (org_id = current_org_id());

ALTER TABLE doc_chunks           ENABLE ROW LEVEL SECURITY;
ALTER TABLE doc_chunks           FORCE  ROW LEVEL SECURITY;
CREATE POLICY org_isolation ON doc_chunks
    USING (org_id = current_org_id()) WITH CHECK (org_id = current_org_id());

ALTER TABLE audit_log            ENABLE ROW LEVEL SECURITY;
ALTER TABLE audit_log            FORCE  ROW LEVEL SECURITY;
CREATE POLICY org_isolation ON audit_log
    USING (org_id = current_org_id()) WITH CHECK (org_id = current_org_id());

-- ---------------------------------------------------------------------------
-- Privileges for the app runtime role.
-- ---------------------------------------------------------------------------
GRANT USAGE ON SCHEMA public TO md_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO md_app;
