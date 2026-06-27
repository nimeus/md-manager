#!/usr/bin/env bash
# Idempotent local Postgres setup for md-manager: two roles + dev/test databases.
# Requires Postgres 17 (Homebrew) running: `brew services start postgresql@17`.
set -euo pipefail

export PATH="/opt/homebrew/opt/postgresql@17/bin:${PATH}"

echo "==> waiting for Postgres..."
for _ in $(seq 1 30); do pg_isready -h localhost -q && break; sleep 1; done
pg_isready -h localhost

echo "==> creating roles md_owner / md_app (if missing)"
psql -d postgres -v ON_ERROR_STOP=1 <<'SQL'
DO $$ BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='md_owner') THEN
    CREATE ROLE md_owner LOGIN PASSWORD 'md_owner_dev' NOSUPERUSER NOBYPASSRLS CREATEDB;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='md_app') THEN
    CREATE ROLE md_app LOGIN PASSWORD 'md_app_dev' NOSUPERUSER NOBYPASSRLS;
  END IF;
END $$;
SQL

echo "==> creating databases md_manager / md_manager_test (owned by md_owner)"
for db in md_manager md_manager_test; do
  if ! psql -d postgres -tAc "SELECT 1 FROM pg_database WHERE datname='${db}'" | grep -q 1; then
    createdb -O md_owner "${db}"
    echo "    created ${db}"
  else
    echo "    ${db} already exists"
  fi
done

echo "==> done. The API runs migrations on startup (as md_owner)."
echo "    Set MDM_DATABASE_URL / MDM_MIGRATION_DATABASE_URL (see .env.example)."
