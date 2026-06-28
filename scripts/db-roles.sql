-- One-time role setup for a MANAGED Postgres (e.g. the database Dokploy provisions).
--
-- md-manager uses two roles on purpose:
--   • md_owner  — owns the tables and runs migrations.
--   • md_app    — the RUNTIME role. It is a NON-OWNER, NOBYPASSRLS role, which is exactly
--                 what makes Postgres row-level security enforce tenant isolation. The API
--                 asserts at startup that md_app cannot bypass RLS and refuses to run if it can.
--
-- HOW TO RUN: connect to your app database (e.g. `md_manager`) as the superuser your managed
-- Postgres gave you, then paste this whole file. CHANGE THE TWO PASSWORDS first.

-- 1) Roles (created only if missing). ----------------------------------------
DO $$
BEGIN
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'md_owner') THEN
    CREATE ROLE md_owner LOGIN PASSWORD 'CHANGE_ME_owner_password';
  END IF;
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'md_app') THEN
    CREATE ROLE md_app LOGIN NOBYPASSRLS PASSWORD 'CHANGE_ME_app_password';
  END IF;
END
$$;

-- Belt-and-suspenders: ensure md_app can never bypass RLS even if it pre-existed.
ALTER ROLE md_app NOBYPASSRLS;

-- 2) Let md_owner own this database's schema so migrations can create tables. -
-- (Run while connected to the app database — `public` here is that database's schema.)
ALTER SCHEMA public OWNER TO md_owner;
GRANT USAGE ON SCHEMA public TO md_app;

-- That's it. On startup the API connects as md_owner to run the migrations (which create the
-- tables, RLS policies, and grant md_app the row-level DML it needs), then serves as md_app.
--
-- OPTIONAL — semantic search (pgvector). As a superuser, in the app database, run once:
--   CREATE EXTENSION IF NOT EXISTS vector;
