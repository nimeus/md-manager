"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useEffect, useState } from "react";

import { logoutAction } from "@/lib/actions";
import Logo from "./logo";

type Org = { id: string; slug: string; name: string; role: string };
type User = { id: string; email: string; name: string };

const links: [string, string][] = [
  ["/projects", "Projects"],
  ["/search", "Search"],
  ["/settings/members", "Members"],
  ["/settings/keys", "API Keys"],
];

function MenuIcon() {
  return (
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
      <path d="M3 6h18M3 12h18M3 18h18" />
    </svg>
  );
}

function CloseIcon() {
  return (
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
      <path d="M6 6l12 12M18 6L6 18" />
    </svg>
  );
}

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
  const [open, setOpen] = useState(false);

  // Close the drawer whenever the route changes (e.g. tapping a nav link).
  useEffect(() => {
    setOpen(false);
  }, [pathname]);

  return (
    <>
      {/* Mobile top bar — only below lg, where the sidebar is off-canvas. */}
      <div className="sticky top-0 z-30 flex h-14 items-center justify-between border-b border-line bg-panel/95 px-4 backdrop-blur lg:hidden">
        <Link href="/" className="inline-flex transition hover:opacity-80">
          <Logo />
        </Link>
        <button
          type="button"
          onClick={() => setOpen(true)}
          aria-label="Open menu"
          className="rounded-md border border-line-2 bg-panel p-2 text-ink-2 transition hover:bg-paper-2"
        >
          <MenuIcon />
        </button>
      </div>

      {/* Backdrop behind the drawer (mobile only). */}
      <div
        onClick={() => setOpen(false)}
        aria-hidden
        className={
          "fixed inset-0 z-40 bg-ink/40 backdrop-blur-sm transition-opacity lg:hidden " +
          (open ? "opacity-100" : "pointer-events-none opacity-0")
        }
      />

      {/* Sidebar: static column on lg, slide-in drawer below it. */}
      <aside
        className={
          "fixed inset-y-0 left-0 z-50 flex w-72 max-w-[85vw] flex-col border-r border-line bg-panel p-4 " +
          "transition-transform duration-200 ease-out lg:static lg:z-auto lg:w-60 lg:max-w-none lg:translate-x-0 " +
          (open ? "translate-x-0 shadow-[var(--shadow-lift)]" : "-translate-x-full lg:shadow-none")
        }
      >
        <div className="mb-5 flex items-center justify-between">
          <Link href="/" className="inline-flex transition hover:opacity-80">
            <Logo />
          </Link>
          <button
            type="button"
            onClick={() => setOpen(false)}
            aria-label="Close menu"
            className="rounded-md p-1 text-ink-soft transition hover:bg-paper-2 hover:text-ink lg:hidden"
          >
            <CloseIcon />
          </button>
        </div>

        {/* Org switcher — a no-JS dropdown of links to the switch route. */}
        <details className="group relative mb-5">
          <summary className="card flex cursor-pointer list-none items-center justify-between p-2 text-sm">
            <span className="truncate">
              <span className="font-medium">{current.name}</span>
              <span className="ml-1 text-xs text-ink-soft">{current.role}</span>
            </span>
            <span className="text-ink-soft">▾</span>
          </summary>
          <div className="absolute left-0 right-0 z-10 mt-1 overflow-hidden rounded-md border border-line-2 bg-panel shadow-lg">
            {orgs.map((o) => (
              <a
                key={o.id}
                href={`/auth/switch?org=${o.id}`}
                className={
                  "flex items-center justify-between px-3 py-2 text-sm hover:bg-paper-2 " +
                  (o.id === current.id ? "text-clay-dark" : "text-ink")
                }
              >
                <span className="truncate">{o.name}</span>
                <span className="text-xs text-ink-soft">{o.role}</span>
              </a>
            ))}
            <Link
              href="/onboarding"
              className="block border-t border-line px-3 py-2 text-sm text-ink-soft hover:bg-paper-2"
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
                  (active ? "bg-clay-soft/60 text-clay-dark" : "text-ink-2 hover:bg-paper-2")
                }
              >
                {label}
              </Link>
            );
          })}
        </nav>

        <div className="mt-4 border-t border-line pt-3">
          <div className="mb-2 truncate text-xs text-ink-soft" title={user.email}>
            {user.name || user.email}
          </div>
          <form action={logoutAction}>
            <button className="btn-ghost w-full" type="submit">
              Sign out
            </button>
          </form>
        </div>
      </aside>
    </>
  );
}
