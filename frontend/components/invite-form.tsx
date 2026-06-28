"use client";

import { useActionState } from "react";

import { inviteAction } from "@/lib/actions";

export default function InviteForm() {
  const [state, action, pending] = useActionState(inviteAction, null);
  return (
    <div className="card">
      <form action={action} className="flex flex-wrap items-end gap-3">
        <div className="min-w-[12rem] flex-1">
          <label className="label" htmlFor="email">
            Teammate email
          </label>
          <input
            id="email"
            name="email"
            type="email"
            className="input"
            placeholder="teammate@company.com"
            required
          />
        </div>
        <div className="w-36">
          <label className="label" htmlFor="role">
            Role
          </label>
          <select id="role" name="role" className="input" defaultValue="member">
            <option value="admin">admin</option>
            <option value="member">member</option>
            <option value="viewer">viewer</option>
          </select>
        </div>
        <button className="btn" type="submit" disabled={pending}>
          {pending ? "Inviting…" : "Invite"}
        </button>
      </form>
      {state?.error && <p className="mt-3 text-sm text-red-600">{state.error}</p>}
      {state?.ok && <p className="mt-3 text-sm text-moss">{state.ok}</p>}
    </div>
  );
}
