import Link from "next/link";

import Logo from "@/components/logo";
import { createOrgAction } from "@/lib/actions";

// Top-level (outside the (app) group) so it isn't subject to the app layout's "must have an
// org" redirect — used both for the rare zero-org case and to create additional orgs.
export default function OnboardingPage() {
  return (
    <div className="paper-texture flex min-h-screen flex-col items-center justify-center px-6">
      <Link href="/" className="mb-8 transition hover:opacity-80">
        <Logo />
      </Link>

      <div className="card w-full max-w-md">
        <span className="eyebrow">New organization</span>
        <h1 className="mt-2 text-2xl font-semibold tracking-tight text-ink">
          Name your organization
        </h1>
        <p className="mt-1.5 text-sm leading-relaxed text-ink-soft">
          Organizations hold your projects and teammates. You can create more and switch between
          them anytime.
        </p>
        <form action={createOrgAction} className="mt-6 space-y-4">
          <div>
            <label className="label" htmlFor="name">
              Name
            </label>
            <input id="name" name="name" className="input" placeholder="Acme Inc" required />
          </div>
          <div>
            <label className="label" htmlFor="slug">
              URL slug
            </label>
            <input
              id="slug"
              name="slug"
              className="input"
              placeholder="acme"
              pattern="[a-z0-9\-]+"
              required
            />
            <p className="mt-1.5 text-xs text-ink-soft">lowercase letters, digits, and hyphens</p>
          </div>
          <button className="btn-accent w-full justify-center py-2.5" type="submit">
            Create organization
          </button>
        </form>
      </div>
    </div>
  );
}
