import { cookies } from "next/headers";
import { NextResponse, type NextRequest } from "next/server";

import { exchangeGoogleToken } from "@/lib/api";
import { exchangeCodeForIdToken } from "@/lib/google-oauth";
import { setSession } from "@/lib/session";

/**
 * Google redirects here with an authorization code. We verify the CSRF `state`, exchange the
 * code for Google's ID token, hand that to the backend (`/v1/auth/google`) which verifies it
 * and provisions the user + orgs, then store the backend session token in our httpOnly cookie.
 */
export async function GET(req: NextRequest) {
  const url = new URL(req.url);
  const origin = url.origin;
  const fail = (msg: string) =>
    NextResponse.redirect(new URL(`/login?error=${encodeURIComponent(msg)}`, origin));

  const code = url.searchParams.get("code");
  const state = url.searchParams.get("state");
  const oauthErr = url.searchParams.get("error");

  const jar = await cookies();
  const expected = jar.get("g_state")?.value;
  jar.delete("g_state");

  if (oauthErr) return fail(oauthErr);
  if (!code || !state || !expected || state !== expected) return fail("invalid_state");

  try {
    const idToken = await exchangeCodeForIdToken(code, `${origin}/auth/callback`);
    const ex = await exchangeGoogleToken(idToken);
    await setSession({
      token: ex.session_token,
      user: ex.user,
      currentOrg: ex.orgs[0]?.id ?? "",
    });
    return NextResponse.redirect(new URL("/projects", origin));
  } catch {
    return fail("signin_failed");
  }
}
