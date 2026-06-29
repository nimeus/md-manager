import Link from "next/link";
import { redirect } from "next/navigation";

import Logo from "@/components/logo";
import { api } from "@/lib/api";
import { getSession, type Session } from "@/lib/session";

import { approveConsentAction, denyConsentAction } from "./actions";

export const metadata = { title: "Authorize connection — md-manager" };

type Org = { id: string; slug: string; name: string; role: string };
type RequestInfo = { client_name: string; scope: string; redirect_uri: string };

export default async function ConsentPage({
  searchParams,
}: {
  searchParams: Promise<{ request_id?: string; error?: string }>;
}) {
  const { request_id, error } = await searchParams;
  const session = await getSession();
  if (!session) {
    const back = `/oauth/consent?request_id=${encodeURIComponent(request_id ?? "")}`;
    redirect(`/login?next=${encodeURIComponent(back)}`);
  }

  return (
    <div className="paper-texture flex min-h-screen flex-col items-center justify-center px-6 py-12">
      <Link href="/" className="mb-8 transition hover:opacity-80">
        <Logo />
      </Link>
      <div className="card w-full max-w-md">
        <Consent requestId={request_id} session={session} error={error} />
      </div>
      <p className="mt-6 text-xs text-ink-soft">md-manager · secure OAuth 2.1 connection</p>
    </div>
  );
}

async function Consent({
  requestId,
  session,
  error,
}: {
  requestId?: string;
  session: Session;
  error?: string;
}) {
  if (!requestId) {
    return <Problem msg="Missing authorization request. Start the connection again from your assistant." />;
  }

  const data = (await Promise.all([api.getOAuthRequest(requestId), api.myOrgs()]).catch(
    () => null,
  )) as [RequestInfo, Org[]] | null;
  if (!data) {
    return <Problem msg="This request is invalid or has expired. Start the connection again from your assistant." />;
  }
  const [info, orgs] = data;
  if (orgs.length === 0) {
    return <Problem msg="You need an organization first. Create one, then reconnect." />;
  }

  const host = hostOf(info.redirect_uri);
  return (
    <>
      <div className="eyebrow mb-1.5">Authorize connection</div>
      {/* client_name is attacker-controlled (DCR) — React escapes it; never dangerouslySetInnerHTML. */}
      <h1 className="text-2xl font-semibold tracking-tight text-ink">{info.client_name}</h1>
      <p className="mt-2 text-sm leading-relaxed text-ink-2">
        wants to read and write your md-manager documents <strong>as you</strong>. Choose the
        organization it can access — it will act with your permissions there.
      </p>
      {host && <p className="mt-2 font-mono text-xs text-ink-soft">redirects to {host}</p>}

      {error && (
        <p className="mt-4 rounded-lg border border-red-200 bg-red-50 px-3 py-2.5 text-sm text-red-700">
          That request expired before it was approved. Start the connection again from your assistant.
        </p>
      )}

      <form action={approveConsentAction} className="mt-5">
        <input type="hidden" name="request_id" value={requestId} />
        <label className="label" htmlFor="org_id">
          Access
        </label>
        <select id="org_id" name="org_id" className="input" defaultValue={session.currentOrg}>
          <option value="all">All my organizations</option>
          {orgs.map((o) => (
            <option key={o.id} value={o.id}>
              Only {o.name} · {o.role}
            </option>
          ))}
        </select>
        <p className="mt-1 text-xs text-ink-soft">
          “All my organizations” lets it work across every org you belong to (it picks one per
          request). A single org limits it to that one.
        </p>
        <button className="btn-accent mt-4 w-full justify-center py-2.5" type="submit">
          Allow access
        </button>
      </form>

      <form action={denyConsentAction} className="mt-2">
        <input type="hidden" name="request_id" value={requestId} />
        <button className="btn-ghost w-full justify-center" type="submit">
          Deny
        </button>
      </form>

      <p className="mt-5 text-center text-xs leading-relaxed text-ink-soft">
        Signed in as {session.user.email}. You can revoke this connection any time in Settings → API Keys.
      </p>
    </>
  );
}

function Problem({ msg }: { msg: string }) {
  return (
    <>
      <h1 className="text-xl font-semibold text-ink">Can&apos;t complete this connection</h1>
      <p className="mt-2 text-sm leading-relaxed text-ink-2">{msg}</p>
      <Link href="/projects" className="btn mt-5 w-full justify-center">
        Go to your workspace
      </Link>
    </>
  );
}

function hostOf(uri: string): string {
  try {
    return new URL(uri).host;
  } catch {
    return "";
  }
}
