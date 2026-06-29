import { NextResponse, type NextRequest } from "next/server";

/**
 * Coarse auth guard: redirect to /login when the session cookie is absent. The actual
 * credential is validated server-side on every API call (a stale key surfaces as 401).
 */
export function middleware(request: NextRequest) {
  const signedIn = request.cookies.has("mdm_session");
  const { pathname } = request.nextUrl;

  // Public surface: landing page, docs, login, and the Google OAuth routes (which must run
  // while signed-out to create the session). Everything else requires a session.
  const isPublic =
    pathname === "/" ||
    pathname === "/login" ||
    pathname.startsWith("/docs") ||
    pathname.startsWith("/auth/") ||
    // The OAuth consent + invite-accept pages self-guard (they redirect to /login?next=… when
    // signed out), so they must not be force-redirected here — that would drop their token.
    pathname.startsWith("/oauth/") ||
    pathname.startsWith("/invite/");

  if (!signedIn && !isPublic) {
    return NextResponse.redirect(new URL("/login", request.url));
  }
  if (signedIn && pathname === "/login") {
    return NextResponse.redirect(new URL("/projects", request.url));
  }
  return NextResponse.next();
}

export const config = {
  // Run on everything except Next internals and static assets.
  matcher: ["/((?!_next/static|_next/image|favicon.ico).*)"],
};
