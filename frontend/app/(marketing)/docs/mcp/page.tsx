import CodeBlock from "@/components/code-block";
import { Code, H1, H2, H3, Lead, Note, P } from "@/components/doc-ui";

export const metadata = { title: "MCP server — md-manager" };

const TOOLS: [string, string][] = [
  ["list_projects", "list_documents", ],
  ["create_project", "create_doc"],
  ["get_doc", "get_doc_by_path"],
  ["update_doc", "append_to_doc"],
  ["move_doc", "delete_doc"],
  ["restore_version", "get_doc_history"],
  ["search_docs", "list_tags"],
  ["add_tag", "list_docs_by_tag"],
  ["list_categories", "create_category"],
  ["categorize_doc", "list_docs_by_category"],
];

export default function McpDocs() {
  return (
    <>
      <H1>MCP server</H1>
      <Lead>
        A stdio Model Context Protocol server (<Code>mdm-mcp</Code>) that exposes your documents as
        20 tools. Point Claude Desktop, Claude Code, or any MCP host at it and your agent can read
        and write docs with the same key and the same rules as the CLI.
      </Lead>

      <H2 id="run">Run it</H2>
      <P>
        The server speaks JSON-RPC 2.0 over stdio and needs two environment variables — the API URL
        and a key. The host launches the process for you; you rarely run it by hand.
      </P>
      <div className="mt-4">
        <CodeBlock
          filename="terminal"
          code={`# build the binary
cargo install --path apps/mcp      # → mdm-mcp

# it reads these from the environment
MDM_API_URL=https://docs.acme.com MDM_API_KEY=mk_live_… mdm-mcp`}
        />
      </div>

      <H2 id="claude-desktop">Claude Desktop</H2>
      <P>
        Add an entry under <Code>mcpServers</Code> in your config file (on macOS:{" "}
        <Code>~/Library/Application Support/Claude/claude_desktop_config.json</Code>), then restart
        Claude.
      </P>
      <div className="mt-4">
        <CodeBlock
          filename="claude_desktop_config.json"
          code={`{
  "mcpServers": {
    "md-manager": {
      "command": "mdm-mcp",
      "env": {
        "MDM_API_URL": "https://docs.acme.com",
        "MDM_API_KEY": "mk_live_…"
      }
    }
  }
}`}
        />
      </div>

      <H2 id="claude-code">Claude Code</H2>
      <div className="mt-4">
        <CodeBlock
          filename="terminal"
          code={`claude mcp add md-manager \\
  --env MDM_API_URL=https://docs.acme.com \\
  --env MDM_API_KEY=mk_live_… \\
  -- mdm-mcp`}
        />
      </div>

      <H2 id="tools">The 20 tools</H2>
      <P>
        Read-only and destructive tools are hinted so hosts can gate them. Everything is
        org-scoped to the key&apos;s tenant.
      </P>
      <div className="mt-4 overflow-hidden rounded-xl border border-line">
        <table className="w-full text-sm">
          <tbody>
            {TOOLS.map((row, i) => (
              <tr key={i} className={i % 2 ? "bg-paper-2/40" : "bg-panel"}>
                {row.map((t) => (
                  <td key={t} className="border-b border-line px-4 py-2 font-mono text-[13px] text-ink-2">
                    {t}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <H3>Sanity-check it by hand</H3>
      <P>You can drive the server directly to confirm it&apos;s wired up:</P>
      <div className="mt-3">
        <CodeBlock
          filename="terminal"
          code={`# list the advertised tools
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' \\
  | MDM_API_URL=… MDM_API_KEY=mk_… mdm-mcp

# call one
echo '{"jsonrpc":"2.0","id":2,"method":"tools/call",
       "params":{"name":"search_docs","arguments":{"query":"deploy"}}}' \\
  | MDM_API_URL=… MDM_API_KEY=mk_… mdm-mcp`}
        />
      </div>
      <Note>
        The MCP server holds its own credentials and never forwards a client&apos;s token upstream
        — the same key, scoped to one org, governs every call. For hosted assistants that connect
        over the web instead of stdio, see <strong>Web connectors</strong>.
      </Note>
    </>
  );
}
