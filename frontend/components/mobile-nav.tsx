"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useEffect, useState } from "react";

export default function MobileNav({
  items,
  signedIn,
}: {
  items: [string, string][];
  signedIn: boolean;
}) {
  const [open, setOpen] = useState(false);
  const pathname = usePathname();

  // Collapse the menu after navigating.
  useEffect(() => {
    setOpen(false);
  }, [pathname]);

  return (
    <div className="md:hidden">
      <button
        type="button"
        aria-label={open ? "Close menu" : "Open menu"}
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        className="rounded-md border border-line-2 bg-panel p-2 text-ink-2 transition hover:bg-paper-2"
      >
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
          {open ? <path d="M6 6l12 12M18 6L6 18" /> : <path d="M3 6h18M3 12h18M3 18h18" />}
        </svg>
      </button>

      {open && (
        <>
          {/* Header sets backdrop-filter, which makes it the containing block for
              fixed descendants — so size the overlay by height, not bottom:0. */}
          <div
            className="fixed inset-x-0 top-16 z-40 h-[100dvh] bg-ink/30 backdrop-blur-sm"
            onClick={() => setOpen(false)}
            aria-hidden
          />
          <div className="fixed inset-x-0 top-16 z-50 border-b border-line bg-paper/95 px-6 py-4 shadow-[var(--shadow-soft)] backdrop-blur">
            <nav className="flex flex-col gap-1">
              {items.map(([href, label]) => (
                <Link
                  key={href}
                  href={href}
                  className="rounded-md px-3 py-2.5 text-sm text-ink-2 transition hover:bg-paper-2 hover:text-ink"
                >
                  {label}
                </Link>
              ))}
              <div className="mt-2 flex flex-col gap-2 border-t border-line pt-3">
                {signedIn ? (
                  <Link href="/projects" className="btn btn-sm w-full">
                    Open app →
                  </Link>
                ) : (
                  <>
                    <Link href="/login" className="btn-ghost btn-sm w-full">
                      Sign in
                    </Link>
                    <Link href="/login" className="btn btn-sm w-full">
                      Get started
                    </Link>
                  </>
                )}
              </div>
            </nav>
          </div>
        </>
      )}
    </div>
  );
}
