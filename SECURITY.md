# Security Policy

## Reporting a vulnerability

Please **do not** open a public issue for security problems.

Report vulnerabilities privately via
[GitHub private vulnerability reporting](https://github.com/nimeus/md-manager/security/advisories/new)
(Security tab → "Report a vulnerability"). You'll get a response as soon as
possible, typically within a few days.

Please include a description of the issue, steps to reproduce, and the impact
you believe it has (e.g. cross-tenant data access, privilege escalation,
token leakage).

## Scope notes

- Tenant isolation is enforced with Postgres row-level security; anything that
  lets one organization read or write another organization's data is the
  highest-severity class of bug here.
- API keys and share tokens are stored only as HMAC-SHA256 hashes with a
  server-side pepper. The RSA key in `apps/api/tests/fixtures/test_key.pem` is
  a throwaway fixture used to sign JWTs in unit tests — it protects nothing
  and is committed intentionally.

## Supported versions

The project is pre-1.0; only the latest `main` is supported.
