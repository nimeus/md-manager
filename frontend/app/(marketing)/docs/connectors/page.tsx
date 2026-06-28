import CodeBlock from "@/components/code-block";
import { Code, H1, H2, Lead, Note, P, Ul } from "@/components/doc-ui";

export const metadata = { title: "Web connectors — md-manager" };

export default function ConnectorsDocs() {
  return (
    <>
      <H1>Web connectors</H1>
      <Lead>
        Hosted assistants like Claude.ai and ChatGPT connect over the web rather than stdio. The
        API serves the <strong>same MCP tools</strong> at <Code>POST /mcp</Code> (Streamable HTTP)
        and acts as an OAuth&nbsp;2.1 resource server, so a connector can authenticate a real user
        and act under their permissions.
      </Lead>

      <H2 id="transport">The transport</H2>
      <P>
        <Code>POST /mcp</Code> speaks the same JSON-RPC as the stdio server. With an API key you can
        exercise it today — useful for testing the endpoint before wiring OAuth:
      </P>
      <div className="mt-4">
        <CodeBlock
          filename="terminal"
          code={`curl -s https://docs.acme.com/mcp \\
  -H "authorization: Bearer mk_live_…" \\
  -H "content-type: application/json" \\
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'`}
        />
      </div>

      <H2 id="oauth">OAuth 2.1</H2>
      <P>
        For hosted connectors, the API validates JWTs issued by an authorization server — checking
        the signature against cached JWKS plus <Code>iss</Code>, <Code>exp</Code>, and the{" "}
        <Code>aud</Code> binding (RFC&nbsp;8707). It advertises the AS from a discovery endpoint and
        challenges missing tokens with <Code>WWW-Authenticate</Code>.
      </P>
      <div className="mt-4">
        <CodeBlock
          filename="discovery"
          code={`GET /.well-known/oauth-protected-resource
{
  "resource": "https://docs.acme.com",
  "authorization_servers": ["https://auth.acme.com/oidc"],
  "scopes_supported": ["mcp:read", "mcp:write"],
  "bearer_methods_supported": ["header"]
}`}
        />
      </div>
      <Note>
        Until OAuth is configured the discovery endpoint returns <Code>404</Code> by design — there
        is no authorization server to advertise. Set the variables below to turn it on.
      </Note>

      <H2 id="go-live">Going live</H2>
      <P>The self-hosted authorization server is the one external piece. To enable connectors:</P>
      <Ul>
        <li>Expose the API over public HTTPS at one canonical URL (used byte-for-byte everywhere).</li>
        <li>
          Run self-hosted <strong>Logto</strong> (the OAuth&nbsp;2.1 AS); configure a resource for
          your <Code>/mcp</Code> URL, the organizations model, and dynamic client registration.
        </li>
        <li>
          Point the API at it with environment variables, then restart:
        </li>
      </Ul>
      <div className="mt-4">
        <CodeBlock
          filename=".env"
          code={`MDM_PUBLIC_URL=https://docs.acme.com
MDM_OAUTH_ISSUER=https://auth.acme.com/oidc
MDM_OAUTH_JWKS_URL=https://auth.acme.com/oidc/jwks
MDM_OAUTH_AUDIENCE=https://docs.acme.com`}
        />
      </div>
      <P>
        Then in Claude.ai or ChatGPT, add a custom connector pointing at{" "}
        <Code>https://docs.acme.com/mcp</Code>. The assistant discovers the AS, runs the OAuth flow,
        and connects. Allow-list the assistant&apos;s callback URLs in Logto and make sure your
        proxy passes the <Code>Authorization</Code> header through.
      </P>

      <Note>
        Status: the resource server (discovery, JWKS validation, <Code>aud</Code>/<Code>iss</Code>/
        <Code>exp</Code> checks, dual API-key&nbsp;or&nbsp;JWT auth) is built and tested. Standing up
        Logto and public HTTPS is the remaining operational step to a live Claude.ai / ChatGPT
        connection.
      </Note>
    </>
  );
}
