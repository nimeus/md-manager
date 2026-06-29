import InviteForm from "@/components/invite-form";
import PageHeader from "@/components/page-header";
import {
  removeMemberAction,
  revokeInviteAction,
  updateMemberRoleAction,
} from "@/lib/actions";
import { api } from "@/lib/api";
import { getSession } from "@/lib/session";

const ROLES = ["owner", "admin", "member", "viewer"] as const;

export default async function MembersPage() {
  const session = await getSession();
  const members: any[] = await api.listMembers().catch(() => []);

  // Listing invitations is owner/admin-only on the API → also tells us if we can manage.
  let invites: any[] = [];
  let canManage = true;
  try {
    invites = await api.listInvitations();
  } catch {
    canManage = false;
  }

  const meId = session?.user.id;
  const meRole = members.find((m) => m.user_id === meId)?.role;
  const isOwner = meRole === "owner";

  return (
    <div>
      <PageHeader
        eyebrow="Settings"
        title="Members"
        description="Invite teammates with a link, and manage who's in this organization."
      />

      {canManage && (
        <>
          <div className="mt-5">
            <InviteForm />
          </div>

          {invites.length > 0 && (
            <>
              <h2 className="mt-6 text-sm font-medium text-ink-soft">Pending invitations</h2>
              <div className="mt-2 divide-y divide-line overflow-hidden rounded-lg border border-line">
                {invites.map((i) => (
                  <div key={i.id} className="flex items-center justify-between p-3">
                    <div>
                      <div className="text-sm font-medium">{i.email}</div>
                      <div className="text-xs text-ink-soft">role: {i.role}</div>
                    </div>
                    <form action={revokeInviteAction}>
                      <input type="hidden" name="id" value={i.id} />
                      <button className="text-xs text-red-600 hover:underline" type="submit">
                        Revoke
                      </button>
                    </form>
                  </div>
                ))}
              </div>
            </>
          )}
        </>
      )}

      <h2 className="mt-8 mb-2 text-sm font-medium text-ink-soft">Members</h2>
      <div className="divide-y divide-line overflow-hidden rounded-xl border border-line bg-panel">
        {members.length === 0 && (
          <p className="px-4 py-8 text-center text-sm text-ink-soft">No members.</p>
        )}
        {members.map((m) => (
          <div
            key={m.user_id}
            className="flex flex-wrap items-center justify-between gap-3 px-4 py-3"
          >
            <div className="min-w-0">
              <div className="text-sm font-medium">
                {m.display_name || m.email}
                {m.user_id === meId && (
                  <span className="ml-2 rounded bg-paper-2 px-1.5 py-0.5 text-xs text-ink-soft">
                    you
                  </span>
                )}
              </div>
              <div className="font-mono text-xs text-ink-soft">{m.email}</div>
            </div>

            {canManage ? (
              <div className="flex items-center gap-2">
                <form action={updateMemberRoleAction} className="flex items-center gap-1.5">
                  <input type="hidden" name="user_id" value={m.user_id} />
                  <select
                    name="role"
                    defaultValue={m.role}
                    aria-label="Role"
                    className="input !w-auto !py-1 !text-xs"
                  >
                    {ROLES.filter((r) => r !== "owner" || isOwner).map((r) => (
                      <option key={r} value={r}>
                        {r}
                      </option>
                    ))}
                  </select>
                  <button className="btn-ghost btn-sm" type="submit">
                    Save
                  </button>
                </form>
                <form action={removeMemberAction}>
                  <input type="hidden" name="user_id" value={m.user_id} />
                  <button className="text-xs text-red-600 hover:underline" type="submit">
                    Remove
                  </button>
                </form>
              </div>
            ) : (
              <span className="rounded bg-paper-2 px-2 py-0.5 text-xs text-ink-soft">{m.role}</span>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
