import { cookies } from "next/headers";

const COOKIE = "mdm_session";

export type SessionUser = { id: string; email: string; name: string };

/**
 * The session holds the backend-issued session token (`mss_…`), the user's identity, and which
 * org they're currently acting in — all server-side, in an httpOnly cookie. The browser never
 * sees the token (BFF pattern). The org list itself is fetched fresh from the API, so newly
 * created/joined orgs show up without re-login.
 */
export type Session = {
  token: string;
  user: SessionUser;
  currentOrg: string;
};

export async function getSession(): Promise<Session | null> {
  const raw = (await cookies()).get(COOKIE)?.value;
  if (!raw) return null;
  try {
    return JSON.parse(Buffer.from(raw, "base64").toString("utf8")) as Session;
  } catch {
    return null;
  }
}

export async function setSession(session: Session): Promise<void> {
  (await cookies()).set(COOKIE, Buffer.from(JSON.stringify(session)).toString("base64"), {
    httpOnly: true,
    sameSite: "lax",
    path: "/",
    secure: process.env.NODE_ENV === "production",
    maxAge: 60 * 60 * 24 * 30,
  });
}

/** Update just the current org (the org switcher), keeping the rest of the session. */
export async function setCurrentOrg(orgId: string): Promise<void> {
  const s = await getSession();
  if (!s) return;
  await setSession({ ...s, currentOrg: orgId });
}

export async function clearSession(): Promise<void> {
  (await cookies()).delete(COOKIE);
}
