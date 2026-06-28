import { NextResponse, type NextRequest } from "next/server";

import { setCurrentOrg } from "@/lib/session";

/**
 * Switch the active org (org switcher). A GET route handler so plain links work and so the
 * app layout can repair a stale org cookie via redirect. Only sets the cookie; the backend
 * still authorizes membership on every request (a non-member org id is rejected there).
 */
export async function GET(req: NextRequest) {
  const origin = new URL(req.url).origin;
  const org = new URL(req.url).searchParams.get("org");
  if (org) await setCurrentOrg(org);
  return NextResponse.redirect(new URL("/projects", origin));
}
