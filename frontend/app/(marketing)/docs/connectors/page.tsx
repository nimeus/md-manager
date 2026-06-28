import Link from "next/link";

import CodeBlock from "@/components/code-block";
import { Code, H1, H2, Lead, P } from "@/components/doc-ui";
import { apiBase } from "@/lib/docs";

export const metadata = { title: "Connectors — md-manager" };

export default function ConnectorsDocs() {
  const API = apiBase();
  return (
    <>
      <H1>Use md-manager in Claude &amp; ChatGPT</H1>
      <Lead>
        Add md-manager as a <strong>custom connector</strong> and sign in with Google. Your
        assistant gets the same 20 tools — scoped to one organization, acting with your
        permissions. No API key to copy or paste.
      </Lead>

      <H2 id="claude">Claude (claude.ai &amp; desktop app)</H2>
      <P>
        Open <strong>Settings → Connectors → Add custom connector</strong>, name it (e.g.
        “md-manager”), and paste this URL:
      </P>
      <div className="mt-4">
        <CodeBlock filename="Connector URL" code={`${API}/mcp`} />
      </div>
      <P>
        Claude opens a sign-in window: <strong>Continue with Google</strong>, choose which
        organization the connector may access, and click <strong>Allow</strong>. The tools show
        up right after — try “search md-manager for the deploy runbook.”
      </P>

      <H2 id="chatgpt">ChatGPT</H2>
      <P>
        Same idea — add a custom connector pointing at <Code>{`${API}/mcp`}</Code> and complete
        the Google sign-in. (Custom connectors require a paid ChatGPT plan.)
      </P>

      <H2 id="how">What happens when you connect</H2>
      <P>
        md-manager is its own OAuth 2.1 provider — there&apos;s nothing extra to run. The
        connector registers itself, you sign in with Google and pick an org, and md-manager
        issues a <strong>revocable</strong> token bound to that organization. The connector can
        only ever do what your role there allows, and you can revoke it any time from{" "}
        <Link href="/settings/keys" className="link-accent">Settings → API Keys</Link>.
      </P>

      <H2 id="key">Prefer a key? (Claude Desktop config)</H2>
      <P>
        You can also wire it up with an API key instead of signing in — handy for scripted or
        headless setups. See <Link href="/docs/mcp" className="link-accent">MCP</Link> for the{" "}
        <Code>mcp-remote</Code> config. Both paths reach the exact same tools and rules.
      </P>
    </>
  );
}
