"use client";

import { useState } from "react";

export default function CodeBlock({
  code,
  filename,
  label,
}: {
  code: string;
  filename?: string;
  label?: string;
}) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      setTimeout(() => setCopied(false), 1300);
    } catch {
      /* clipboard may be blocked; ignore */
    }
  };
  return (
    <div className="overflow-hidden rounded-xl border border-line-2 bg-ink shadow-[var(--shadow-soft)]">
      <div className="flex items-center justify-between border-b border-white/10 px-4 py-2.5">
        <span className="font-mono text-xs text-paper/55">{filename ?? label ?? "shell"}</span>
        <button
          onClick={copy}
          className="font-mono text-xs text-paper/55 transition hover:text-paper"
          type="button"
        >
          {copied ? "copied ✓" : "copy"}
        </button>
      </div>
      <pre className="overflow-x-auto p-4 text-[13px] leading-relaxed text-paper">
        <code>{code}</code>
      </pre>
    </div>
  );
}
