import Link from "next/link";

import EmptyState from "@/components/empty-state";
import { createDocumentAction } from "@/lib/actions";
import { api } from "@/lib/api";

function NewDocumentForm({ projectId }: { projectId: string }) {
  return (
    <form action={createDocumentAction} className="space-y-3">
      <input type="hidden" name="projectId" value={projectId} />
      <div className="flex flex-wrap gap-3">
        <div className="flex-1">
          <label className="label" htmlFor="path">
            Path
          </label>
          <input id="path" name="path" className="input" placeholder="guides/setup" required />
        </div>
        <div className="flex-1">
          <label className="label" htmlFor="title">
            Title
          </label>
          <input id="title" name="title" className="input" placeholder="Setup Guide" required />
        </div>
      </div>
      <div>
        <label className="label" htmlFor="content">
          Content (markdown)
        </label>
        <textarea
          id="content"
          name="content"
          className="input font-mono"
          rows={6}
          placeholder="# Setup Guide&#10;&#10;…"
        />
      </div>
      <button className="btn" type="submit">
        Create document
      </button>
    </form>
  );
}

export default async function ProjectPage({ params }: { params: Promise<{ slug: string }> }) {
  const { slug } = await params;
  const project: any = await api.getProject(slug);
  const docs: any[] = await api.listDocuments(project.id);

  return (
    <div>
      <Link
        href="/projects"
        className="text-sm text-ink-soft transition hover:text-clay"
      >
        ← Projects
      </Link>
      <div className="mt-3 mb-8 border-b border-line pb-5">
        <h1 className="text-2xl font-semibold tracking-tight text-ink">{project.name}</h1>
        <div className="mt-1 font-mono text-sm text-ink-soft">/{project.slug}</div>
      </div>

      {docs.length === 0 ? (
        <EmptyState
          title="No documents yet"
          description="Create the first document in this project — or have an agent do it via the CLI or MCP."
        >
          <div className="card w-full max-w-xl text-left">
            <NewDocumentForm projectId={project.id} />
          </div>
        </EmptyState>
      ) : (
        <>
          <details className="card group mb-6">
            <summary className="flex cursor-pointer list-none items-center justify-between text-sm font-medium text-ink">
              New document
              <span className="text-ink-soft transition group-open:rotate-45">+</span>
            </summary>
            <div className="mt-4">
              <NewDocumentForm projectId={project.id} />
            </div>
          </details>

          <div className="overflow-hidden rounded-xl border border-line bg-panel">
            {docs.map((d, i) => (
              <Link
                key={d.id}
                href={`/documents/${d.id}`}
                className={
                  "flex items-center justify-between px-4 py-3 transition hover:bg-paper-2/60 " +
                  (i > 0 ? "border-t border-line" : "")
                }
              >
                <div className="min-w-0">
                  <div className="truncate text-sm font-medium text-ink">{d.title}</div>
                  <div className="truncate font-mono text-xs text-ink-soft">{d.path}</div>
                </div>
                <span className="chip shrink-0">v{d.current_version}</span>
              </Link>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
