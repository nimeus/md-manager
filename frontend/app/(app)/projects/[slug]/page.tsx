import Link from "next/link";

import { createDocumentAction } from "@/lib/actions";
import { api } from "@/lib/api";

export default async function ProjectPage({ params }: { params: Promise<{ slug: string }> }) {
  const { slug } = await params;
  const project: any = await api.getProject(slug);
  const docs: any[] = await api.listDocuments(project.id);

  return (
    <div>
      <div className="flex items-baseline justify-between">
        <div>
          <Link href="/projects" className="text-xs text-zinc-500 hover:text-zinc-300">← Projects</Link>
          <h1 className="mt-1 text-xl font-semibold">{project.name}</h1>
          <div className="text-sm text-zinc-500">/{project.slug}</div>
        </div>
      </div>

      <details className="card mt-5">
        <summary className="cursor-pointer text-sm font-medium">New document</summary>
        <form action={createDocumentAction} className="mt-4 space-y-3">
          <input type="hidden" name="projectId" value={project.id} />
          <div className="flex flex-wrap gap-3">
            <div className="flex-1">
              <label className="label" htmlFor="path">Path</label>
              <input id="path" name="path" className="input" placeholder="guides/setup" required />
            </div>
            <div className="flex-1">
              <label className="label" htmlFor="title">Title</label>
              <input id="title" name="title" className="input" placeholder="Setup Guide" required />
            </div>
          </div>
          <div>
            <label className="label" htmlFor="content">Content (markdown)</label>
            <textarea id="content" name="content" className="input font-mono" rows={6} placeholder="# Setup Guide&#10;&#10;…" />
          </div>
          <button className="btn" type="submit">Create document</button>
        </form>
      </details>

      <h2 className="mt-6 text-sm font-medium text-zinc-400">Documents</h2>
      <div className="mt-2 divide-y divide-zinc-800 overflow-hidden rounded-lg border border-zinc-800">
        {docs.length === 0 && <p className="p-4 text-sm text-zinc-500">No documents yet.</p>}
        {docs.map((d) => (
          <Link key={d.id} href={`/documents/${d.id}`} className="flex items-center justify-between p-3 transition hover:bg-zinc-900">
            <div>
              <div className="text-sm font-medium">{d.title}</div>
              <div className="text-xs text-zinc-500">{d.path}</div>
            </div>
            <span className="text-xs text-zinc-500">v{d.current_version}</span>
          </Link>
        ))}
      </div>
    </div>
  );
}
