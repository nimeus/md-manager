-- Categories: an org-scoped, hierarchical taxonomy that crosses projects (unlike a
-- project's documents). A document can be filed under many categories. Complements the
-- flat `tags` table.

CREATE TABLE categories (
    id         uuid PRIMARY KEY,
    org_id     uuid NOT NULL REFERENCES organizations(id),
    parent_id  uuid REFERENCES categories(id),
    slug       text NOT NULL,
    name       text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
-- Unique slug per (org, parent); COALESCE folds the NULL (root) parent so two roots
-- can't share a slug.
CREATE UNIQUE INDEX categories_org_parent_slug_uniq
    ON categories (org_id, COALESCE(parent_id, '00000000-0000-0000-0000-000000000000'::uuid), slug);
CREATE INDEX categories_parent_idx ON categories (parent_id);

CREATE TABLE document_categories (
    org_id      uuid NOT NULL REFERENCES organizations(id),
    document_id uuid NOT NULL REFERENCES documents(id),
    category_id uuid NOT NULL REFERENCES categories(id),
    PRIMARY KEY (document_id, category_id)
);
CREATE INDEX document_categories_category_idx ON document_categories (category_id);

ALTER TABLE categories          ENABLE ROW LEVEL SECURITY;
ALTER TABLE categories          FORCE  ROW LEVEL SECURITY;
CREATE POLICY org_isolation ON categories
    USING (org_id = current_org_id()) WITH CHECK (org_id = current_org_id());

ALTER TABLE document_categories ENABLE ROW LEVEL SECURITY;
ALTER TABLE document_categories FORCE  ROW LEVEL SECURITY;
CREATE POLICY org_isolation ON document_categories
    USING (org_id = current_org_id()) WITH CHECK (org_id = current_org_id());

GRANT SELECT, INSERT, UPDATE, DELETE ON categories TO md_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON document_categories TO md_app;
