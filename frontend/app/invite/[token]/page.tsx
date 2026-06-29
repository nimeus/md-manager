import Link from "next/link";
import { redirect } from "next/navigation";

import Logo from "@/components/logo";
import { acceptInviteAction } from "@/lib/actions";
import { getSession } from "@/lib/session";

export const metadata = { title: "Join organization — md-manager" };

export default async function InvitePage({
  params,
}: {
  params: Promise<{ token: string }>;
}) {
  const { token } = await params;
  const session = await getSession();
  if (!session) {
    redirect(`/login?next=/invite/${encodeURIComponent(token)}`);
  }

  return (
    <div className="paper-texture flex min-h-screen flex-col items-center justify-center px-6">
      <Link href="/" className="mb-8 transition hover:opacity-80">
        <Logo />
      </Link>
      <div className="card w-full max-w-sm">
        <div className="eyebrow mb-1.5">You&apos;re invited</div>
        <h1 className="text-2xl font-semibold tracking-tight text-ink">Join the organization</h1>
        <p className="mt-2 text-sm leading-relaxed text-ink-2">
          Accept this invitation to join a md-manager organization with the role you were given.
          You&apos;re signed in as <span className="text-ink">{session.user.email}</span>.
        </p>
        <form action={acceptInviteAction} className="mt-5">
          <input type="hidden" name="token" value={token} />
          <button className="btn-accent w-full justify-center py-2.5" type="submit">
            Join organization
          </button>
        </form>
        <Link href="/projects" className="btn-ghost mt-2 w-full justify-center">
          Not now
        </Link>
      </div>
    </div>
  );
}
