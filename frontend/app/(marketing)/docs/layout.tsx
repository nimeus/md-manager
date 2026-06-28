import DocsSidebar from "@/components/docs-sidebar";

export default function DocsLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="mx-auto max-w-6xl px-6 py-12">
      <div className="grid gap-10 lg:grid-cols-[15rem_1fr]">
        <aside className="lg:sticky lg:top-24 lg:self-start">
          <DocsSidebar />
        </aside>
        <article className="min-w-0 max-w-3xl pb-10">{children}</article>
      </div>
    </div>
  );
}
