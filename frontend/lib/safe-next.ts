/**
 * Validate a post-auth redirect target ("return-to"). Only same-origin **relative paths** are
 * allowed — an open-redirect guard. We reject:
 *  - anything not starting with "/" (absolute URLs, `mailto:`, …)
 *  - protocol-relative "//host" (would navigate off-origin)
 *  - backslashes anywhere: the WHATWG URL parser folds "\" → "/", so "/\evil.com" resolves to
 *    "//evil.com" → a different origin
 *  - ASCII control characters (incl. tab/newline that browsers strip mid-URL)
 *
 * Because the result is guaranteed to start with a single "/" and contain no authority, building
 * `new URL(path, origin)` can never escape `origin`.
 */
export function safeNextPath(raw: string | null | undefined): string | null {
  if (!raw) return null;
  if (!raw.startsWith("/")) return null;
  if (raw.startsWith("//")) return null;
  if (raw.includes("\\")) return null;
  for (let i = 0; i < raw.length; i++) {
    const c = raw.charCodeAt(i);
    if (c <= 0x1f || c === 0x7f) return null; // ASCII control chars (incl. \t \n \r, DEL)
  }
  return raw;
}
