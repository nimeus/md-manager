import Link from "next/link";

import { api } from "@/lib/api";

export default async function SearchPage({
  searchParams,
}: {
  searchParams: Promise<{ q?: string }>;
}) {
  const { q } = await searchParams;
  const results: any[] = q ? await api.search(q) : [];

  return (
    <div>
      <h1 className="text-xl font-semibold">Search</h1>
      <p className="mt-1 text-sm text-zinc-400">Keyword full-text search across your documents.</p>

      <form className="mt-5 flex gap-2">
        <input name="q" className="input" placeholder="deployment runbook…" defaultValue={q ?? ""} autoFocus />
        <button className="btn" type="submit">Search</button>
      </form>

      {q && (
        <div className="mt-6 space-y-2">
          {results.length === 0 && <p className="text-sm text-zinc-500">No matches for “{q}”.</p>}
          {results.map((h) => (
            <Link key={h.document_id} href={`/documents/${h.document_id}`} className="card block transition hover:border-zinc-700">
              <div className="flex items-center justify-between">
                <div className="font-medium">{h.title}</div>
                <span className="text-xs text-zinc-600">rank {Number(h.rank).toFixed(3)}</span>
              </div>
              <div className="text-xs text-zinc-500">{h.path}</div>
              <p className="mt-1 text-sm text-zinc-400">{h.snippet}</p>
            </Link>
          ))}
        </div>
      )}
    </div>
  );
}
