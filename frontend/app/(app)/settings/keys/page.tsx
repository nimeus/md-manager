import KeyCreator from "@/components/key-creator";
import { revokeKeyAction } from "@/lib/actions";
import { api } from "@/lib/api";

export default async function KeysPage() {
  const keys: any[] = await api.listKeys();

  return (
    <div>
      <h1 className="text-xl font-semibold">API keys</h1>
      <p className="mt-1 text-sm text-ink-soft">
        Keys authenticate the CLI and AI agents (MCP). A key is clamped to its creator&apos;s role.
      </p>

      <div className="mt-5">
        <KeyCreator />
      </div>

      <div className="mt-6 divide-y divide-line overflow-hidden rounded-lg border border-line">
        {keys.length === 0 && <p className="p-4 text-sm text-ink-soft">No keys yet.</p>}
        {keys.map((k) => (
          <div key={k.id} className="flex items-center justify-between p-3">
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
