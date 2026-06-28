import { NextResponse, type NextRequest } from "next/server";

/**
 * Coarse auth guard: redirect to /login when the session cookie is absent. The actual
 * credential is validated server-side on every API call (a stale key surfaces as 401).
 */
export function middleware(request: NextRequest) {
  const signedIn = request.cookies.has("mdm_session");
  const { pathname } = request.nextUrl;

  if (!signedIn && pathname !== "/login") {
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
