"use client";

import {
  createContext,
  useContext,
  useState,
  useCallback,
  ReactNode,
} from "react";
import {
  StellarWalletsKit,
  WalletNetwork,
  FREIGHTER_ID,
  allowAllModules,
} from "@creit.tech/stellar-wallets-kit";
import { NETWORK_PASSPHRASE } from "@/lib/contract";

interface WalletCtx {
  address: string | null;
  connect: () => Promise<void>;
  disconnect: () => void;
  sign: (xdr: string) => Promise<string>;
}

const WalletContext = createContext<WalletCtx | null>(null);

const kit = new StellarWalletsKit({
  network: NETWORK_PASSPHRASE as WalletNetwork,
  selectedWalletId: FREIGHTER_ID,
  modules: allowAllModules(),
});

export function WalletProvider({ children }: { children: ReactNode }) {
  const [address, setAddress] = useState<string | null>(null);

  const connect = useCallback(async () => {
    await kit.openModal({
      onWalletSelected: async (option) => {
        kit.setWallet(option.id);
        const { address } = await kit.getAddress();
        setAddress(address);
      },
    });
  }, []);

  const disconnect = useCallback(() => setAddress(null), []);

  const sign = useCallback(async (xdr: string) => {
    const { signedTxXdr } = await kit.signTransaction(xdr, {
      address: address ?? undefined,
      networkPassphrase: NETWORK_PASSPHRASE,
    });
    return signedTxXdr;
  }, [address]);

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
