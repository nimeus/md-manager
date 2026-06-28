"use server";

import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";

import { api, ApiError, type UpdateResult } from "./api";
import { clearSession, setSession } from "./session";

export type FormState = { error?: string } | null;

export async function loginAction(_prev: FormState, formData: FormData): Promise<FormState> {
  const apiKey = String(formData.get("apiKey") ?? "").trim();
  if (!apiKey) return { error: "An API key is required." };

  // The API host is fixed server-side (MDM_API_URL); clients only supply the key.
  await setSession({ apiKey });
  try {
    await api.whoami();
  } catch {
    await clearSession();
    return { error: "Could not authenticate — check the key (and that the server can reach the API)." };
  }
  redirect("/projects");
}

export async function logoutAction(): Promise<void> {
  await clearSession();
  redirect("/login");
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
