import CodeBlock from "@/components/code-block";
import { Code, H1, H2, Lead, Note, P } from "@/components/doc-ui";
import { apiBase } from "@/lib/docs";

export const metadata = { title: "MCP — md-manager" };

const TOOLS: [string, string][] = [
  ["search_docs", "list_documents"],
  ["get_doc", "get_doc_by_path"],
  ["create_doc", "update_doc"],
  ["append_to_doc", "move_doc"],
  ["delete_doc", "restore_version"],
  ["get_doc_history", "list_projects"],
  ["create_project", "list_tags"],
  ["add_tag", "list_docs_by_tag"],
  ["list_categories", "create_category"],
  ["categorize_doc", "list_docs_by_category"],
];

export default function McpDocs() {
  const API = apiBase();
  const MCP_URL = `${API}/mcp`;
  return (
    <>
      <H1>Connect over MCP</H1>
      <Lead>
        md-manager speaks the Model Context Protocol at <Code>{MCP_URL}</Code>. Point Claude
        Desktop, Claude Code, Cursor, or any MCP client at it with an API key — 20 tools for
        reading and writing your docs. No install required.
      </Lead>

      <H2 id="claude-desktop">Claude Desktop</H2>
      <P>
        Add this to your MCP config (Claude Desktop → Settings → Developer → Edit Config). It
        bridges to the hosted server with <Code>mcp-remote</Code> (fetched on demand by{" "}
        <Code>npx</Code> — nothing to install), passing your key.
      </P>
      <div className="mt-4">
        <CodeBlock
          filename="claude_desktop_config.json"
          code={`{
  "mcpServers": {
    "md-manager": {
      "command": "npx",
      "args": [
        "-y", "mcp-remote", "${MCP_URL}",
        "--header", "Authorization: Bearer mk_live_…"
      ]
    }
  }
}`}
        />
      </div>

      <H2 id="claude-code">Claude Code</H2>
      <div className="mt-4">
        <CodeBlock
          filename="terminal"
          code={`claude mcp add --transport http md-manager ${MCP_URL} \\
  --header "Authorization: Bearer mk_live_…"`}
        />
      </div>

      <H2 id="tools">The 20 tools</H2>
      <P>Everything is scoped to your key&apos;s organization and permissions.</P>
      <div className="mt-4 overflow-hidden rounded-xl border border-line">
        <table className="w-full text-sm">
          <tbody>
            {TOOLS.map((row, i) => (
              <tr key={i} className={i % 2 ? "bg-paper-2/40" : "bg-panel"}>
                {row.map((t) => (
                  <td
                    key={t}
                    className="border-b border-line px-4 py-2 font-mono text-[13px] text-ink-2"
                  >
                    {t}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <Note>
        Prefer a local binary over <Code>npx</Code>? Build <Code>mdm-mcp</Code> from the repo
        (<Code>cargo install --path apps/mcp</Code>) and run it with{" "}
        <Code>MDM_API_URL={API}</Code> and <Code>MDM_API_KEY=mk_live_…</Code> set in its
        environment — your host launches it over stdio.
      </Note>
    </>
  );
}
