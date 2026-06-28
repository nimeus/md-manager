import Link from "next/link";

import { createProjectAction } from "@/lib/actions";
import { api } from "@/lib/api";

export default async function ProjectsPage() {
  const projects: any[] = await api.listProjects();

  return (
    <div>
      <h1 className="text-xl font-semibold">Projects</h1>
      <p className="mt-1 text-sm text-zinc-400">Document containers in your organization.</p>

      <form action={createProjectAction} className="mt-5 flex flex-wrap items-end gap-3">
        <div className="w-40">
          <label className="label" htmlFor="slug">Slug</label>
          <input id="slug" name="slug" className="input" placeholder="handbook" required />
        </div>
        <div className="w-56">
          <label className="label" htmlFor="name">Name</label>
          <input id="name" name="name" className="input" placeholder="Team Handbook" required />
        </div>
        <button className="btn" type="submit">Create project</button>
      </form>

      <div className="mt-6 grid gap-3 sm:grid-cols-2">
        {projects.length === 0 && (
          <p className="text-sm text-zinc-500">No projects yet — create one above.</p>
        )}
        {projects.map((p) => (
          <Link key={p.id} href={`/projects/${p.slug}`} className="card transition hover:border-zinc-700">
            <div className="font-medium">{p.name}</div>
            <div className="mt-1 text-xs text-zinc-500">/{p.slug}</div>
          </Link>
        ))}
      </div>
    </div>
  );
}
