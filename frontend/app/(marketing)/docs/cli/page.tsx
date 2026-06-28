import CodeBlock from "@/components/code-block";
import { Code, H1, H2, H3, Lead, Note, P, Ul } from "@/components/doc-ui";

export const metadata = { title: "CLI — md-manager" };

export default function CliDocs() {
  return (
    <>
      <H1>
        The <span className="font-mono text-[0.8em] text-clay-dark">mdm</span> CLI
      </H1>
      <Lead>
        A single binary that talks to the md-manager API over HTTP. Built for humans and
        shell-capable agents — <Code>mdm doc get</Code> prints raw markdown to stdout, so an agent
        can pipe it straight into context.
      </Lead>

      <H2 id="install">Install</H2>
      <P>From the workspace, build the binary (named `mdm`) or run it directly with cargo:</P>
      <div className="mt-4">
        <CodeBlock
          filename="terminal"
          code={`# install the binary onto your PATH
cargo install --path apps/cli      # → mdm

# …or run without installing
cargo run -p mdm-cli -- --help`}
        />
      </div>

      <H2 id="auth">Authenticate</H2>
      <P>
        The CLI needs an API base URL and a key. Save them once with <Code>mdm auth login</Code>{" "}
        (written to <Code>~/.config/md-manager/config.json</Code>), or pass them per-command via{" "}
        <Code>--api-url</Code> / <Code>--api-key</Code> or the <Code>MDM_API_URL</Code> /{" "}
        <Code>MDM_API_KEY</Code> environment variables.
      </P>
      <div className="mt-4">
        <CodeBlock
          filename="terminal"
          code={`# save credentials
mdm auth login --api-url https://docs.acme.com --api-key mk_live_…
mdm whoami                      # confirm identity (org + role)

# first-time setup of a brand new org + admin key (needs the server's bootstrap token)
mdm bootstrap --email you@acme.com --name "You" \\
  --org-slug acme --org-name "Acme" --token "$MDM_BOOTSTRAP_TOKEN" --save`}
        />
      </div>
      <Note>
        Mint keys for your agents from the web app (<strong>Settings → API Keys</strong>) or with{" "}
        <Code>mdm keys create</Code>. A key is clamped to its creator&apos;s current role and dies
        if that access is removed.
      </Note>

      <H2 id="documents">Work with documents</H2>
      <P>
        Create, read, edit, and move documents. Body content comes from <Code>--file</Code>, an
        inline <Code>-m</Code>, or piped stdin — so agents never block on an editor.
      </P>
      <div className="mt-4">
        <CodeBlock
          filename="terminal"
          code={`# create a project + a document
mdm proj create --slug handbook --name "Team Handbook"
mdm doc create --project handbook --path runbooks/deploy \\
  --title "Deploy" -m "# Deploy\\n1. Tag the release\\n2. Ship"

# read raw markdown (pipe into an agent); add --json for metadata
mdm doc get 019f0e…                 # → markdown on stdout
mdm doc get-path --project handbook --path runbooks/deploy

# edit with optimistic concurrency; a stale version returns a 409 to merge
echo "# Deploy (v2)" | mdm doc edit 019f0e… --expected-version 1
mdm doc append 019f0e… -m "\\n## Rollback\\n…"
mdm doc mv 019f0e… runbooks/deploy-v2
mdm doc history 019f0e… && mdm doc restore 019f0e… --version 3
mdm doc rm 019f0e…                  # soft delete`}
        />
      </div>

      <H2 id="search">Search &amp; organize</H2>
      <div className="mt-4">
        <CodeBlock
          filename="terminal"
          code={`mdm search "rollback procedure"                 # keyword (full-text)
mdm search "how do we ship" --mode hybrid       # keyword + semantic (RRF)

mdm tag add 019f0e… runbook && mdm tag docs runbook
mdm cat create --slug ops --name Ops
mdm cat add 019f0e… <category-id> && mdm cat docs <category-id>`}
        />
      </div>

      <H2 id="teams">Teams, sharing &amp; admin</H2>
      <Ul>
        <li>
          <Code>mdm team</Code> — create teams and add members for bulk grants.
        </li>
        <li>
          <Code>mdm grant project|document</Code> — give a user or team a role on a project or doc.
        </li>
        <li>
          <Code>mdm share</Code> — create a public, read-only, expiring link to a document.
        </li>
        <li>
          <Code>mdm audit</Code> — admin view of who did what (filter by target or action).
        </li>
        <li>
          <Code>mdm keys</Code> — mint (shown once) and revoke API keys.
        </li>
      </Ul>

      <H2 id="agents">Tips for agents</H2>
      <Ul>
        <li>
          Data goes to <strong>stdout</strong>, logs to stderr. <Code>mdm doc get</Code> is raw
          markdown by default — perfect for piping into a prompt.
        </li>
        <li>
          Add <Code>--json</Code> to any command for structured output.
        </li>
        <li>
          Stable exit codes (0 ok, 2 usage, 3 auth, 4 not-found, 5 network) make scripting safe.
        </li>
      </Ul>
      <H3>Shell completions</H3>
      <div className="mt-3">
        <CodeBlock
          filename="terminal"
          code={`mdm completions zsh  > ~/.zfunc/_mdm        # bash | zsh | fish | powershell | elvish`}
        />
      </div>
    </>
  );
}
