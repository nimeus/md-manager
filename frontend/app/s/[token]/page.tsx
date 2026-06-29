import Link from "next/link";
import { redirect } from "next/navigation";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

import Logo from "@/components/logo";
import { getSharedDoc } from "@/lib/api";

export const metadata = { title: "Shared document — md-manager" };

export default async function SharedPage({
  params,
}: {
  params: Promise<{ token: string }>;
}) {
  const { token } = await params;
  const { status, data } = await getSharedDoc(token);
  // Private link, viewer not signed in → sign in and come back.
  if (status === 401) {
    redirect(`/login?next=/s/${encodeURIComponent(token)}`);
  }

  return (
    <div className="min-h-screen bg-paper">
      <header className="sticky top-0 z-30 border-b border-line/70 bg-paper/85 backdrop-blur">
        <div className="mx-auto flex h-14 max-w-3xl items-center justify-between px-6">
          <Link href="/" className="transition hover:opacity-80">
            <Logo />
          </Link>
          <span className="font-mono text-xs text-ink-soft">read-only · shared</span>
        </div>
      </header>
      <main className="mx-auto max-w-3xl px-6 py-10">
        {status === 200 && data ? (
          <article className="prose-md">
            <div className="mb-2 font-mono text-xs text-ink-soft">{data.path}</div>
            <ReactMarkdown remarkPlugins={[remarkGfm]}>{data.content}</ReactMarkdown>
          </article>
        ) : (
          <div className="card mx-auto max-w-md text-center">
            <h1 className="text-lg font-semibold text-ink">
              {status === 403 ? "You don't have access" : "Link unavailable"}
            </h1>
            <p className="mt-2 text-sm leading-relaxed text-ink-2">
              {status === 403
                ? "This shared document is restricted, and your account isn't on its list."
                : "This link is invalid, expired, or was revoked."}
            </p>
            <Link href="/" className="btn-ghost mt-5 inline-flex">
              Go home
            </Link>
          </div>
        )}
      </main>
    </div>
  );
}
