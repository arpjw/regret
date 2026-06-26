import "./globals.css";
import type { Metadata } from "next";
import Link from "next/link";

export const metadata: Metadata = {
  title: "Regret — CFR+ Solver",
  description:
    "A CFR+ Nash equilibrium solver for Heads-Up No-Limit Hold'em with live convergence and an interactive range explorer.",
};

const NAV = [
  { href: "/", label: "Solver" },
  { href: "/explorer", label: "Range Explorer" },
  { href: "/lab", label: "Convergence Lab" },
] as const;

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body className="min-h-screen bg-ink text-zinc-100">
        <header className="border-b border-line bg-panel">
          <div className="mx-auto flex max-w-7xl items-center justify-between px-6 py-3">
            <Link href="/" className="font-mono text-sm tracking-tight">
              <span className="text-accent-purple">regret</span>
              <span className="text-zinc-500">{" // cfr+ solver"}</span>
            </Link>
            <nav className="flex items-center gap-1 text-xs font-medium uppercase tracking-wide">
              {NAV.map((item) => (
                <Link
                  key={item.href}
                  href={item.href}
                  className="rounded-md px-3 py-1.5 text-zinc-400 hover:bg-line hover:text-zinc-100"
                >
                  {item.label}
                </Link>
              ))}
            </nav>
          </div>
        </header>
        <main className="mx-auto max-w-7xl px-6 py-6">{children}</main>
      </body>
    </html>
  );
}
