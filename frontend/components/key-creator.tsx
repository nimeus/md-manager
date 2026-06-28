"use client";

import { useActionState } from "react";

import { createKeyAction } from "@/lib/actions";

export default function KeyCreator() {
  const [state, action, pending] = useActionState(createKeyAction, null);

  return (
    <div className="card">
      <form action={action} className="flex flex-wrap items-end gap-3">
        <div className="w-48">
          <label className="label" htmlFor="name">Name</label>
          <input id="name" name="name" className="input" placeholder="ci-bot" required />
        </div>
        <div className="w-36">
          <label className="label" htmlFor="role">Role</label>
          <select id="role" name="role" className="input" defaultValue="member">
            <option value="viewer">viewer</option>
            <option value="member">member</option>
            <option value="admin">admin</option>
          </select>
        </div>
        <button className="btn" type="submit" disabled={pending}>
          {pending ? "Creating…" : "Create key"}
        </button>
      </form>

      {state?.error && <p className="mt-3 text-sm text-red-600">{state.error}</p>}
      {state?.secret && (
        <div className="mt-3 rounded-md border border-moss/40 bg-moss/10 p-3">
          <div className="text-xs font-medium text-moss">
            Copy this key now — it is shown only once:
          </div>
          <code className="mt-1 block break-all text-sm text-moss">{state.secret}</code>
        </div>
      )}
    </div>
  );
}
