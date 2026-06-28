"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

const SECTIONS: { title: string; items: [string, string][] }[] = [
  { title: "Getting started", items: [["/docs", "Overview"]] },
  {
    title: "Agent surfaces",
    items: [
      ["/docs/cli", "CLI (mdm)"],
      ["/docs/mcp", "MCP server"],
      ["/docs/connectors", "Web connectors"],
    ],
  },
];

export default function DocsSidebar() {
  const pathname = usePathname();
  return (
    <nav className="space-y-6 text-sm">
      {SECTIONS.map((s) => (
        <div key={s.title}>
          <div className="mb-2 text-xs font-semibold uppercase tracking-wider text-ink-soft">
            {s.title}
          </div>
          <ul className="space-y-1">
            {s.items.map(([href, label]) => {
              const active = pathname === href;
              return (
                <li key={href}>
                  <Link
                    href={href}
                    className={
                      "block rounded-md px-3 py-1.5 transition " +
                      (active
                        ? "bg-clay-soft/60 font-medium text-clay-dark"
                        : "text-ink-2 hover:bg-paper-2 hover:text-ink")
                    }
                  >
                    {label}
                  </Link>
                </li>
              );
            })}
          </ul>
        </div>
      ))}
    </nav>
  );
}
