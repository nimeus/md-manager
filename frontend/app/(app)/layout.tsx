import { redirect } from "next/navigation";

import Nav from "@/components/nav";
import { api } from "@/lib/api";
import { getSession } from "@/lib/session";

export type Org = { id: string; slug: string; name: string; role: string };

export default async function AppLayout({ children }: { children: React.ReactNode }) {
  const session = await getSession();
  if (!session) redirect("/login");

  let orgs: Org[] = [];
  try {
    orgs = await api.myOrgs();
  } catch {
    // Stale/invalid session token, or the API is unreachable → back to sign-in.
    redirect("/login");
  }
  if (orgs.length === 0) redirect("/onboarding");

  // Repair a stale current-org cookie (e.g. removed from that org) via the switch route —
  // cookies can't be written during render, so redirect to the route handler that can.
  const current = orgs.find((o) => o.id === session.currentOrg);
  if (!current) redirect(`/auth/switch?org=${orgs[0].id}`);

  return (
    <div className="flex min-h-screen">
      <Nav user={session.user} orgs={orgs} current={current} />
      <main className="flex-1 overflow-x-hidden">
        <div className="mx-auto max-w-5xl p-8">{children}</div>
      </main>
    </div>
  );
}
