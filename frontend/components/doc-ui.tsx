import type { ReactNode } from "react";

export const H1 = ({ children }: { children: ReactNode }) => (
  <h1 className="text-3xl font-semibold tracking-tight text-ink">{children}</h1>
);
export const Lead = ({ children }: { children: ReactNode }) => (
  <p className="mt-3 text-lg leading-relaxed text-ink-2">{children}</p>
);
export const H2 = ({ id, children }: { id?: string; children: ReactNode }) => (
  <h2
    id={id}
    className="mt-12 scroll-mt-24 border-b border-line pb-2 text-xl font-semibold text-ink"
  >
    {children}
  </h2>
);
export const H3 = ({ children }: { children: ReactNode }) => (
  <h3 className="mt-7 text-base font-semibold text-ink">{children}</h3>
);
export const P = ({ children }: { children: ReactNode }) => (
  <p className="mt-3 leading-relaxed text-ink-2">{children}</p>
);
export const Ul = ({ children }: { children: ReactNode }) => (
  <ul className="mt-3 list-disc space-y-1.5 pl-5 text-ink-2 marker:text-line-2">{children}</ul>
);
export const Note = ({ children }: { children: ReactNode }) => (
  <div className="mt-5 rounded-lg border border-line-2 bg-clay-soft/25 px-4 py-3 text-sm leading-relaxed text-ink-2">
    {children}
  </div>
);
export const Code = ({ children }: { children: ReactNode }) => (
  <code className="rounded bg-paper-2 px-1.5 py-0.5 font-mono text-[0.85em] text-clay-dark">
    {children}
  </code>
);
