import Link from "next/link";

import EmptyState from "@/components/empty-state";
import PageHeader from "@/components/page-header";
import { api } from "@/lib/api";

function SearchIcon() {
  return (
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6">
      <circle cx="11" cy="11" r="7" />
      <path d="m21 21-4.3-4.3" />
    </svg>
  );
}

export default async function SearchPage({
  searchParams,
}: {
  searchParams: Promise<{ q?: string }>;
}) {
  const { q } = await searchParams;
  const results: any[] = q ? await api.search(q) : [];

  return (
    <div>
      <PageHeader
        eyebrow="Find"
        title="Search"
        description="Full-text search across every document in this organization."
      />

      <form className="flex gap-2">
        <input
          name="q"
          className="input"
          placeholder="deployment runbook, onboarding, API keys…"
          defaultValue={q ?? ""}
          autoFocus
        />
        <button className="btn" type="submit">
          Search
        </button>
      </form>

      {!q && (
        <div className="mt-10">
          <EmptyState
            icon={<SearchIcon />}
            title="Search your documents"
            description="Type a query above. The same index powers agent search over the CLI and MCP — keyword, and semantic when embeddings are enabled."
          />
        </div>
      )}

      {q && (
        <div className="mt-6 space-y-3">
          <p className="text-sm text-ink-soft">
            {results.length} result{results.length === 1 ? "" : "s"} for &ldquo;
            <span className="text-ink-2">{q}</span>&rdquo;
          </p>
          {results.length === 0 && (
            <p className="rounded-xl border border-dashed border-line-2 bg-paper-2/30 px-4 py-8 text-center text-sm text-ink-soft">
              No matches. Try fewer or different words.
            </p>
          )}
          {results.map((h) => (
            <Link
              key={h.document_id}
              href={`/documents/${h.document_id}`}
              className="card block transition hover:border-line-2 hover:shadow-[var(--shadow-lift)]"
            >
              <div className="flex items-center justify-between gap-3">
                <div className="truncate font-medium text-ink">{h.title}</div>
                <span className="chip shrink-0">rank {Number(h.rank).toFixed(2)}</span>
              </div>
              <div className="mt-0.5 truncate font-mono text-xs text-ink-soft">{h.path}</div>
              {h.snippet && <p className="mt-2 text-sm leading-relaxed text-ink-2">{h.snippet}</p>}
            </Link>
          ))}
        </div>
      )}
    </div>
  );
}
