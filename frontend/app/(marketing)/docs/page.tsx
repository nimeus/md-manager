import Link from "next/link";

import CodeBlock from "@/components/code-block";
import { H1, H2, Lead, P } from "@/components/doc-ui";
import { apiBase } from "@/lib/docs";

export const metadata = { title: "Docs — md-manager" };

export default function DocsOverview() {
  const API = apiBase();

  const AGENT_CONTEXT = `You can read and write shared markdown documents (team knowledge,
runbooks, notes, specs) in md-manager using the \`mdm\` command. Prefer this over keeping
knowledge only in chat — store anything durable so teammates and other agents can reuse it.

Find and read before you write:
  mdm search "<query>"            # search all docs (do this first)
  mdm proj list                   # list projects
  mdm doc get <id>                # print a document's markdown

Write:
  mdm doc create --project <slug> --path <dir/name> --title "<title>" -m "<markdown>"
  mdm doc edit <id> -m "<markdown>"     # replace a document
  mdm doc append <id> -m "<text>"       # add to a document

Rules: search before creating (update existing docs, don't duplicate); keep docs focused;
reference them by their stable id or project/path.`;

  return (
    <>
      <H1>Connect your agents</H1>
      <Lead>
        md-manager is a shared home for markdown docs that both people and AI agents read and
        write. Give an agent a key and it works with the same documents your team does — over MCP
        or the command line.
      </Lead>

      <H2>1. Get a key</H2>
      <P>
        In the app, open <Link href="/settings/keys" className="link-accent">Settings → API Keys</Link>{" "}
        and create one. It&apos;s shown once — copy it. A key is scoped to your organization and to
        its creator&apos;s role.
      </P>

      <H2>2. Choose how the agent connects</H2>
      <div className="mt-6 grid gap-4 sm:grid-cols-2">
        <Link
          href="/docs/mcp"
          className="card group transition hover:border-line-2 hover:shadow-[var(--shadow-lift)]"
        >
          <div className="eyebrow">MCP</div>
          <h3 className="mt-2 text-lg font-semibold text-ink">Claude, Cursor & MCP hosts</h3>
          <p className="mt-2 text-sm leading-relaxed text-ink-2">
            Point Claude Desktop, Claude Code, or any MCP client at this instance — no install
            needed. 20 tools for reading and writing docs.
          </p>
          <span className="mt-4 inline-block text-sm text-clay">Set up MCP →</span>
        </Link>
        <Link
          href="/docs/cli"
          className="card group transition hover:border-line-2 hover:shadow-[var(--shadow-lift)]"
        >
          <div className="eyebrow">CLI</div>
          <h3 className="mt-2 text-lg font-semibold text-ink">Shell-capable agents</h3>
          <p className="mt-2 text-sm leading-relaxed text-ink-2">
            The <code className="font-mono text-[0.85em]">mdm</code> command reads and writes docs
            from any terminal — ideal for coding agents that can run shell commands.
          </p>
          <span className="mt-4 inline-block text-sm text-clay">Use the CLI →</span>
        </Link>
      </div>

      <H2>3. Give your agent context</H2>
      <P>
        Paste this into your agent&apos;s instructions (system prompt, rules file, or project
        context) so it knows md-manager exists and how to use it. It assumes the{" "}
        <code className="font-mono text-[0.85em]">mdm</code> CLI is available and signed in
        (see <Link href="/docs/cli" className="link-accent">CLI</Link>).
      </P>
      <div className="mt-4">
        <CodeBlock filename="agent context — copy & paste" code={AGENT_CONTEXT} />
      </div>
      <P>
        Using MCP instead of the CLI? The same idea applies — your agent calls the tools{" "}
        <code className="font-mono text-[0.85em]">search_docs</code>,{" "}
        <code className="font-mono text-[0.85em]">get_doc</code>,{" "}
        <code className="font-mono text-[0.85em]">create_doc</code>,{" "}
        <code className="font-mono text-[0.85em]">update_doc</code>, and{" "}
        <code className="font-mono text-[0.85em]">append_to_doc</code>. Tell it to search before
        creating and to persist durable knowledge as docs.
      </P>

      <P>
        <span className="text-ink-soft">This instance&apos;s API:</span>{" "}
        <code className="font-mono text-[0.85em] text-clay-dark">{API}</code>
      </P>
    </>
  );
}
