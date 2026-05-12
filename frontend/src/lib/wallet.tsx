"use client";

import {
  createContext,
  useContext,
  useState,
  useCallback,
  useRef,
  ReactNode,
} from "react";
import type { StellarWalletsKit as KitType } from "@creit.tech/stellar-wallets-kit";
import { NETWORK_PASSPHRASE } from "@/lib/contract";

interface WalletCtx {
  address: string | null;
  connect: () => Promise<void>;
  disconnect: () => void;
  sign: (xdr: string) => Promise<string>;
}

const WalletContext = createContext<WalletCtx | null>(null);

function getKit(): KitType {
  // Lazy import keeps StellarWalletsKit out of SSR entirely
  const {
    StellarWalletsKit,
    WalletNetwork,
    FREIGHTER_ID,
    allowAllModules,
  } = require("@creit.tech/stellar-wallets-kit");

  return new StellarWalletsKit({
    network: NETWORK_PASSPHRASE as typeof WalletNetwork,
    selectedWalletId: FREIGHTER_ID,
    modules: allowAllModules(),
  });
}

export function WalletProvider({ children }: { children: ReactNode }) {
  const [address, setAddress] = useState<string | null>(null);
  const kitRef = useRef<KitType | null>(null);

  function kit(): KitType {
    if (!kitRef.current) kitRef.current = getKit();
    return kitRef.current;
  }

  const connect = useCallback(async () => {
    const k = kit();
    await k.openModal({
      onWalletSelected: async (option: { id: string }) => {
        k.setWallet(option.id);
        const { address } = await k.getAddress();
        setAddress(address);
      },
    });
  }, []);

  const disconnect = useCallback(() => {
    kitRef.current = null;
    setAddress(null);
  }, []);

  const sign = useCallback(
    async (xdr: string) => {
      const { signedTxXdr } = await kit().signTransaction(xdr, {
        address: address ?? undefined,
        networkPassphrase: NETWORK_PASSPHRASE,
      });
      return signedTxXdr;
    },
    [address]
  );

  return (
    <WalletContext.Provider value={{ address, connect, disconnect, sign }}>
      {children}
    </WalletContext.Provider>
  );
}

export function useWallet() {
  const ctx = useContext(WalletContext);
  if (!ctx) throw new Error("useWallet must be used inside WalletProvider");
  return ctx;
}
