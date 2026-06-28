import Link from "next/link";

import CodeBlock from "@/components/code-block";
import { apiBase } from "@/lib/docs";

export const metadata = {
  title: "md-manager — markdown docs for humans and AI agents",
  description:
    "Keep your markdown in Postgres — versioned, searchable, permissioned — and let people and AI agents read and write it under the same rules.",
};

const FEATURES: { title: string; body: string; tag: string }[] = [
  {
    tag: "Agent-native",
    title: "Built for agents, not bolted on",
    body: "An MCP server and a CLI expose the same 20 operations, so Claude, Gemini, and GPT read and write your docs directly — with the same permissions as a person.",
  },
  {
    tag: "Safe writes",
    title: "Conflict-aware editing",
    body: "Every save is version-checked. A stale write returns the current and base content so humans or agents can do a clean 3-way merge instead of clobbering work.",
  },
  {
    tag: "Findable",
    title: "Search that actually finds it",
    body: "Postgres full-text out of the box, plus semantic and hybrid search with pgvector — aggregated to documents, not raw chunks.",
  },
  {
    tag: "Multi-tenant",
    title: "Teams, enforced in the database",
    body: "Organizations, teams, projects, and a real role lattice — isolated by Postgres row-level security, not just app-layer checks.",
  },
  {
    tag: "Versioned",
    title: "Full history, instant restore",
    body: "Every version is a snapshot. Roll back any document to any point. Agent autosave churn is coalesced so history stays readable.",
  },
  {
    tag: "Yours",
    title: "Lives in Postgres",
    body: "Your documents are rows in a database you control — never files scattered across laptops and chat threads. Export, back up, and own them.",
  },
];

