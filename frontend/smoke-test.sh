#!/usr/bin/env bash
# End-to-end smoke test for the md-manager web app (BFF) against a running Rust API.
#
# It proves the whole chain works: the Next server renders a page, reads the httpOnly
# session cookie, calls the Rust API server-side with the bearer key, and shows the data.
#
# Prereqs (two servers running):
#   1) Rust API:  (repo root) MDM_ADMIN_BOOTSTRAP_TOKEN=dev-bootstrap-token cargo run -p mdm-api
#   2) Web app:   (frontend/) MDM_API_URL=http://127.0.0.1:8080 npm run build && npm run start
#
# Usage:
#   ./smoke-test.sh
#   API_URL=http://127.0.0.1:8080 WEB_URL=http://localhost:3000 BOOTSTRAP_TOKEN=dev-bootstrap-token ./smoke-test.sh
set -u

API_URL="${API_URL:-${MDM_API_URL:-http://127.0.0.1:8080}}"
WEB_URL="${WEB_URL:-http://localhost:3000}"
BOOTSTRAP_TOKEN="${BOOTSTRAP_TOKEN:-${MDM_BOOTSTRAP_TOKEN:-dev-bootstrap-token}}"
PASS=0; FAIL=0
ok()   { echo "  ✓ $1"; PASS=$((PASS+1)); }
bad()  { echo "  ✗ $1"; FAIL=$((FAIL+1)); }
die()  { echo; echo "ABORTED: $1"; exit 1; }

echo "API: $API_URL   WEB: $WEB_URL"
echo

echo "[1/5] Rust API reachable"
code=$(curl -s -m 5 -o /dev/null -w "%{http_code}" "$API_URL/healthz")
[ "$code" = "200" ] && ok "GET /healthz -> 200" || die "API not reachable at $API_URL (got $code). Start it first."

echo "[2/5] Web server reachable (login page renders)"
login=$(curl -s -m 5 "$WEB_URL/login")
echo "$login" | grep -q "Sign in" && ok "GET /login renders the sign-in page" || die "Web app not serving at $WEB_URL. Run 'npm run start' (or 'npm run dev')."

echo "[3/5] Bootstrap a tenant + key via the API"
n=$(date +%s)
boot=$(curl -s -m 10 -X POST "$API_URL/v1/bootstrap" \
  -H 'content-type: application/json' -H "x-bootstrap-token: $BOOTSTRAP_TOKEN" \
  -d "{\"email\":\"smoke$n@example.com\",\"display_name\":\"Smoke\",\"org_slug\":\"smoke$n\",\"org_name\":\"Smoke $n\",\"key_name\":\"smoke\"}")
KEY=$(printf '%s' "$boot" | sed -n 's/.*"secret":"\([^"]*\)".*/\1/p')
[ -n "$KEY" ] && ok "minted API key ${KEY:0:8}…" || die "bootstrap failed (wrong BOOTSTRAP_TOKEN?). Response: $boot"

echo "[4/5] Seed a project via the API"
AUTH="authorization: Bearer $KEY"
proj=$(curl -s -m 10 -X POST "$API_URL/v1/projects" -H "$AUTH" \
  -H 'content-type: application/json' -d "{\"slug\":\"smoke$n\",\"name\":\"Smoke Project $n\"}")
echo "$proj" | grep -q "\"id\"" && ok "created project 'Smoke Project $n'" || die "could not create project: $proj"

echo "[5/5] Web BFF renders API data (the real end-to-end check)"
# The session cookie is exactly what lib/session.ts writes: base64(JSON.stringify({apiKey})).
COOKIE=$(printf '{"apiKey":"%s"}' "$KEY" | base64 | tr -d '\n')
page=$(curl -s -m 10 -b "mdm_session=$COOKIE" "$WEB_URL/projects")
echo "$page" | grep -q "Smoke Project $n" \
  && ok "GET /projects (with session) shows the seeded project — BFF→API path works" \
  || bad "the project did not render — check MDM_API_URL on the Next server points at $API_URL"

echo
echo "RESULT: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] && echo "✅ web app is wired to the API correctly" || echo "❌ see failures above"
exit "$FAIL"
