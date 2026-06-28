import { getSession } from "./session";

export class ApiError extends Error {
  constructor(
    public status: number,
    public code: string,
    message: string,
  ) {
    super(message);
  }
}

/** Low-level request: returns status + parsed body, never throws on HTTP error. */
async function raw(
  path: string,
  init?: RequestInit,
): Promise<{ status: number; data: any }> {
  const session = await getSession();
  if (!session) return { status: 401, data: { error: "unauthorized", message: "not signed in" } };

  const headers: Record<string, string> = {
    authorization: `Bearer ${session.apiKey}`,
  };
  if (init?.body) headers["content-type"] = "application/json";

  const res = await fetch(`${session.apiUrl}${path}`, {
    ...init,
    headers: { ...headers, ...(init?.headers as Record<string, string>) },
    cache: "no-store",
  });
  const text = await res.text();
  const data = text ? safeJson(text) : null;
  return { status: res.status, data };
}

function safeJson(text: string): any {
  try {
    return JSON.parse(text);
  } catch {
    return { raw: text };
  }
}

/** Request that throws [`ApiError`] on a non-2xx response. */
async function req(path: string, init?: RequestInit): Promise<any> {
  const { status, data } = await raw(path, init);
  if (status >= 400) {
    throw new ApiError(status, data?.error ?? "error", data?.message ?? `HTTP ${status}`);
  }
  return data;
}

const json = (body: unknown): RequestInit => ({ method: "POST", body: JSON.stringify(body) });

export type UpdateResult =
  | { ok: true; version: number; doc: any }
  | { ok: false; currentVersion: number; current: string; base: string };

export const api = {
  whoami: () => req("/v1/me"),

  listProjects: () => req("/v1/projects"),
  createProject: (slug: string, name: string) => req("/v1/projects", json({ slug, name })),
  getProject: (slug: string) => req(`/v1/projects/${slug}`),

  listDocuments: (projectId: string) => req(`/v1/projects/${projectId}/documents?limit=200`),
  createDocument: (projectId: string, path: string, title: string, content: string) =>
    req(`/v1/projects/${projectId}/documents`, json({ path, title, content })),

  getDocument: (id: string) => req(`/v1/documents/${id}`),
  history: (id: string) => req(`/v1/documents/${id}/history`),
  deleteDocument: (id: string) => req(`/v1/documents/${id}`, { method: "DELETE" }),
  restoreVersion: (id: string, version: number) =>
    req(`/v1/documents/${id}/restore`, json({ version })),

  /** Update with optimistic concurrency; distinguishes the 409 conflict path. */
  async updateDocument(
    id: string,
    content: string,
    expectedVersion: number,
    kind = "checkpoint",
  ): Promise<UpdateResult> {
    const { status, data } = await raw(`/v1/documents/${id}`, {
      method: "PUT",
      body: JSON.stringify({ content, expected_version: expectedVersion, kind }),
    });
    if (status >= 200 && status < 300) {
      return { ok: true, version: data.current_version, doc: data };
    }
    if (status === 409) {
      return {
        ok: false,
        currentVersion: data.current_version,
        current: data.current_content ?? "",
        base: data.base_content ?? "",
      };
    }
    throw new ApiError(status, data?.error ?? "error", data?.message ?? `HTTP ${status}`);
  },

  search: (query: string, projectId?: string) => {
    const qs = new URLSearchParams({ q: query });
    if (projectId) qs.set("project_id", projectId);
    return req(`/v1/search?${qs.toString()}`);
  },

  listKeys: () => req("/v1/api-keys"),
  createKey: (name: string, role: string) => req("/v1/api-keys", json({ name, role })),
  revokeKey: (id: string) => req(`/v1/api-keys/${id}`, { method: "DELETE" }),
};
