import KeyCreator from "@/components/key-creator";
import PageHeader from "@/components/page-header";
import { revokeKeyAction } from "@/lib/actions";
import { api } from "@/lib/api";

export default async function KeysPage() {
  const keys: any[] = await api.listKeys();

  return (
    <div>
      <PageHeader
        eyebrow="Settings"
        title="API keys"
        description="Keys authenticate the CLI and AI agents (MCP). Each key is clamped to its creator’s role and shown only once."
      />

      <KeyCreator />

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
    </div>
  );
}
