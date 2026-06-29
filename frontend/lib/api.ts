import { getSession } from "./session";

/**
 * Upstream API base URL — fixed by the server operator (NOT client-supplied), so a user
 * cannot make the BFF fetch an arbitrary host (SSRF). Configure per deployment.
 */
const API_BASE = (process.env.MDM_API_URL ?? "http://127.0.0.1:8080").replace(/\/+$/, "");

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
    authorization: `Bearer ${session.token}`,
  };
  // The org switcher: tell the API which of the user's orgs to act in.
  if (session.currentOrg) headers["x-org-id"] = session.currentOrg;
  if (init?.body) headers["content-type"] = "application/json";

  const res = await fetch(`${API_BASE}${path}`, {
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

  // orgs + invitations (web SaaS)
  myOrgs: () => req("/v1/me/orgs"),
  createOrg: (slug: string, name: string) => req("/v1/orgs", json({ slug, name })),
  listInvitations: () => req("/v1/invitations"),
  createInvitation: (email: string, role: string) =>
    req("/v1/invitations", json({ email, role })),
  revokeInvitation: (id: string) => req(`/v1/invitations/${id}`, { method: "DELETE" }),
  acceptInvite: (token: string) => req("/v1/invitations/accept", json({ token })),

  // members
  listMembers: () => req("/v1/members"),
  updateMemberRole: (userId: string, role: string) =>
    req(`/v1/members/${userId}`, { method: "PUT", body: JSON.stringify({ role }) }),
  removeMember: (userId: string) => req(`/v1/members/${userId}`, { method: "DELETE" }),

  // sharing
  createShare: (docId: string, audience: string, recipients: string[], expiresInDays?: number) =>
    req(`/v1/documents/${docId}/shares`, json({ audience, recipients, expires_in_days: expiresInDays ?? null })),
  listShares: (docId: string) => req(`/v1/documents/${docId}/shares`),
  revokeShare: (linkId: string) => req(`/v1/shares/${linkId}`, { method: "DELETE" }),

  // OAuth consent (built-in connector authorization server)
  getOAuthRequest: (id: string) => req(`/v1/oauth/authorization-requests/${id}`),
  approveOAuthConsent: (id: string, orgId: string) =>
    req(
      `/v1/oauth/authorization-requests/${id}/approve`,
      json(orgId === "all" ? { all_orgs: true } : { org_id: orgId }),
    ),
  denyOAuthConsent: (id: string) =>
    req(`/v1/oauth/authorization-requests/${id}/deny`, { method: "POST" }),

  // Connected apps (manage connector grants)
  listOAuthGrants: () => req("/v1/oauth/grants"),
  revokeOAuthGrant: (clientId: string, orgId: string) =>
    req(`/v1/oauth/grants/${encodeURIComponent(clientId)}/revoke`, json({ org_id: orgId })),
  switchOAuthGrant: (clientId: string, fromOrgId: string, toOrgId: string) =>
    req(
      `/v1/oauth/grants/${encodeURIComponent(clientId)}/switch`,
      json({ from_org_id: fromOrgId, to_org_id: toOrgId }),
    ),
};

/** Result of exchanging a Google ID token for a backend session (used only during login). */
export type GoogleExchange = {
  session_token: string;
  user: { id: string; email: string; name: string };
  orgs: { id: string; slug: string; name: string; role: string }[];
};

/**
 * Exchange a verified Google ID token for a backend session token. Session-less (this IS the
 * login), so it calls the API directly. Throws [`ApiError`] on failure.
 */
/**
 * Resolve a shared document. Session-aware but not session-required: anonymous works for public
 * links; private links need the viewer's session (attached if present). Returns status so the
 * page can branch (200 render / 401 sign-in / 403 no-access / 404 invalid).
 */
export async function getSharedDoc(token: string): Promise<{ status: number; data: any }> {
  const session = await getSession();
  const headers: Record<string, string> = {};
  if (session) headers.authorization = `Bearer ${session.token}`;
  const res = await fetch(`${API_BASE}/v1/shared/${encodeURIComponent(token)}`, {
    headers,
    cache: "no-store",
  });
  const text = await res.text();
  return { status: res.status, data: text ? safeJson(text) : null };
}

export async function exchangeGoogleToken(idToken: string): Promise<GoogleExchange> {
  const res = await fetch(`${API_BASE}/v1/auth/google`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ id_token: idToken }),
    cache: "no-store",
  });
  const text = await res.text();
  const data = text ? safeJson(text) : null;
  if (res.status >= 400) {
    throw new ApiError(res.status, data?.error ?? "error", data?.message ?? `HTTP ${res.status}`);
  }
  return data as GoogleExchange;
}
