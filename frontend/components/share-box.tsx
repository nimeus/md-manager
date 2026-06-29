"use client";

import { useActionState, useState } from "react";

import { createShareAction, revokeShareAction } from "@/lib/actions";

function audienceLabel(a: string) {
  return a === "members" ? "org members" : a === "emails" ? "specific people" : "public";
}

export default function ShareBox({ docId, shares }: { docId: string; shares: any[] }) {
  const [open, setOpen] = useState(false);
  const active = shares.filter((s) => !s.revoked_at);
  return (
    <div className="card">
      <div className="flex items-center justify-between gap-3">
        <div className="min-w-0">
          <h2 className="text-sm font-medium text-ink">Share</h2>
          <p className="text-xs text-ink-soft">
            Read-only links — public, org members, or specific people.
          </p>
        </div>
        <button onClick={() => setOpen((v) => !v)} className="btn-ghost btn-sm shrink-0" type="button">
          {open ? "Close" : "New link"}
        </button>
      </div>

      {open && <CreateForm docId={docId} />}

      {active.length > 0 && (
        <div className="mt-4 divide-y divide-line overflow-hidden rounded-lg border border-line">
          {active.map((s) => (
            <div key={s.id} className="flex items-center justify-between gap-3 px-3 py-2">
              <div className="min-w-0 text-sm">
                <span className="rounded bg-paper-2 px-1.5 py-0.5 text-xs text-ink-soft">
                  {audienceLabel(s.audience)}
                </span>
                <span className="ml-2 font-mono text-xs text-ink-soft">{s.token_prefix}…</span>
                {s.expires_at && (
                  <span className="ml-2 text-xs text-ink-soft">
                    · expires {new Date(s.expires_at).toLocaleDateString()}
                  </span>
                )}
              </div>
              <form action={revokeShareAction}>
                <input type="hidden" name="link_id" value={s.id} />
                <input type="hidden" name="doc_id" value={docId} />
                <button className="text-xs text-red-600 hover:underline" type="submit">
                  Revoke
                </button>
              </form>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function CreateForm({ docId }: { docId: string }) {
  const [state, action, pending] = useActionState(createShareAction, null);
  const [audience, setAudience] = useState("public");
  return (
    <form action={action} className="mt-3 space-y-3 border-t border-line pt-3">
      <input type="hidden" name="doc_id" value={docId} />
      <div>
        <label className="label" htmlFor="audience">
          Who can open it
        </label>
        <select
          id="audience"
          name="audience"
          value={audience}
          onChange={(e) => setAudience(e.target.value)}
          className="input"
        >
          <option value="public">Anyone with the link</option>
          <option value="members">Members of this organization</option>
          <option value="emails">Specific people (by email)</option>
        </select>
      </div>
      {audience === "emails" && (
        <div>
          <label className="label" htmlFor="recipients">
            Recipient emails
          </label>
          <textarea
            id="recipients"
            name="recipients"
            rows={2}
            className="input"
            placeholder="alice@x.com, bob@y.com"
          />
        </div>
      )}
      <div className="w-40">
        <label className="label" htmlFor="expires_days">
          Expires in (days)
        </label>
        <input
          id="expires_days"
          name="expires_days"
          type="number"
          min="1"
          className="input"
          placeholder="never"
        />
      </div>
      <button className="btn" type="submit" disabled={pending}>
        {pending ? "Creating…" : "Create link"}
      </button>
      {state?.error && <p className="text-sm text-red-600">{state.error}</p>}
      {state?.link && <CreatedLink token={state.link} />}
    </form>
  );
}

function CreatedLink({ token }: { token: string }) {
  const [copied, setCopied] = useState(false);
  const link =
    typeof window !== "undefined" ? `${window.location.origin}/s/${token}` : `/s/${token}`;
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(link);
      setCopied(true);
      setTimeout(() => setCopied(false), 1300);
    } catch {
      /* clipboard blocked */
    }
  };
  return (
    <div className="rounded-lg border border-line-2 bg-clay-soft/25 px-3 py-2.5">
      <p className="text-sm text-moss">Link created:</p>
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
