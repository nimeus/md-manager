import type { Metadata } from "next";

import "./globals.css";

export const metadata: Metadata = {
  title: "md-manager",
  description: "Markdown docs for humans and AI agents.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
