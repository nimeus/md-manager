import { createOrgAction } from "@/lib/actions";

// Top-level (outside the (app) group) so it isn't subject to the app layout's "must have an
// org" redirect — used both for the rare zero-org case and to create additional orgs.
export default function OnboardingPage() {
  return (
    <div className="flex min-h-screen items-center justify-center p-6">
      <div className="card w-full max-w-md">
        <h1 className="text-lg font-semibold">Create an organization</h1>
        <p className="mt-1 text-sm text-zinc-400">
          Organizations hold your projects and teammates. You can create more and switch between
          them anytime.
        </p>
        <form action={createOrgAction} className="mt-5 space-y-3">
          <div>
            <label className="label" htmlFor="name">
              Name
            </label>
            <input id="name" name="name" className="input" placeholder="Acme Inc" required />
          </div>
          <div>
            <label className="label" htmlFor="slug">
              Slug
            </label>
            <input
              id="slug"
              name="slug"
              className="input"
              placeholder="acme"
              pattern="[a-z0-9\-]+"
              required
            />
            <p className="mt-1 text-xs text-zinc-500">lowercase letters, digits, and hyphens</p>
          </div>
          <button className="btn w-full" type="submit">
            Create organization
          </button>
        </form>
      </div>
    </div>
  );
}
