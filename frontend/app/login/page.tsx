import Link from "next/link";

import Logo from "@/components/logo";

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
    <div className="paper-texture flex min-h-screen flex-col items-center justify-center px-6">
      <Link href="/" className="mb-8 transition hover:opacity-80">
        <Logo />
      </Link>

      <div className="card w-full max-w-sm">
        <h1 className="text-2xl font-semibold tracking-tight text-ink">Welcome back</h1>
        <p className="mt-1.5 text-sm leading-relaxed text-ink-soft">
          Sign in to your markdown workspace — shared by your team and your AI agents.
        </p>

        {error && (
          <p className="mt-4 rounded-lg border border-red-200 bg-red-50 px-3 py-2.5 text-sm text-red-700">
            {ERRORS[error] ?? "Sign-in failed — please try again."}
          </p>
        )}

        {/* A plain link triggers the server-side OAuth redirect (no client JS needed). */}
        <a
          href="/auth/google"
          className="btn-ghost mt-6 w-full justify-center gap-3 py-2.5 text-[15px]"
        >
          <svg width="18" height="18" viewBox="0 0 18 18" aria-hidden="true">
            <path
              fill="#4285F4"
              d="M17.64 9.2c0-.637-.057-1.251-.164-1.84H9v3.481h4.844c-.209 1.125-.843 2.078-1.796 2.717v2.258h2.908c1.702-1.567 2.684-3.874 2.684-6.615z"
            />
            <path
              fill="#34A853"
              d="M9 18c2.43 0 4.467-.806 5.956-2.18l-2.908-2.259c-.806.54-1.837.86-3.048.86-2.344 0-4.328-1.584-5.036-3.711H.957v2.332A8.997 8.997 0 0 0 9 18z"
            />
            <path
              fill="#FBBC05"
              d="M3.964 10.71A5.41 5.41 0 0 1 3.682 9c0-.593.102-1.17.282-1.71V4.958H.957A8.996 8.996 0 0 0 0 9c0 1.452.348 2.827.957 4.042l3.007-2.332z"
            />
            <path
              fill="#EA4335"
              d="M9 3.58c1.321 0 2.508.454 3.44 1.345l2.582-2.58C13.463.891 11.426 0 9 0A8.997 8.997 0 0 0 .957 4.958L3.964 7.29C4.672 5.163 6.656 3.58 9 3.58z"
            />
          </svg>
          Continue with Google
        </a>

        <p className="mt-5 text-center text-xs leading-relaxed text-ink-soft">
          New here? We&apos;ll create your organization automatically.
        </p>
      </div>

      <p className="mt-6 text-xs text-ink-soft">
        Building an agent?{" "}
        <Link href="/docs" className="text-clay underline decoration-clay/30 underline-offset-2">
          Read the docs
        </Link>
      </p>
    </div>
  );
}
