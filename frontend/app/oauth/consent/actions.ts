"use server";

import { redirect } from "next/navigation";

import { api } from "@/lib/api";

// Server actions are POST-only and Next.js verifies the Origin against the Host, so these are
// CSRF-safe. The consenting user's identity comes from their forwarded `mss_` session inside
// `api.*` (the API resolves it) — never from anything the browser supplies here.

export async function approveConsentAction(formData: FormData): Promise<void> {
  const requestId = String(formData.get("request_id") ?? "");
  const orgId = String(formData.get("org_id") ?? "");
  if (!requestId || !orgId) redirect("/projects");

  let redirectTo: string;
  try {
    const res = await api.approveOAuthConsent(requestId, orgId);
    redirectTo = res.redirect_to;
  } catch {
    redirect(`/oauth/consent?request_id=${encodeURIComponent(requestId)}&error=expired`);
  }
  redirect(redirectTo); // back to the client's redirect_uri with the code
}

export async function denyConsentAction(formData: FormData): Promise<void> {
  const requestId = String(formData.get("request_id") ?? "");
  let redirectTo = "/projects";
  try {
    const res = await api.denyOAuthConsent(requestId);
    redirectTo = res.redirect_to; // back to the client with ?error=access_denied
  } catch {
    redirectTo = "/projects";
  }
  redirect(redirectTo);
}
