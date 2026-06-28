import Link from "next/link";

import Logo from "./logo";

function Col({ title, links }: { title: string; links: [string, string][] }) {
  return (
    <div>
      <div className="mb-3 text-xs font-semibold uppercase tracking-wider text-ink-soft">
        {title}
      </div>
      <ul className="space-y-2.5 text-sm text-ink-2">
        {links.map(([href, label]) => (
          <li key={href}>
            <Link href={href} className="transition hover:text-clay">
              {label}
            </Link>
          </li>
        ))}
      </ul>
    </div>
  );
}

export default function SiteFooter() {
  return (
    <footer className="border-t border-line bg-paper-2/40">
      <div className="mx-auto grid max-w-6xl gap-10 px-6 py-14 sm:grid-cols-2 md:grid-cols-4">
        <div className="sm:col-span-2 md:col-span-1">
          <Logo />
          <p className="mt-3 max-w-xs text-sm leading-relaxed text-ink-soft">
            Markdown docs in Postgres, for humans and AI agents — under one set of rules.
          </p>
        </div>
        <Col
          title="Product"
          links={[
            ["/#features", "Features"],
            ["/#code", "Examples"],
            ["/login", "Get started"],
          ]}
        />
        <Col
          title="For agents"
          links={[
            ["/docs/cli", "CLI"],
            ["/docs/mcp", "MCP server"],
            ["/docs/connectors", "Connectors"],
          ]}
        />
        <Col
          title="Docs"
          links={[
            ["/docs", "Overview"],
            ["/login", "Sign in"],
          ]}
        />
      </div>
      <div className="border-t border-line">
        <div className="mx-auto max-w-6xl px-6 py-5 text-xs text-ink-soft">
          © md-manager · built with Rust + Next.js · your documents, your database
        </div>
      </div>
    </footer>
  );
}
