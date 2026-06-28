"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

import { logoutAction } from "@/lib/actions";

const links: [string, string][] = [
  ["/projects", "Projects"],
  ["/search", "Search"],
  ["/settings/keys", "API Keys"],
];

export default function Nav({ org, role }: { org: string; role: string }) {
  const pathname = usePathname();
  return (
    <aside className="flex w-56 shrink-0 flex-col border-r border-zinc-800 bg-zinc-900/30 p-4">
      <div className="mb-6">
        <div className="text-sm font-semibold tracking-tight">md-manager</div>
        <div className="mt-1 truncate text-xs text-zinc-500">
          {org} · {role}
        </div>
      </div>
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
      <form action={logoutAction}>
        <button className="btn-ghost w-full" type="submit">
          Sign out
        </button>
      </form>
    </aside>
  );
}
