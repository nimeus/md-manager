import { redirect } from "next/navigation";

import Nav from "@/components/nav";
import { api } from "@/lib/api";
import { getSession } from "@/lib/session";

export default async function AppLayout({ children }: { children: React.ReactNode }) {
  if (!(await getSession())) redirect("/login");

  let org = "—";
  let role = "—";
  try {
    const me = await api.whoami();
    org = me.org_id?.slice(0, 8) ?? "—";
    role = me.role ?? "—";
  } catch {
    redirect("/login");
  }

  return (
    <div className="flex min-h-screen">
      <Nav org={org} role={role} />
      <main className="flex-1 overflow-x-hidden">
        <div className="mx-auto max-w-5xl p-8">{children}</div>
      </main>
    </div>
  );
}
