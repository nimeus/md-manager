import InviteForm from "@/components/invite-form";
import { revokeInviteAction } from "@/lib/actions";
import { api } from "@/lib/api";

export default async function MembersPage() {
  // Listing invitations is owner/admin-only on the API; members/viewers get a friendly note.
  let invites: any[] = [];
  let canManage = true;
  try {
    invites = await api.listInvitations();
  } catch {
    canManage = false;
  }

  return (
    <div>
      <h1 className="text-xl font-semibold">Members</h1>
      <p className="mt-1 text-sm text-ink-soft">
        Invite teammates to this organization. They join automatically when they sign in with
        Google using the invited email address.
      </p>

      {!canManage ? (
        <p className="card mt-5 text-sm text-ink-soft">
          Only owners and admins can manage members.
        </p>
      ) : (
        <>
          <div className="mt-5">
            <InviteForm />
          </div>

          <h2 className="mt-6 text-sm font-medium text-ink-soft">Pending invitations</h2>
          <div className="mt-2 divide-y divide-line overflow-hidden rounded-lg border border-line">
            {invites.length === 0 && (
              <p className="p-4 text-sm text-ink-soft">No pending invitations.</p>
            )}
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
    </div>
  );
}
