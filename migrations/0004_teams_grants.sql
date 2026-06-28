-- Teams and the grant tables that layer per-project / per-document access on top of the
-- org base role. A grant's subject is a user or a team. A document grant with role 'none'
-- is an explicit DENY. Effective role is resolved in mdm_core::rbac::resolve_doc_role
-- (most-permissive accumulation, deny-veto unless owner/admin, org-viewer ceiling).

CREATE TABLE teams (
    id         uuid PRIMARY KEY,
    org_id     uuid NOT NULL REFERENCES organizations(id),
    slug       text NOT NULL,
    name       text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX teams_org_slug_uniq ON teams (org_id, slug);

CREATE TABLE team_members (
    org_id     uuid NOT NULL REFERENCES organizations(id),
    team_id    uuid NOT NULL REFERENCES teams(id),
    user_id    uuid NOT NULL REFERENCES users(id),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (team_id, user_id)
);
CREATE INDEX team_members_user_idx ON team_members (user_id);

CREATE TABLE project_grants (
    id           uuid PRIMARY KEY,
    org_id       uuid NOT NULL REFERENCES organizations(id),
    project_id   uuid NOT NULL REFERENCES projects(id),
    subject_type text NOT NULL CHECK (subject_type IN ('user','team')),
    subject_id   uuid NOT NULL,
    role         text NOT NULL CHECK (role IN ('viewer','commenter','editor','admin')),
    created_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (project_id, subject_type, subject_id)
);
CREATE INDEX project_grants_project_idx ON project_grants (project_id);

CREATE TABLE document_grants (
    id           uuid PRIMARY KEY,
    org_id       uuid NOT NULL REFERENCES organizations(id),
    document_id  uuid NOT NULL REFERENCES documents(id),
    subject_type text NOT NULL CHECK (subject_type IN ('user','team')),
    subject_id   uuid NOT NULL,
    role         text NOT NULL CHECK (role IN ('none','viewer','commenter','editor','admin')),
    created_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (document_id, subject_type, subject_id)
);
CREATE INDEX document_grants_document_idx ON document_grants (document_id);

-- RLS: strict org isolation on every new table.
ALTER TABLE teams           ENABLE ROW LEVEL SECURITY;
ALTER TABLE teams           FORCE  ROW LEVEL SECURITY;
CREATE POLICY org_isolation ON teams
    USING (org_id = current_org_id()) WITH CHECK (org_id = current_org_id());

ALTER TABLE team_members    ENABLE ROW LEVEL SECURITY;
ALTER TABLE team_members    FORCE  ROW LEVEL SECURITY;
CREATE POLICY org_isolation ON team_members
    USING (org_id = current_org_id()) WITH CHECK (org_id = current_org_id());

ALTER TABLE project_grants  ENABLE ROW LEVEL SECURITY;
ALTER TABLE project_grants  FORCE  ROW LEVEL SECURITY;
CREATE POLICY org_isolation ON project_grants
    USING (org_id = current_org_id()) WITH CHECK (org_id = current_org_id());

ALTER TABLE document_grants ENABLE ROW LEVEL SECURITY;
ALTER TABLE document_grants FORCE  ROW LEVEL SECURITY;
CREATE POLICY org_isolation ON document_grants
    USING (org_id = current_org_id()) WITH CHECK (org_id = current_org_id());

GRANT SELECT, INSERT, UPDATE, DELETE ON teams TO md_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON team_members TO md_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON project_grants TO md_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON document_grants TO md_app;
