import Link from "next/link";

import CodeBlock from "@/components/code-block";
import { Code, H1, H2, Lead, Note, P } from "@/components/doc-ui";
import { apiBase } from "@/lib/docs";

export const metadata = { title: "CLI — md-manager" };

export default function CliDocs() {
  const API = apiBase();
  return (
    <>
      <H1>
        The <span className="font-mono text-[0.8em] text-clay-dark">mdm</span> CLI
      </H1>
      <Lead>
        Read and write your documents from any terminal. <Code>mdm doc get</Code> prints raw
        markdown to stdout, so a shell-capable agent can pipe a doc straight into its context.
      </Lead>

      <H2 id="install">Install</H2>
      <P>
        Build the <Code>mdm</Code> binary from the md-manager repository (needs Rust):
      </P>
      <div className="mt-4">
        <CodeBlock filename="terminal" code={`cargo install --path apps/cli   # installs the 'mdm' command`} />
      </div>
      <Note>
        Don&apos;t want to install anything? Most agents connect over{" "}
        <Link href="/docs/mcp" className="link-accent">MCP</Link> instead — no build required.
      </Note>

      <H2 id="sign-in">Sign in</H2>
      <P>
        Point the CLI at this instance and your key (from{" "}
        <Link href="/settings/keys" className="link-accent">Settings → API Keys</Link>). Saved to{" "}
        <Code>~/.config/md-manager/config.json</Code>.
      </P>
      <div className="mt-4">
        <CodeBlock
          filename="terminal"
          code={`mdm auth login --api-url ${API} --api-key mk_live_…
mdm whoami     # confirm your identity (org + role)`}
        />
      </div>

      <H2 id="use">Read &amp; write documents</H2>
      <div className="mt-4">
        <CodeBlock
          filename="terminal"
          code={`# find before you create
mdm search "deployment runbook"
mdm proj list

# read a document (raw markdown → stdout); add --json for metadata
mdm doc get 019f0e…
mdm doc get-path --project handbook --path runbooks/deploy

# create / update
mdm doc create --project handbook --path runbooks/deploy \\
  --title "Deploy" -m "# Deploy\\n1. Tag the release\\n2. Ship"
mdm doc edit 019f0e… -m "# Deploy (updated)"
mdm doc append 019f0e… -m "\\n## Rollback\\n…"
mdm doc mv 019f0e… runbooks/deploy-v2

# history + restore
mdm doc history 019f0e…
mdm doc restore 019f0e… --version 3`}
        />
      </div>

      <H2 id="organize">Organize</H2>
      <div className="mt-4">
        <CodeBlock
          filename="terminal"
          code={`mdm tag add 019f0e… runbook   &&  mdm tag docs runbook
mdm cat create --slug ops --name Ops
mdm search "rollback" --mode hybrid    # keyword, semantic, or hybrid`}
        />
      </div>

      <Note>
        For agents: data goes to stdout, logs to stderr, and <Code>mdm doc get</Code> is raw
        markdown — perfect for piping into a prompt. Run <Code>mdm --help</Code> for everything
        else, and grab the ready-made agent instructions on the{" "}
        <Link href="/docs" className="link-accent">overview</Link>.
      </Note>
    </>
  );
}
