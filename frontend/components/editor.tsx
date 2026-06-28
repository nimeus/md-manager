"use client";

import { useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

import { saveDocumentAction } from "@/lib/actions";

type Conflict = { currentVersion: number; current: string; base: string };

export default function Editor({
  id,
  initialContent,
  initialVersion,
}: {
  id: string;
  initialContent: string;
  initialVersion: number;
}) {
  const [content, setContent] = useState(initialContent);
  const [version, setVersion] = useState(initialVersion);
  const [tab, setTab] = useState<"edit" | "preview">("edit");
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState("");
  const [conflict, setConflict] = useState<Conflict | null>(null);

  const dirty = content !== initialContent || version !== initialVersion;

  async function save(overrideVersion?: number) {
    setSaving(true);
    setStatus("");
    const result = await saveDocumentAction(id, content, overrideVersion ?? version);
    setSaving(false);
    if (result.ok) {
      setVersion(result.version);
      setConflict(null);
      setStatus(`Saved · v${result.version}`);
    } else {
      setConflict({
        currentVersion: result.currentVersion,
        current: result.current,
        base: result.base,
      });
      setStatus("Conflict — the document changed since you opened it.");
    }
  }

  return (
    <div className="card">
      <div className="mb-3 flex items-center justify-between">
        <div className="inline-flex rounded-md border border-zinc-700 p-0.5 text-sm">
          {(["edit", "preview"] as const).map((t) => (
            <button
              key={t}
              onClick={() => setTab(t)}
              className={
                "rounded px-3 py-1 capitalize transition " +
                (tab === t ? "bg-zinc-800 text-zinc-100" : "text-zinc-400 hover:text-zinc-200")
              }
            >
              {t}
            </button>
          ))}
        </div>
        <div className="flex items-center gap-3">
          <span className="text-xs text-zinc-500">{status || (dirty ? "Unsaved changes" : `v${version}`)}</span>
          <button className="btn" disabled={saving || !dirty} onClick={() => save()}>
            {saving ? "Saving…" : "Save"}
          </button>
        </div>
      </div>

      {tab === "edit" ? (
        <textarea
          className="input min-h-[26rem] font-mono leading-relaxed"
          value={content}
          onChange={(e) => setContent(e.target.value)}
          spellCheck={false}
        />
      ) : (
        <div className="prose-md min-h-[26rem] rounded-md border border-zinc-800 bg-zinc-950/40 p-4">
          <ReactMarkdown remarkPlugins={[remarkGfm]}>{content}</ReactMarkdown>
        </div>
      )}

      {conflict && (
        <div className="mt-4 rounded-md border border-amber-700/60 bg-amber-950/30 p-4">
          <div className="text-sm font-medium text-amber-300">
            Version conflict — current is v{conflict.currentVersion}
          </div>
          <p className="mt-1 text-xs text-amber-200/80">
            Your save was rejected so nothing was lost. Review the current version, then either
            load it (and re-apply your edits) or overwrite it with what you have.
          </p>
          <details className="mt-3 text-xs">
            <summary className="cursor-pointer text-amber-200">Show current version</summary>
            <pre className="mt-2 max-h-60 overflow-auto rounded bg-zinc-900 p-3 text-zinc-200">
              {conflict.current}
            </pre>
          </details>
          <div className="mt-3 flex gap-2">
            <button
              className="btn-ghost"
              onClick={() => {
                setContent(conflict.current);
                setVersion(conflict.currentVersion);
                setConflict(null);
                setStatus(`Loaded current (v${conflict.currentVersion})`);
              }}
            >
              Load current
            </button>
            <button className="btn" onClick={() => save(conflict.currentVersion)}>
              Overwrite with mine
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
