/**
 * The public base URL of this deployment's API, shown in the docs so examples are copy-paste
 * ready for whoever is reading them. Read at runtime (the docs pages are dynamic), so it
 * reflects the live instance (`MDM_API_URL`), falling back to a placeholder in local builds.
 */
export function apiBase(): string {
  return (process.env.MDM_API_URL ?? "https://your-md-manager-api").replace(/\/+$/, "");
}
