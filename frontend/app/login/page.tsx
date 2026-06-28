"use client";

import { useActionState } from "react";

import { loginAction, type FormState } from "@/lib/actions";

export default function LoginPage() {
  const [state, action, pending] = useActionState<FormState, FormData>(loginAction, null);

  return (
    <div className="flex min-h-screen items-center justify-center p-6">
      <div className="card w-full max-w-sm">
        <h1 className="text-lg font-semibold">Sign in to md-manager</h1>
        <p className="mt-1 text-sm text-zinc-400">
          Paste an API key (<code className="text-amber-300">mk_…</code>). It is stored only in
          a server-side httpOnly cookie.
        </p>
        <form action={action} className="mt-5 space-y-4">
          <div>
            <label className="label" htmlFor="apiUrl">
              API URL
            </label>
            <input
              id="apiUrl"
              name="apiUrl"
              className="input"
              defaultValue="http://127.0.0.1:8080"
              autoComplete="off"
            />
          </div>
          <div>
            <label className="label" htmlFor="apiKey">
              API key
            </label>
            <input
              id="apiKey"
              name="apiKey"
              type="password"
              className="input"
              placeholder="mk_…"
              autoComplete="off"
            />
          </div>
          {state?.error && <p className="text-sm text-red-400">{state.error}</p>}
          <button className="btn w-full" type="submit" disabled={pending}>
            {pending ? "Signing in…" : "Sign in"}
          </button>
        </form>
      </div>
    </div>
  );
}
