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
    // Optional post-login destination (e.g. the OAuth consent page). Same-origin paths only.
    const nextParam = req.nextUrl.searchParams.get("next");
    const safeNext =
      nextParam && nextParam.startsWith("/") && !nextParam.startsWith("//") ? nextParam : null;

    const state = crypto.randomUUID();
    const jar = await cookies();
    const cookieOpts = {
      httpOnly: true,
      sameSite: "lax" as const,
      path: "/",
      secure: process.env.NODE_ENV === "production",
      maxAge: 600,
    };
    jar.set("g_state", state, cookieOpts);
    if (safeNext) {
      jar.set("g_next", safeNext, cookieOpts);
    } else {
      jar.delete("g_next");
    }
    return NextResponse.redirect(authUrl(`${origin}/auth/callback`, state));
  } catch {
    return NextResponse.redirect(new URL("/login?error=not_configured", origin));
  }
}
