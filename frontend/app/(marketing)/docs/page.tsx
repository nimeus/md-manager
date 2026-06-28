import Link from "next/link";

import { H1, H2, Lead, P } from "@/components/doc-ui";

export const metadata = { title: "Docs — md-manager" };

const CARDS: { href: string; tag: string; title: string; body: string }[] = [
  {
    href: "/docs/cli",
    tag: "CLI",
    title: "The mdm command",
    body: "Read and write docs from any shell. Ideal for Claude Code, Gemini CLI, and scripts — raw markdown to stdout.",
  },
  {
    href: "/docs/mcp",
    tag: "MCP",
    title: "The MCP server",
    body: "A stdio MCP server exposing 20 tools. Point Claude Desktop or any MCP host at your documents.",
  },
  {
    href: "/docs/connectors",
    tag: "Connectors",
    title: "Web connectors",
    body: "Remote MCP over HTTP with OAuth 2.1, so hosted assistants like Claude.ai and ChatGPT can connect.",
  },
];

export default function DocsOverview() {
  return (
    <>
      <H1>Documentation</H1>
      <Lead>
        md-manager stores markdown in Postgres and exposes it to people and AI agents under one
        permission model. There are three ways an agent connects — pick whichever fits your host.
      </Lead>

      <H2>The shape of it</H2>
      <P>
        Everything goes through one HTTP API backed by Postgres row-level security. The web app is
        a thin BFF for humans; the CLI and MCP server are thin clients for agents. Same rules,
        same data, three doors.
      </P>

      <div className="mt-8 grid gap-4 sm:grid-cols-2">
        {CARDS.map((c) => (
          <Link
            key={c.href}
            href={c.href}
            className="card group transition hover:border-line-2 hover:shadow-[var(--shadow-lift)]"
          >
            <div className="eyebrow">{c.tag}</div>
            <h3 className="mt-2 text-lg font-semibold text-ink">{c.title}</h3>
            <p className="mt-2 text-sm leading-relaxed text-ink-2">{c.body}</p>
            <span className="mt-4 inline-block text-sm text-clay transition group-hover:translate-x-0.5">
              Read more →
            </span>
          </Link>
        ))}
      </div>

      <H2>Core concepts</H2>
      <P>
        A <strong>document</strong> has a stable UUID and a mutable path; it lives only in the
        database. Documents belong to <strong>projects</strong>, inside an{" "}
        <strong>organization</strong> (your tenant boundary). Tags and categories cross projects.
        Every write is versioned, and stale writes return a structured conflict so nothing is lost.
      </P>
      <P>
        Authentication is a scoped <strong>API key</strong> (<code className="font-mono">mk_…</code>
        ) for the CLI and MCP server, or OAuth for web connectors. Either way you resolve to the
        same <em>(user, org, role)</em> and the same row-level rules.
      </P>
    </>
  );
}
