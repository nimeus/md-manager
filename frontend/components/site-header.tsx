import Link from "next/link";

import { getSession } from "@/lib/session";

import Logo from "./logo";

const NAV: [string, string][] = [
  ["/#features", "Product"],
  ["/docs", "Docs"],
  ["/docs/cli", "CLI"],
  ["/docs/mcp", "MCP"],
  ["/docs/connectors", "Connectors"],
];

export default async function SiteHeader() {
  const session = await getSession();
  return (
    <header className="sticky top-0 z-30 border-b border-line/70 bg-paper/85 backdrop-blur">
      <div className="mx-auto flex h-16 max-w-6xl items-center justify-between px-6">
        <Link href="/" className="transition hover:opacity-80">
          <Logo />
        </Link>
        <nav className="hidden items-center gap-7 text-sm text-ink-2 md:flex">
          {NAV.map(([href, label]) => (
            <Link key={href} href={href} className="transition hover:text-ink">
              {label}
            </Link>
          ))}
        </nav>
        <div className="flex items-center gap-3">
          {session ? (
            <Link href="/projects" className="btn btn-sm">
              Open app →
            </Link>
          ) : (
            <>
              <Link
                href="/login"
                className="hidden text-sm text-ink-2 transition hover:text-ink sm:inline"
              >
                Sign in
              </Link>
              <Link href="/login" className="btn btn-sm">
                Get started
              </Link>
            </>
          )}
        </div>
      </div>
    </header>
  );
}