export default function Landing() {
  const API = apiBase();
  const CLI_SNIPPET = `# install once, then point it at this instance
mdm auth login --api-url ${API} --api-key mk_live_…

# create a doc, then read it as raw markdown — pipe straight into an agent
mdm doc create --project handbook --path runbooks/deploy \\
  --title "Deploy" -m "# Deploy\\n1. Tag the release\\n2. Ship"
mdm doc get 019f… | claude -p "summarize the deploy steps"

# search across everything (keyword, semantic, or hybrid)
mdm search "rollback procedure" --mode hybrid`;

  const MCP_SNIPPET = `// point any MCP client at the server with a key — no install
{
  "mcpServers": {
    "md-manager": {
      "command": "npx",
      "args": ["-y", "mcp-remote", "${API}/mcp",
               "--header", "Authorization: Bearer mk_live_…"]
    }
  }
}`;
  return (
    <>
      {/* Hero */}
      <section className="paper-texture border-b border-line">
        <div className="mx-auto grid max-w-6xl items-center gap-12 px-6 py-20 lg:grid-cols-[1.05fr_0.95fr] lg:py-28">
          <div>
            <span className="eyebrow">Docs for humans &amp; AI agents</span>
            <h1 className="mt-4 text-4xl font-semibold leading-[1.05] tracking-tight text-ink sm:text-5xl lg:text-[3.4rem]">
              Documentation your&nbsp;agents can actually use.
            </h1>
            <p className="mt-5 max-w-xl text-lg leading-relaxed text-ink-2">
              md-manager keeps your markdown in Postgres — versioned, searchable, and
              permissioned — and lets people and AI agents read and write it under the exact
              same rules. One source of truth, three ways in.
            </p>
            <div className="mt-8 flex flex-wrap items-center gap-3">
              <Link href="/login" className="btn-accent">
                Start with Google
              </Link>
              <Link href="/docs" className="btn-ghost">
                Explore the docs
              </Link>
            </div>
            <p className="mt-6 font-mono text-xs text-ink-soft">
              web app · CLI · MCP server · OAuth connectors
            </p>
          </div>

          {/* Visual: a document + an agent reading it */}
          <div className="relative">
            <div className="card overflow-hidden p-0">
              <div className="flex items-center gap-2 border-b border-line bg-paper-2/60 px-4 py-2.5">
                <span className="h-2.5 w-2.5 rounded-full bg-clay/70" />
                <span className="h-2.5 w-2.5 rounded-full bg-line-2" />
                <span className="h-2.5 w-2.5 rounded-full bg-line-2" />
                <span className="ml-2 font-mono text-xs text-ink-soft">
                  handbook / runbooks / deploy.md
                </span>
              </div>
              <div className="prose-md p-5">
                <h2 className="!mt-0">Deploy Runbook</h2>
                <p>
                  Ship a release to production. Owned by <code>@platform</code>, kept current
                  by both humans and the on-call agent.
                </p>
                <ol>
                  <li>Tag the release and wait for green CI.</li>
                  <li>
                    Promote with <code>mdm ship --canary</code>.
                  </li>
                  <li>Watch error rate; roll back on regression.</li>
                </ol>
              </div>
            </div>
            <div className="mt-3 rounded-xl border border-line-2 bg-ink px-4 py-3 font-mono text-[12.5px] text-paper shadow-[var(--shadow-lift)]">
              <span className="text-paper/45">agent&nbsp;›</span> get_doc(&quot;deploy&quot;) →{" "}
              <span className="text-clay-soft">3 steps, v7, updated 2m ago</span>
            </div>
          </div>
        </div>
      </section>

      {/* Two audiences */}
      <section className="mx-auto max-w-6xl px-6 py-20">
        <div className="grid gap-6 md:grid-cols-2">
          <div className="card">
            <div className="chip">For people</div>
            <h3 className="mt-4 text-2xl font-semibold text-ink">A clean place to write</h3>
            <p className="mt-2 leading-relaxed text-ink-2">
              A focused editor with live preview, version history, full-text search, teammates,
              and roles. Sign in with Google and you&apos;re in.
            </p>
          </div>
          <div className="card">
            <div className="chip">For agents</div>
            <h3 className="mt-4 text-2xl font-semibold text-ink">A clean place to act</h3>
            <p className="mt-2 leading-relaxed text-ink-2">
              The same documents over a CLI and an MCP server — create, update, append, search,
              tag — with scoped keys and the identical permission model.
            </p>
          </div>
        </div>
      </section>

      {/* Features */}
      <section id="features" className="border-y border-line bg-paper-2/40">
        <div className="mx-auto max-w-6xl px-6 py-20">
          <div className="max-w-2xl">
            <span className="eyebrow">Why md-manager</span>
            <h2 className="mt-3 text-3xl font-semibold tracking-tight text-ink sm:text-4xl">
              Everything a shared knowledge base needs — for both kinds of readers.
            </h2>
          </div>
          <div className="mt-12 grid gap-px overflow-hidden rounded-2xl border border-line bg-line md:grid-cols-2 lg:grid-cols-3">
            {FEATURES.map((f) => (
              <div key={f.title} className="bg-panel p-6">
                <div className="eyebrow">{f.tag}</div>
                <h3 className="mt-3 text-lg font-semibold text-ink">{f.title}</h3>
                <p className="mt-2 text-sm leading-relaxed text-ink-2">{f.body}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* Code */}
      <section id="code" className="mx-auto max-w-6xl px-6 py-20">
        <div className="max-w-2xl">
          <span className="eyebrow">In practice</span>
          <h2 className="mt-3 text-3xl font-semibold tracking-tight text-ink sm:text-4xl">
            Two lines for a person. Two tools for an agent.
          </h2>
          <p className="mt-3 text-ink-2">
            The CLI is great for shell-capable agents (cheap, scriptable). The MCP server is for
            hosts that speak MCP. Both go through the same API and rules.
          </p>
        </div>
        <div className="mt-10 grid gap-6 lg:grid-cols-2">
          <div>
            <div className="mb-3 flex items-center gap-2">
              <span className="chip">CLI</span>
              <span className="text-sm text-ink-soft">read &amp; write from any shell</span>
            </div>
            <CodeBlock filename="terminal" code={CLI_SNIPPET} />
          </div>
          <div>
            <div className="mb-3 flex items-center gap-2">
              <span className="chip">MCP</span>
              <span className="text-sm text-ink-soft">point an MCP host at your docs</span>
            </div>
            <CodeBlock filename="claude_desktop_config.json" code={MCP_SNIPPET} />
          </div>
        </div>
        <div className="mt-8 flex flex-wrap gap-4 text-sm">
          <Link href="/docs/cli" className="link-accent">
            CLI reference →
          </Link>
          <Link href="/docs/mcp" className="link-accent">
            MCP setup →
          </Link>
          <Link href="/docs/connectors" className="link-accent">
            Web connectors →
          </Link>
        </div>
      </section>

      {/* CTA */}
      <section className="border-t border-line">
        <div className="mx-auto max-w-6xl px-6 py-20">
          <div className="paper-texture rounded-3xl border border-line bg-panel px-8 py-14 text-center shadow-[var(--shadow-soft)]">
            <h2 className="mx-auto max-w-2xl text-3xl font-semibold tracking-tight text-ink sm:text-4xl">
              Bring your docs into one place your team and your agents both trust.
            </h2>
            <p className="mx-auto mt-4 max-w-xl text-ink-2">
              Free to start. Sign in with Google, create your organization, and invite the rest —
              humans and agents alike.
            </p>
            <div className="mt-8 flex flex-wrap justify-center gap-3">
              <Link href="/login" className="btn-accent">
                Start with Google
              </Link>
              <Link href="/docs" className="btn-ghost">
                Read the docs
              </Link>
            </div>
          </div>
        </div>
      </section>
    </>
  );
}
