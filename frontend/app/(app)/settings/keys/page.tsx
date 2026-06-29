import Link from "next/link";

import KeyCreator from "@/components/key-creator";
import PageHeader from "@/components/page-header";
import {
  revokeKeyAction,
  revokeOAuthGrantAction,
  switchOAuthGrantAction,
} from "@/lib/actions";
import { api } from "@/lib/api";

export default async function KeysPage() {
  const [keys, grants, orgs]: [any[], any[], any[]] = await Promise.all([
    api.listKeys(),
    api.listOAuthGrants().catch(() => []),
    api.myOrgs().catch(() => []),
  ]);

  return (
    <div>
      <PageHeader
        eyebrow="Settings"
        title="API keys"
        description="Keys authenticate the CLI and AI agents (MCP). Each key is clamped to its creator’s role and shown only once."
      />

      <KeyCreator />

      <div className="mt-4 rounded-lg border border-line-2 bg-clay-soft/25 px-4 py-3 text-sm text-ink-2">
        Use a key to connect an agent:{" "}
        <Link href="/docs/mcp" className="link-accent">Claude &amp; MCP hosts</Link>,{" "}
        <Link href="/docs/cli" className="link-accent">the CLI</Link>, or grab{" "}
        <Link href="/docs" className="link-accent">ready-made agent instructions</Link>.
      </div>

      <h2 className="mt-8 mb-2 text-sm font-medium text-ink-soft">Keys</h2>
      <div className="divide-y divide-line overflow-hidden rounded-xl border border-line bg-panel">
        {keys.length === 0 && (
          <p className="px-4 py-8 text-center text-sm text-ink-soft">No keys yet.</p>
        )}
        {keys.map((k) => (
          <div key={k.id} className="flex items-center justify-between px-4 py-3">
            <div>
              <div className="text-sm font-medium">
                {k.name}{" "}
                <span className="ml-1 rounded bg-paper-2 px-2 py-0.5 text-xs text-ink-soft">{k.role}</span>
                {k.revoked_at && <span className="ml-2 text-xs text-red-600">revoked</span>}
              </div>
              <div className="font-mono text-xs text-ink-soft">{k.key_prefix}…</div>
            </div>
            {!k.revoked_at && (
              <form action={revokeKeyAction}>
                <input type="hidden" name="id" value={k.id} />
                <button className="text-xs text-red-600 hover:underline" type="submit">
                  Revoke
                </button>
              </form>
            )}
          </div>
        ))}
      </div>

      <ConnectedApps grants={grants} orgs={orgs} />
    </div>
  );
}

function ConnectedApps({ grants, orgs }: { grants: any[]; orgs: any[] }) {
  return (
    <>
      <h2 className="mt-8 mb-1 text-sm font-medium text-ink-soft">Connected apps</h2>
      <p className="mb-2 text-xs text-ink-soft">
        Assistants (Claude, ChatGPT) connected via OAuth. Switch which organization a connection
        uses — no reconnect needed — or revoke it.
      </p>
      <div className="divide-y divide-line overflow-hidden rounded-xl border border-line bg-panel">
        {grants.length === 0 && (
          <p className="px-4 py-8 text-center text-sm text-ink-soft">
            No connected apps yet. Add md-manager as a connector in Claude or ChatGPT —{" "}
            <Link href="/docs/connectors" className="link-accent">
              see the docs
            </Link>
            .
          </p>
        )}
        {grants.map((g) => (
          <div
            key={g.client_id + g.org_id}
            className="flex flex-col gap-3 px-4 py-3 sm:flex-row sm:items-center sm:justify-between"
          >
            <div className="min-w-0">
              <div className="text-sm font-medium">{g.client_name}</div>
              <div className="text-xs text-ink-soft">
                {g.all_orgs ? (
                  <span className="text-ink-2">All organizations</span>
                ) : (
                  <>
                    org: <span className="text-ink-2">{g.org_name}</span>
                  </>
                )}
                {g.last_used_at
                  ? ` · last used ${new Date(g.last_used_at).toLocaleDateString()}`
                  : ""}
              </div>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              {!g.all_orgs && orgs.length > 1 && (
                <form action={switchOAuthGrantAction} className="flex items-center gap-1.5">
                  <input type="hidden" name="client_id" value={g.client_id} />
                  <input type="hidden" name="from_org_id" value={g.org_id} />
                  <select
                    name="to_org_id"
                    defaultValue={g.org_id}
                    aria-label="Switch organization"
                    className="input !w-auto !py-1 !text-xs"
                  >
                    {orgs.map((o) => (
                      <option key={o.id} value={o.id}>
                        {o.name}
                      </option>
                    ))}
                  </select>
                  <button className="btn-ghost btn-sm" type="submit">
                    Switch
                  </button>
                </form>
              )}
              <form action={revokeOAuthGrantAction}>
                <input type="hidden" name="client_id" value={g.client_id} />
                <input type="hidden" name="org_id" value={g.org_id} />
                <button className="text-xs text-red-600 hover:underline" type="submit">
                  Revoke
                </button>
              </form>
            </div>
          </div>
        ))}
      </div>
    </>
  );
}
