"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { useWallet } from "@/lib/wallet";
import { buildCreateTask, submitSignedTx } from "@/lib/contract";

const DEADLINE_MIN_OFFSET = 48 * 60 * 60 + 60; // 48h + 1min buffer

export default function CreateTask() {
  const { address, connect, sign } = useWallet();
  const router = useRouter();

  const [form, setForm] = useState({
    worker: "",
    token: "",
    amount: "",
    vkHash: "",
    deadlineDate: "",
  });
  const [status, setStatus] = useState<"idle" | "building" | "signing" | "submitting" | "done">("idle");
  const [error, setError] = useState<string | null>(null);
  const [txHash, setTxHash] = useState<string | null>(null);

  function set(field: string, value: string) {
    setForm((f) => ({ ...f, [field]: value }));
  }

  function minDeadline() {
    return new Date(Date.now() + DEADLINE_MIN_OFFSET * 1000)
      .toISOString()
      .slice(0, 16);
  }

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!address) { await connect(); return; }
    setError(null);

    try {
      setStatus("building");
      const deadlineUnix = BigInt(Math.floor(new Date(form.deadlineDate).getTime() / 1000));
      const xdr = await buildCreateTask(
        address,
        form.worker,
        form.token,
        BigInt(Math.round(parseFloat(form.amount) * 1e7)),
        form.vkHash,
        deadlineUnix
      );

      setStatus("signing");
      const signed = await sign(xdr);

      setStatus("submitting");
      const hash = await submitSignedTx(signed);
      setTxHash(hash);
      setStatus("done");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Transaction failed.");
      setStatus("idle");
    }
  }

  if (status === "done") {
    return (
      <div className="space-y-4">
        <h1 className="text-xl font-semibold text-green-400">Task Created</h1>
        <p className="text-sm text-zinc-400">Transaction hash:</p>
        <code className="block text-xs text-zinc-300 break-all bg-hive-card border border-hive-border rounded p-3">
          {txHash}
        </code>
        <button onClick={() => router.push("/")} className="text-sm text-hive-yellow hover:underline">
          ← Back to dashboard
        </button>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <h1 className="text-xl font-semibold">Create Task</h1>

      <form onSubmit={submit} className="space-y-4">
        <Field label="Worker Address" placeholder="G…" value={form.worker} onChange={(v) => set("worker", v)} />
        <Field label="Token Contract" placeholder="C… (SAC address)" value={form.token} onChange={(v) => set("token", v)} />
        <Field label="Amount (XLM)" type="number" placeholder="10.00" value={form.amount} onChange={(v) => set("amount", v)} />
        <Field
          label="VK Hash (hex, 32 bytes)"
          placeholder="0x…"
          value={form.vkHash}
          onChange={(v) => set("vkHash", v)}
          pattern="[0-9a-fA-F]{64}"
          title="64 hex characters (32 bytes)"
        />
        <Field
          label="Deadline"
          type="datetime-local"
          value={form.deadlineDate}
          onChange={(v) => set("deadlineDate", v)}
          min={minDeadline()}
        />

        {error && <p className="text-red-400 text-sm">{error}</p>}

        <button
          type="submit"
          disabled={status !== "idle"}
          className="w-full py-2.5 bg-hive-yellow text-black font-semibold rounded hover:bg-yellow-400 disabled:opacity-40 transition-colors"
        >
          {status === "idle" && (address ? "Create Task" : "Connect Wallet")}
          {status === "building"   && "Building transaction…"}
          {status === "signing"    && "Sign in wallet…"}
          {status === "submitting" && "Submitting…"}
        </button>
      </form>
    </div>
  );
}

function Field({
  label, value, onChange, ...props
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  [k: string]: unknown;
}) {
  return (
    <div className="space-y-1">
      <label className="text-xs text-zinc-400">{label}</label>
      <input
        value={value}
        onChange={(e) => onChange(e.currentTarget.value)}
        required
        className="w-full bg-hive-card border border-hive-border rounded px-3 py-2 text-sm focus:outline-none focus:border-hive-yellow"
        {...props}
      />
    </div>
  );
}
