import type { Metadata } from "next";
import "./globals.css";
import { WalletProvider } from "@/lib/wallet";
import { ConnectWallet } from "@/components/ConnectWallet";
import Link from "next/link";

export const metadata: Metadata = {
  title: "HiveEscrow",
  description: "M2M escrow protocol on Stellar",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>
        <WalletProvider>
          <header className="border-b border-hive-border px-6 py-4 flex items-center justify-between">
            <Link href="/" className="text-hive-yellow font-semibold tracking-widest text-sm">
              HIVE<span className="text-white">ESCROW</span>
            </Link>
            <div className="flex items-center gap-4">
              <Link href="/create" className="text-sm text-zinc-400 hover:text-white transition-colors">
                + New Task
              </Link>
              <ConnectWallet />
            </div>
          </header>
          <main className="max-w-3xl mx-auto px-6 py-10">{children}</main>
        </WalletProvider>
      </body>
    </html>
  );
}
