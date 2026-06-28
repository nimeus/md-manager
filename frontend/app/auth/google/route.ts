import { cookies } from "next/headers";
import { NextResponse, type NextRequest } from "next/server";

import { authUrl } from "@/lib/google-oauth";

/**
 * Start "Sign in with Google": set a short-lived CSRF `state` cookie and redirect to Google's
 * consent screen. The redirect URI is derived from this request's origin, so it works on
 * localhost and in production without extra config (register both in the Google console).
 */
export async function GET(req: NextRequest) {
  const origin = new URL(req.url).origin;
  try {
    const state = crypto.randomUUID();
    (await cookies()).set("g_state", state, {
      httpOnly: true,
      sameSite: "lax",
      path: "/",
      secure: process.env.NODE_ENV === "production",
      maxAge: 600,
    });
    return NextResponse.redirect(authUrl(`${origin}/auth/callback`, state));
  } catch {
    return NextResponse.redirect(new URL("/login?error=not_configured", origin));
  }
}
