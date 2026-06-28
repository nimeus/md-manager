import Link from "next/link";

import Editor from "@/components/editor";
import { deleteDocumentAction, restoreVersionAction } from "@/lib/actions";
import { api } from "@/lib/api";

export default async function DocumentPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  const doc: any = await api.getDocument(id);
  const history: any[] = await api.history(id);

  // Bind ids into the server actions used by the inline forms below.
  const del = deleteDocumentAction.bind(null, id);

  return (
    <div className="space-y-6">
      <div className="flex items-start justify-between">
        <div>
          <h1 className="text-xl font-semibold">{doc.title}</h1>
          <div className="text-sm text-ink-soft">{doc.path}</div>
        </div>
        <form action={del}>
          <button className="btn-ghost text-red-600 hover:bg-red-50" type="submit">
            Delete
          </button>
        </form>
      </div>

      <Editor id={doc.id} initialContent={doc.content} initialVersion={doc.current_version} />

      <div>
        <h2 className="text-sm font-medium text-ink-soft">History</h2>
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
