const ERRORS: Record<string, string> = {
  not_configured: "Google sign-in isn't configured on the server yet.",
  invalid_state: "Sign-in expired or was tampered with — please try again.",
  signin_failed: "Could not complete sign-in. Please try again.",
  access_denied: "You cancelled the Google sign-in.",
};

export default async function LoginPage({
  searchParams,
}: {
  searchParams: Promise<{ error?: string }>;
}) {
  const { error } = await searchParams;
  return (
    <div className="flex min-h-screen items-center justify-center p-6">
      <div className="card w-full max-w-sm text-center">
        <h1 className="text-lg font-semibold">Sign in to md-manager</h1>
        <p className="mt-1 text-sm text-zinc-400">
          Markdown docs for you and your AI agents.
        </p>

        {error && (
          <p className="mt-4 rounded-md border border-red-800/60 bg-red-950/30 p-2 text-sm text-red-300">
            {ERRORS[error] ?? "Sign-in failed — please try again."}
          </p>
        )}

        {/* A plain link triggers the server-side OAuth redirect (no client JS needed). */}
        <a href="/auth/google" className="btn mt-5 w-full justify-center gap-3">
          <svg width="18" height="18" viewBox="0 0 18 18" aria-hidden="true">
            <path
              fill="#fff"
              d="M17.64 9.2c0-.64-.06-1.25-.16-1.84H9v3.48h4.84a4.14 4.14 0 0 1-1.8 2.72v2.26h2.92c1.71-1.57 2.68-3.89 2.68-6.62z"
            />
            <path
              fill="#fff"
              d="M9 18c2.43 0 4.47-.8 5.96-2.18l-2.92-2.26c-.8.54-1.84.86-3.04.86-2.34 0-4.32-1.58-5.02-3.7H.96v2.34A9 9 0 0 0 9 18z"
            />
            <path
              fill="#fff"
              d="M3.98 10.72a5.4 5.4 0 0 1 0-3.44V4.94H.96a9 9 0 0 0 0 8.12l3.02-2.34z"
            />
            <path
              fill="#fff"
              d="M9 3.58c1.32 0 2.5.45 3.44 1.35l2.58-2.58A9 9 0 0 0 .96 4.94l3.02 2.34C4.68 5.16 6.66 3.58 9 3.58z"
            />
          </svg>
          Sign in with Google
        </a>
        <p className="mt-4 text-xs text-zinc-500">
          We create your organization automatically — invite teammates once you&apos;re in.
        </p>
      </div>
    </div>
  );
}
