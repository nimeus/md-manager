import Link from "next/link";

import Editor from "@/components/editor";
import ShareBox from "@/components/share-box";
import { deleteDocumentAction, restoreVersionAction } from "@/lib/actions";
import { api } from "@/lib/api";

export default async function DocumentPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  const doc: any = await api.getDocument(id);
  const history: any[] = await api.history(id);
  const shares: any[] = await api.listShares(id).catch(() => []);

  // Bind ids into the server actions used by the inline forms below.
  const del = deleteDocumentAction.bind(null, id);

  return (
    <div className="space-y-6">
      <div>
        <Link href="/projects" className="text-sm text-ink-soft transition hover:text-clay">
          ← Projects
        </Link>
        <div className="mt-3 flex flex-wrap items-end justify-between gap-3 border-b border-line pb-5">
          <div className="min-w-0">
            <h1 className="truncate text-2xl font-semibold tracking-tight text-ink">{doc.title}</h1>
            <div className="mt-1 flex items-center gap-2 font-mono text-sm text-ink-soft">
              <span className="truncate">{doc.path}</span>
              <span className="chip">v{doc.current_version}</span>
            </div>
          </div>
          <form action={del}>
            <button className="btn-ghost text-red-600 hover:border-red-300 hover:bg-red-50" type="submit">
              Delete
            </button>
          </form>
        </div>
      </div>

      <Editor id={doc.id} initialContent={doc.content} initialVersion={doc.current_version} />

      <ShareBox docId={doc.id} shares={shares} />

      <div>
        <h2 className="text-sm font-medium text-ink-soft">Version history</h2>
        <div className="mt-2 divide-y divide-line overflow-hidden rounded-lg border border-line text-sm">
          {history.map((v) => {
            const restore = restoreVersionAction.bind(null, id, v.version);
            return (
              <div key={v.version} className="flex items-center justify-between p-3">
                <div className="flex items-center gap-3">
                  <span className="font-mono text-ink-2">v{v.version}</span>
                  <span className="rounded bg-paper-2 px-2 py-0.5 text-xs text-ink-soft">
                    {v.version_kind}
                  </span>
                  <span className="text-xs text-ink-soft">{v.actor_type}</span>
                </div>
                {v.version !== doc.current_version && (
                  <form action={restore}>
                    <button className="text-xs text-clay hover:underline" type="submit">
                      Restore
                    </button>
                  </form>
                )}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
