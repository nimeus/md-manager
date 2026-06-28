import { cookies } from "next/headers";
import { NextResponse, type NextRequest } from "next/server";

import { authUrl, publicOrigin } from "@/lib/google-oauth";

/**
 * Start "Sign in with Google": set a short-lived CSRF `state` cookie and redirect to Google's
 * consent screen. The redirect URI uses the app's PUBLIC origin (works behind a proxy).
 */
export async function GET(req: NextRequest) {
  const origin = publicOrigin(req);
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
