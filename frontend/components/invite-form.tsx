"use client";

import { useActionState, useState } from "react";

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
          {pending ? "Creating…" : "Create invite"}
        </button>
      </form>
      {state?.error && <p className="mt-3 text-sm text-red-600">{state.error}</p>}
      {state?.ok && state.token && <InviteLink token={state.token} message={state.ok} />}
    </div>
  );
}

function InviteLink({ token, message }: { token: string; message: string }) {
  const [copied, setCopied] = useState(false);
  // The link is the secret; build it from the current public origin (works behind the proxy).
  const link =
    typeof window !== "undefined" ? `${window.location.origin}/invite/${token}` : `/invite/${token}`;
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(link);
      setCopied(true);
      setTimeout(() => setCopied(false), 1300);
    } catch {
      /* clipboard blocked; ignore */
    }
  };
  return (
    <div className="mt-3 rounded-lg border border-line-2 bg-clay-soft/25 px-3 py-2.5 text-sm">
      <p className="text-moss">{message} Send them this link (works once):</p>
      <div className="mt-1.5 flex items-center gap-2">
        <code className="min-w-0 flex-1 truncate rounded bg-panel px-2 py-1 font-mono text-xs text-ink-2">
          {link}
        </code>
        <button onClick={copy} type="button" className="btn-ghost btn-sm shrink-0">
          {copied ? "copied ✓" : "copy"}
        </button>
      </div>
    </div>
  );
}
