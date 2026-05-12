"use client";

import { useWallet } from "@/lib/wallet";

export function ConnectWallet() {
  const { address, connect, disconnect } = useWallet();

  if (address) {
    return (
      <div className="flex items-center gap-3">
        <span className="text-xs text-zinc-400 font-mono">
          {address.slice(0, 6)}…{address.slice(-4)}
        </span>
        <button
          onClick={disconnect}
          className="text-xs text-zinc-500 hover:text-white transition-colors"
        >
          Disconnect
        </button>
      </div>
    );
  }

  return (
    <button
      onClick={connect}
      className="px-4 py-2 rounded bg-hive-yellow text-black text-sm font-semibold hover:bg-yellow-400 transition-colors"
    >
      Connect Wallet
    </button>
  );
}
