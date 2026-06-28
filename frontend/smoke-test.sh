#!/usr/bin/env bash
# Smoke test for the md-manager web app (BFF) against a running Rust API.
#
# Login goes through Google, so a pure-curl test can't sign in. This checks:
#   1) the Rust API is up,
#   2) the web server renders the "Sign in with Google" page,
#   3) (optional) if you pass the API's MDM_SESSION_SECRET, it bootstraps a tenant, seeds a
#      project, mints the exact session cookie the BFF uses (a backend session token + org),
#      and confirms the BFF renders that org's data — exercising the full server-side path.
#
# Usage:
#   ./smoke-test.sh
#   MDM_SESSION_SECRET=<same-as-API> ./smoke-test.sh
set -u

API_URL="${API_URL:-${MDM_API_URL:-http://127.0.0.1:8080}}"
WEB_URL="${WEB_URL:-http://localhost:3000}"
BOOTSTRAP_TOKEN="${BOOTSTRAP_TOKEN:-${MDM_ADMIN_BOOTSTRAP_TOKEN:-dev-bootstrap-token}}"
SESSION_SECRET="${MDM_SESSION_SECRET:-}"
PASS=0; FAIL=0
ok()  { echo "  ✓ $1"; PASS=$((PASS+1)); }
bad() { echo "  ✗ $1"; FAIL=$((FAIL+1)); }
die() { echo; echo "ABORTED: $1"; exit 1; }

echo "API: $API_URL   WEB: $WEB_URL"; echo

echo "[1/4] Rust API reachable"
[ "$(curl -s -m5 -o /dev/null -w '%{http_code}' "$API_URL/healthz")" = "200" ] \
  && ok "GET /healthz -> 200" || die "API not reachable at $API_URL. Start it first."

echo "[2/4] Web server renders the Google sign-in page"
login=$(curl -s -m5 "$WEB_URL/login")
echo "$login" | grep -q "Sign in with Google" \
  && ok "GET /login shows 'Sign in with Google'" \
  || die "Web app not serving (or login page changed) at $WEB_URL. Run 'npm run dev'."

if [ -z "$SESSION_SECRET" ]; then
  echo "[3/4] (skipped) full BFF render — set MDM_SESSION_SECRET=<same as API> to run it"
  echo "[4/4] (skipped)"
  echo; echo "RESULT: $PASS passed, $FAIL failed  (login flow itself needs a browser + Google)"
  exit "$FAIL"
fi

echo "[3/4] Bootstrap a tenant + seed a project via the API"
n=$(date +%s)
boot=$(curl -s -m10 -X POST "$API_URL/v1/bootstrap" \
  -H 'content-type: application/json' -H "x-bootstrap-token: $BOOTSTRAP_TOKEN" \
  -d "{\"email\":\"smoke$n@example.com\",\"display_name\":\"Smoke\",\"org_slug\":\"smoke$n\",\"org_name\":\"Smoke $n\",\"key_name\":\"smoke\"}")
# Parse with python (reliable across platforms; BSD/macOS sed mishandles the JSON braces).
read USERID ORGID KEY < <(printf '%s' "$boot" | python3 -c \
  'import sys,json;d=json.load(sys.stdin);print(d["user"]["id"],d["org"]["id"],d["api_key"]["secret"])' 2>/dev/null)
[ -n "$USERID" ] && [ -n "$KEY" ] || die "bootstrap failed (wrong token?). Response: $boot"
curl -s -m10 -X POST "$API_URL/v1/projects" -H "authorization: Bearer $KEY" \
  -H 'content-type: application/json' -d "{\"slug\":\"smoke$n\",\"name\":\"Smoke Project $n\"}" -o /dev/null
ok "seeded project 'Smoke Project $n' (user=$USERID)"

echo "[4/4] BFF renders org data with a real session cookie"
# Mint the backend session token (mss_ HS256) the way the API does, then the exact mdm_session
# cookie lib/session.ts writes: base64(JSON({token,user,currentOrg})).
COOKIE=$(python3 - "$USERID" "$ORGID" "$SESSION_SECRET" <<'PY'
import sys,json,hmac,hashlib,base64,time
uid,org,secret=sys.argv[1],sys.argv[2],sys.argv[3].encode()
b64=lambda b: base64.urlsafe_b64encode(b).rstrip(b'=').decode()
h=b64(json.dumps({"alg":"HS256","typ":"JWT"},separators=(',',':')).encode())
p=b64(json.dumps({"sub":uid,"typ":"mdm_session","iat":int(time.time()),"exp":int(time.time())+3600},separators=(',',':')).encode())
sig=b64(hmac.new(secret,f"{h}.{p}".encode(),hashlib.sha256).digest())
token=f"mss_{h}.{p}.{sig}"
session={"token":token,"user":{"id":uid,"email":"smoke@example.com","name":"Smoke"},"currentOrg":org}
print(base64.b64encode(json.dumps(session).encode()).decode())
PY
)
page=$(curl -s -m10 -b "mdm_session=$COOKIE" "$WEB_URL/projects")
echo "$page" | grep -q "Smoke Project $n" \
  && ok "GET /projects renders the seeded project — full BFF->API path works" \
  || bad "project did not render — check MDM_API_URL on the Next server + that secrets match"

echo; echo "RESULT: $PASS passed, $FAIL failed"
exit "$FAIL"
