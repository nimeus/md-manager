/**
 * Minimal Google OAuth 2.0 authorization-code flow, server-side (the BFF is a confidential
 * client). We only need Google's ID token once — we exchange it for our own backend session
 * token — so there's no refresh-token handling. The client secret stays on the server.
 */

const GOOGLE_AUTH = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN = "https://oauth2.googleapis.com/token";

/**
 * The app's PUBLIC origin (e.g. `https://mdm.example.com`), used for the OAuth `redirect_uri`
 * and post-login redirects. Behind a reverse proxy the Next server only sees its internal bind
 * address, so we resolve it from a TRUSTED source:
 *  1. `MDM_APP_URL` — authoritative, not attacker-controllable. Set this in production.
 *  2. `X-Forwarded-Host` — only when it matches `MDM_ALLOWED_HOSTS`, so a forged header can't
 *     redirect the flow to an arbitrary host (host-header injection / open redirect).
 *  3. the request URL — correct for local dev with no proxy.
 */
export function publicOrigin(req: Request): string {
  const env = process.env.MDM_APP_URL?.replace(/\/+$/, "");
  if (env) return env;

  const allowed = (process.env.MDM_ALLOWED_HOSTS ?? "")
    .split(",")
    .map((h) => h.trim().toLowerCase())
    .filter(Boolean);
  const fwdHost = req.headers.get("x-forwarded-host")?.split(",")[0].trim();
  if (fwdHost && allowed.includes(fwdHost.toLowerCase())) {
    const rawProto = req.headers.get("x-forwarded-proto")?.split(",")[0].trim().toLowerCase();
    const proto = rawProto === "http" || rawProto === "https" ? rawProto : "https";
    return `${proto}://${fwdHost}`;
  }

  // Local dev (no proxy): the request URL is correct.
  return new URL(req.url).origin;
}

function config() {
  const clientId = process.env.MDM_GOOGLE_CLIENT_ID;
  const clientSecret = process.env.MDM_GOOGLE_CLIENT_SECRET;
  if (!clientId || !clientSecret) {
    throw new Error(
      "Google sign-in is not configured (set MDM_GOOGLE_CLIENT_ID + MDM_GOOGLE_CLIENT_SECRET)",
    );
  }
  return { clientId, clientSecret };
}

/** Build the Google consent URL to redirect the user to. */
export function authUrl(redirectUri: string, state: string): string {
  const { clientId } = config();
  const params = new URLSearchParams({
    client_id: clientId,
    redirect_uri: redirectUri,
    response_type: "code",
    scope: "openid email profile",
    state,
    access_type: "online",
    prompt: "select_account",
  });
  return `${GOOGLE_AUTH}?${params.toString()}`;
}

/** Exchange an authorization code for the Google ID token (a verified JWT of the user). */
export async function exchangeCodeForIdToken(code: string, redirectUri: string): Promise<string> {
  const { clientId, clientSecret } = config();
  const res = await fetch(GOOGLE_TOKEN, {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({
      code,
      client_id: clientId,
      client_secret: clientSecret,
      redirect_uri: redirectUri,
      grant_type: "authorization_code",
    }),
    cache: "no-store",
  });
  if (!res.ok) {
    throw new Error(`Google token exchange failed: ${res.status} ${await res.text()}`);
  }
  const data = (await res.json()) as { id_token?: string };
  if (!data.id_token) throw new Error("Google token response had no id_token");
  return data.id_token;
}
