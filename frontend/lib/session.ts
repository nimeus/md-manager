import { cookies } from "next/headers";

const COOKIE = "mdm_session";

export type Session = { apiKey: string };

/**
 * The session holds only the API key, server-side, in an httpOnly cookie. The browser
 * never sees it (BFF pattern). The upstream API host is fixed by the server (env
 * `MDM_API_URL`) — clients cannot choose it, which prevents SSRF. When Logto OAuth lands,
 * only the login step changes.
 */
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
  (await cookies()).set(
    COOKIE,
    Buffer.from(JSON.stringify(session)).toString("base64"),
    {
      httpOnly: true,
      sameSite: "lax",
      path: "/",
      secure: process.env.NODE_ENV === "production",
      maxAge: 60 * 60 * 24 * 30,
    },
  );
}

export async function clearSession(): Promise<void> {
  (await cookies()).delete(COOKIE);
}
