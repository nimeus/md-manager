-- Public/private share links. `audience` controls who may open a link:
--   public  — anyone with the link (read-only, unauthenticated)  [default; unchanged behavior]
--   members — any signed-in member of the document's org
--   emails  — only the allow-listed recipients (after signing in)
--
-- Like share_links, `share_link_recipients` is intentionally RLS-exempt: a link is resolved by
-- token (and recipients by share_link_id) before an org is known. Management is scoped via the
-- parent link's org in service code.

ALTER TABLE share_links
    ADD COLUMN audience text NOT NULL DEFAULT 'public'
        CHECK (audience IN ('public', 'members', 'emails'));

CREATE TABLE share_link_recipients (
    share_link_id uuid NOT NULL REFERENCES share_links(id) ON DELETE CASCADE,
    email         text NOT NULL,
    PRIMARY KEY (share_link_id, email)
);

GRANT SELECT, INSERT, UPDATE, DELETE ON share_link_recipients TO md_app;
