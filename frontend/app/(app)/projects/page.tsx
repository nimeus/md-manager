import Link from "next/link";

import EmptyState from "@/components/empty-state";
import PageHeader from "@/components/page-header";
import { createProjectAction } from "@/lib/actions";
import { api } from "@/lib/api";

function NewProjectForm() {
  return (
    <form action={createProjectAction} className="flex flex-wrap items-end gap-3">
      <div className="w-full sm:w-44">
        <label className="label" htmlFor="slug">
          Slug
        </label>
        <input id="slug" name="slug" className="input" placeholder="handbook" required />
      </div>
      <div className="w-full sm:w-56">
        <label className="label" htmlFor="name">
          Name
        </label>
        <input id="name" name="name" className="input" placeholder="Team Handbook" required />
      </div>
      <button className="btn w-full sm:w-auto" type="submit">
        Create project
      </button>
    </form>
  );
}

export default async function ProjectsPage() {
  const projects: any[] = await api.listProjects();

  return (
    <div>
      <PageHeader
        eyebrow="Workspace"
        title="Projects"
        description="Containers for your documents. Each project holds a set of markdown docs your team and agents can read and write."
      />

      {projects.length === 0 ? (
        <EmptyState
          title="No projects yet"
          description="Create your first project to start adding documents."
        >
          <div className="card text-left">
            <NewProjectForm />
          </div>
        </EmptyState>
      ) : (
        <>
          <details className="card group mb-6">
            <summary className="flex cursor-pointer list-none items-center justify-between text-sm font-medium text-ink">
              New project
              <span className="text-ink-soft transition group-open:rotate-45">+</span>
            </summary>
            <div className="mt-4">
              <NewProjectForm />
            </div>
          </details>

          <div className="grid gap-4 sm:grid-cols-2">
            {projects.map((p) => (
              <Link
                key={p.id}
                href={`/projects/${p.slug}`}
                className="card group flex items-center justify-between transition hover:border-line-2 hover:shadow-[var(--shadow-lift)]"
              >
                <div className="min-w-0">
                  <div className="truncate font-medium text-ink">{p.name}</div>
                  <div className="mt-1 font-mono text-xs text-ink-soft">/{p.slug}</div>
                </div>
                <span className="text-ink-soft transition group-hover:translate-x-0.5 group-hover:text-clay">
                  →
                </span>
              </Link>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
