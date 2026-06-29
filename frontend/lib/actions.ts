"use server";

import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";

import { api, ApiError, type UpdateResult } from "./api";
import { clearSession, setCurrentOrg } from "./session";

export async function logoutAction(): Promise<void> {
  await clearSession();
  redirect("/login");
}

// --- orgs + invitations ------------------------------------------------------
// (Org switching is a GET route handler — app/auth/switch — so plain links work and the
//  cookie can be repaired during layout render via redirect.)

/** Create a new org (caller becomes owner) and switch into it. */
export async function createOrgAction(formData: FormData): Promise<void> {
  const slug = String(formData.get("slug") ?? "").trim();
  const name = String(formData.get("name") ?? "").trim();
  const org = await api.createOrg(slug, name);
  await setCurrentOrg(org.id);
  redirect("/projects");
}

/** Invite a teammate by email to the current org. */
export async function inviteAction(
  _prev: { ok?: string; error?: string; token?: string } | null,
  formData: FormData,
): Promise<{ ok?: string; error?: string; token?: string } | null> {
  const email = String(formData.get("email") ?? "").trim();
  const role = String(formData.get("role") ?? "member");
  if (!email) return { error: "An email is required." };
  try {
    const inv = await api.createInvitation(email, role);
    revalidatePath("/settings/members");
    return { ok: `Invited ${email}.`, token: inv.token };
  } catch (e) {
    return { error: e instanceof ApiError ? e.message : "Failed to send invite" };
  }
}

export async function revokeInviteAction(formData: FormData): Promise<void> {
  await api.revokeInvitation(String(formData.get("id") ?? ""));
  revalidatePath("/settings/members");
}

export async function createProjectAction(formData: FormData): Promise<void> {
  const slug = String(formData.get("slug") ?? "").trim();
  const name = String(formData.get("name") ?? "").trim();
  await api.createProject(slug, name);
  revalidatePath("/projects");
}

export async function createDocumentAction(formData: FormData): Promise<void> {
  const projectId = String(formData.get("projectId") ?? "");
  const path = String(formData.get("path") ?? "").trim();
  const title = String(formData.get("title") ?? "").trim();
  const content = String(formData.get("content") ?? `# ${title}\n`);
  const doc = await api.createDocument(projectId, path, title, content);
  redirect(`/documents/${doc.id}`);
}

export async function saveDocumentAction(
  id: string,
  content: string,
  expectedVersion: number,
): Promise<UpdateResult> {
  const result = await api.updateDocument(id, content, expectedVersion, "checkpoint");
  if (result.ok) revalidatePath(`/documents/${id}`);
  return result;
}

export async function deleteDocumentAction(id: string): Promise<void> {
  await api.deleteDocument(id);
  redirect("/projects");
}

export async function restoreVersionAction(id: string, version: number): Promise<void> {
  await api.restoreVersion(id, version);
  revalidatePath(`/documents/${id}`);
}

export async function createKeyAction(
  _prev: { secret?: string; error?: string } | null,
  formData: FormData,
): Promise<{ secret?: string; error?: string } | null> {
  const name = String(formData.get("name") ?? "").trim();
  const role = String(formData.get("role") ?? "member");
  try {
    const key = await api.createKey(name, role);
    revalidatePath("/settings/keys");
    return { secret: key.secret };
  } catch (e) {
    return { error: e instanceof ApiError ? e.message : "Failed to create key" };
  }
}

export async function revokeKeyAction(formData: FormData): Promise<void> {
  await api.revokeKey(String(formData.get("id") ?? ""));
  revalidatePath("/settings/keys");
}

// --- connected apps (OAuth connector grants) ---------------------------------

export async function revokeOAuthGrantAction(formData: FormData): Promise<void> {
  const clientId = String(formData.get("client_id") ?? "");
  const orgId = String(formData.get("org_id") ?? "");
  if (clientId && orgId) await api.revokeOAuthGrant(clientId, orgId);
  revalidatePath("/settings/keys");
}

export async function switchOAuthGrantAction(formData: FormData): Promise<void> {
  const clientId = String(formData.get("client_id") ?? "");
  const fromOrgId = String(formData.get("from_org_id") ?? "");
  const toOrgId = String(formData.get("to_org_id") ?? "");
  if (clientId && fromOrgId && toOrgId && fromOrgId !== toOrgId) {
    await api.switchOAuthGrant(clientId, fromOrgId, toOrgId);
  }
  revalidatePath("/settings/keys");
}

// --- members + invite acceptance ---------------------------------------------

export async function updateMemberRoleAction(formData: FormData): Promise<void> {
  const userId = String(formData.get("user_id") ?? "");
  const role = String(formData.get("role") ?? "");
  if (userId && role) {
    try {
      await api.updateMemberRole(userId, role);
    } catch {
      /* guard rejections (e.g. last owner) — row stays unchanged on revalidate */
    }
  }
  revalidatePath("/settings/members");
}

export async function removeMemberAction(formData: FormData): Promise<void> {
  const userId = String(formData.get("user_id") ?? "");
  if (userId) {
    try {
      await api.removeMember(userId);
    } catch {
      /* guard rejections — ignore */
    }
  }
  revalidatePath("/settings/members");
}

export async function acceptInviteAction(formData: FormData): Promise<void> {
  const token = String(formData.get("token") ?? "");
  let orgId: string | null = null;
  try {
    const org = await api.acceptInvite(token);
    orgId = org.id;
  } catch {
    redirect("/projects?invite=invalid");
  }
  if (orgId) await setCurrentOrg(orgId);
  redirect("/projects");
}
