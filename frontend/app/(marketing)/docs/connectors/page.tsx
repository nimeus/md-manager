import Link from "next/link";

import CodeBlock from "@/components/code-block";
import { Code, H1, H2, Lead, Note, P } from "@/components/doc-ui";
import { apiBase } from "@/lib/docs";

export const metadata = { title: "Connectors — md-manager" };

export default function ConnectorsDocs() {
  const API = apiBase();
  return (
    <>
      <H1>Hosted assistants</H1>
      <Lead>
        Want Claude or another assistant to use md-manager? The most reliable way today is to
        connect it as an MCP server with an API key — that works with Claude Desktop, Claude Code,
        Cursor, and anything that speaks MCP.
      </Lead>

      <H2 id="how">How to connect</H2>
      <P>
        See <Link href="/docs/mcp" className="link-accent">MCP</Link> for copy-paste setup. In
        short, point your client at this server with a key from{" "}
        <Link href="/settings/keys" className="link-accent">Settings → API Keys</Link>:
      </P>
      <div className="mt-4">
        <CodeBlock filename="MCP endpoint" code={`${API}/mcp     (Authorization: Bearer mk_live_…)`} />
      </div>

      <H2 id="raw">Call it directly</H2>
      <P>Any tool that can make an HTTP request can use it — it&apos;s plain JSON-RPC:</P>
      <div className="mt-4">
        <CodeBlock
          filename="terminal"
          code={`curl -s ${API}/mcp \\
  -H "authorization: Bearer mk_live_…" \\
  -H "content-type: application/json" \\
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'`}
        />
      </div>

      <Note>
        Native one-click connectors inside <strong>Claude.ai</strong> and <strong>ChatGPT</strong>{" "}
        use a hosted sign-in (OAuth) flow that isn&apos;t enabled on this instance yet. Until it
        is, connect through Claude Desktop or Claude Code as above — same tools, same documents.
      </Note>
    </>
  );
}
