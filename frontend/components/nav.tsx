"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

import { logoutAction } from "@/lib/actions";

type Org = { id: string; slug: string; name: string; role: string };
type User = { id: string; email: string; name: string };

const links: [string, string][] = [
  ["/projects", "Projects"],
  ["/search", "Search"],
  ["/settings/members", "Members"],
  ["/settings/keys", "API Keys"],
];

export default function Nav({
  user,
  orgs,
  current,
}: {
  user: User;
  orgs: Org[];
  current: Org;
}) {
  const pathname = usePathname();
  return (
    <aside className="flex w-60 shrink-0 flex-col border-r border-zinc-800 bg-zinc-900/30 p-4">
      <div className="mb-4 text-sm font-semibold tracking-tight">md-manager</div>

      {/* Org switcher — a no-JS dropdown of links to the switch route. */}
      <details className="group relative mb-5">
        <summary className="card flex cursor-pointer list-none items-center justify-between p-2 text-sm">
          <span className="truncate">
            <span className="font-medium">{current.name}</span>
            <span className="ml-1 text-xs text-zinc-500">{current.role}</span>
          </span>
          <span className="text-zinc-500">▾</span>
        </summary>
        <div className="absolute left-0 right-0 z-10 mt-1 overflow-hidden rounded-md border border-zinc-700 bg-zinc-900 shadow-lg">
          {orgs.map((o) => (
            <a
              key={o.id}
              href={`/auth/switch?org=${o.id}`}
              className={
                "flex items-center justify-between px-3 py-2 text-sm hover:bg-zinc-800 " +
                (o.id === current.id ? "text-indigo-300" : "text-zinc-200")
              }
            >
              <span className="truncate">{o.name}</span>
              <span className="text-xs text-zinc-500">{o.role}</span>
            </a>
          ))}
          <Link
            href="/onboarding"
            className="block border-t border-zinc-800 px-3 py-2 text-sm text-zinc-400 hover:bg-zinc-800"
          >
            + New organization
          </Link>
        </div>
      </details>

      <nav className="flex flex-1 flex-col gap-1">
        {links.map(([href, label]) => {
          const active = pathname === href || pathname.startsWith(href + "/");
          return (
            <Link
              key={href}
              href={href}
              className={
                "rounded-md px-3 py-2 text-sm transition " +
                (active ? "bg-indigo-600/20 text-indigo-300" : "text-zinc-300 hover:bg-zinc-800")
              }
            >
              {label}
            </Link>
          );
        })}
      </nav>

      <div className="mt-4 border-t border-zinc-800 pt-3">
        <div className="mb-2 truncate text-xs text-zinc-500" title={user.email}>
          {user.name || user.email}
        </div>
        <form action={logoutAction}>
          <button className="btn-ghost w-full" type="submit">
            Sign out
          </button>
        </form>
      </div>
    </aside>
  );
}
