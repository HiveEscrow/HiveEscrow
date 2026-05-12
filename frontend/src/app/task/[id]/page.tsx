"use client";

import { useEffect, useState } from "react";
import { useParams } from "next/navigation";
import { useWallet } from "@/lib/wallet";
import {
  getTask,
  buildRefund,
  buildClaimReward,
  submitSignedTx,
  type EscrowTask,
} from "@/lib/contract";
import { StatusBadge } from "@/components/StatusBadge";
import { xdr, nativeToScVal } from "@stellar/stellar-sdk";

export default function TaskDetail() {
  const { id } = useParams<{ id: string }>();
  const { address, connect, sign } = useWallet();

  const [task, setTask] = useState<EscrowTask | null>(null);
  const [loading, setLoading] = useState(true);
  const [actionStatus, setActionStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [txHash, setTxHash] = useState<string | null>(null);

  // Claim form state (raw JSON inputs from the ZK prover)
  const [proofJson, setProofJson] = useState("");
  const [vkJson, setVkJson] = useState("");
  const [vkBytesHex, setVkBytesHex] = useState("");
  const [publicInputsJson, setPublicInputsJson] = useState("[]");

  useEffect(() => {
    getTask(BigInt(id))
      .then(setTask)
      .finally(() => setLoading(false));
  }, [id]);

  async function doRefund() {
    if (!address) { await connect(); return; }
    if (!task) return;
    setError(null);
    try {
      setActionStatus("Building…");
      const txXdr = await buildRefund(address, task.taskId);
      setActionStatus("Sign in wallet…");
      const signed = await sign(txXdr);
      setActionStatus("Submitting…");
      const hash = await submitSignedTx(signed);
      setTxHash(hash);
      setActionStatus("done");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed");
      setActionStatus(null);
    }
  }

  async function doClaim(e: React.FormEvent) {
    e.preventDefault();
    if (!address) { await connect(); return; }
    if (!task) return;
    setError(null);
    try {
      setActionStatus("Building…");

      // Parse prover outputs — these come from snarkjs/Noir/Circom
      const proof = JSON.parse(proofJson);
      const vk = JSON.parse(vkJson);
      const inputs: string[] = JSON.parse(publicInputsJson);

      const proofScVal = proofToScVal(proof);
      const vkScVal = vkToScVal(vk);
      const inputsScVal = xdr.ScVal.scvVec(
        inputs.map((hex) => xdr.ScVal.scvBytes(Buffer.from(hex.replace("0x", ""), "hex")))
      );

      const txXdr = await buildClaimReward(
        address,
        task.taskId,
        vkBytesHex,
        vkScVal,
        proofScVal,
        inputsScVal
      );
      setActionStatus("Sign in wallet…");
      const signed = await sign(txXdr);
      setActionStatus("Submitting…");
      const hash = await submitSignedTx(signed);
      setTxHash(hash);
      setActionStatus("done");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed");
      setActionStatus(null);
    }
  }

  if (loading) return <p className="text-zinc-500 text-sm">Loading…</p>;
  if (!task) return <p className="text-red-400 text-sm">Task not found or archived.</p>;

  const now = BigInt(Math.floor(Date.now() / 1000));
  const isEmployer = address === task.employer;
  const isWorker = address === task.worker;
  const canRefund = isEmployer && task.status === "Open" && now > task.deadline;
  const canClaim = isWorker && task.status === "Open" && now <= task.deadline;

  if (actionStatus === "done") {
    return (
      <div className="space-y-4">
        <p className="text-green-400 font-semibold">Transaction submitted</p>
        <code className="block text-xs break-all bg-hive-card border border-hive-border rounded p-3">{txHash}</code>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">Task #{task.taskId.toString()}</h1>
        <StatusBadge status={task.status} />
      </div>

      {/* Details */}
      <div className="rounded-lg border border-hive-border bg-hive-card p-4 space-y-2 text-sm">
        <Row label="Employer" value={task.employer} mono />
        <Row label="Worker"   value={task.worker}   mono />
        <Row label="Token"    value={task.token}     mono />
        <Row label="Amount"   value={`${(Number(task.amount) / 1e7).toFixed(7)} XLM`} />
        <Row label="Deadline" value={new Date(Number(task.deadline) * 1000).toLocaleString()} />
        <Row label="VK Hash"  value={task.vkHash} mono />
      </div>

      {error && <p className="text-red-400 text-sm">{error}</p>}
      {actionStatus && <p className="text-zinc-400 text-sm">{actionStatus}</p>}

      {/* Refund */}
      {canRefund && (
        <button
          onClick={doRefund}
          className="w-full py-2.5 border border-red-500/50 text-red-400 rounded hover:bg-red-500/10 transition-colors text-sm font-semibold"
        >
          Refund Deposit
        </button>
      )}

      {/* Claim */}
      {canClaim && (
        <form onSubmit={doClaim} className="space-y-4">
          <h2 className="text-sm font-semibold text-zinc-300">Submit ZK Proof</h2>

          <Textarea label="VK Bytes (hex)" value={vkBytesHex} onChange={setVkBytesHex}
            placeholder="Serialized verification key as hex string" rows={2} />
          <Textarea label="Verifying Key (JSON from prover)" value={vkJson} onChange={setVkJson}
            placeholder='{"alpha":{"x":"...","y":"..."},"beta":...}' rows={4} />
          <Textarea label="Proof (JSON from prover)" value={proofJson} onChange={setProofJson}
            placeholder='{"a":{"x":"...","y":"..."},"b":...,"c":...}' rows={4} />
          <Textarea label="Public Inputs (JSON array of hex strings)" value={publicInputsJson}
            onChange={setPublicInputsJson} placeholder='["0x...","0x..."]' rows={2} />

          <button
            type="submit"
            className="w-full py-2.5 bg-hive-yellow text-black font-semibold rounded hover:bg-yellow-400 transition-colors text-sm"
          >
            {address ? "Claim Reward" : "Connect Wallet"}
          </button>
        </form>
      )}
    </div>
  );
}

function Row({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="flex justify-between gap-4">
      <span className="text-zinc-500 shrink-0">{label}</span>
      <span className={`text-zinc-200 text-right break-all ${mono ? "font-mono text-xs" : ""}`}>{value}</span>
    </div>
  );
}

function Textarea({ label, value, onChange, placeholder, rows = 3 }: {
  label: string; value: string; onChange: (v: string) => void;
  placeholder?: string; rows?: number;
}) {
  return (
    <div className="space-y-1">
      <label className="text-xs text-zinc-400">{label}</label>
      <textarea
        value={value}
        onChange={(e) => onChange(e.currentTarget.value)}
        placeholder={placeholder}
        rows={rows}
        required
        className="w-full bg-hive-card border border-hive-border rounded px-3 py-2 text-xs font-mono focus:outline-none focus:border-hive-yellow resize-none"
      />
    </div>
  );
}

// ── ScVal serializers for Groth16 proof / VK ─────────────────────────────────
// These expect the JSON format output by snarkjs exportVerificationKey / proof.json

function g1ToScVal(point: { x: string; y: string }): xdr.ScVal {
  const buf = Buffer.alloc(64);
  Buffer.from(point.x.replace("0x", "").padStart(64, "0"), "hex").copy(buf, 0);
  Buffer.from(point.y.replace("0x", "").padStart(64, "0"), "hex").copy(buf, 32);
  return xdr.ScVal.scvBytes(buf);
}

function g2ToScVal(point: { x: [string, string]; y: [string, string] }): xdr.ScVal {
  const buf = Buffer.alloc(128);
  // Ethereum format: x_c1 || x_c0 || y_c1 || y_c0
  Buffer.from(point.x[1].replace("0x", "").padStart(64, "0"), "hex").copy(buf, 0);
  Buffer.from(point.x[0].replace("0x", "").padStart(64, "0"), "hex").copy(buf, 32);
  Buffer.from(point.y[1].replace("0x", "").padStart(64, "0"), "hex").copy(buf, 64);
  Buffer.from(point.y[0].replace("0x", "").padStart(64, "0"), "hex").copy(buf, 96);
  return xdr.ScVal.scvBytes(buf);
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function proofToScVal(p: any): xdr.ScVal {
  return xdr.ScVal.scvMap([
    new xdr.ScMapEntry({ key: xdr.ScVal.scvSymbol("a"), val: g1ToScVal(p.pi_a ?? p.a) }),
    new xdr.ScMapEntry({ key: xdr.ScVal.scvSymbol("b"), val: g2ToScVal(p.pi_b ?? p.b) }),
    new xdr.ScMapEntry({ key: xdr.ScVal.scvSymbol("c"), val: g1ToScVal(p.pi_c ?? p.c) }),
  ]);
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function vkToScVal(vk: any): xdr.ScVal {
  const icPoints = (vk.IC ?? vk.ic).map(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (p: any) => g1ToScVal(p)
  );
  return xdr.ScVal.scvMap([
    new xdr.ScMapEntry({ key: xdr.ScVal.scvSymbol("alpha"), val: g1ToScVal(vk.vk_alpha_1 ?? vk.alpha) }),
    new xdr.ScMapEntry({ key: xdr.ScVal.scvSymbol("beta"),  val: g2ToScVal(vk.vk_beta_2  ?? vk.beta) }),
    new xdr.ScMapEntry({ key: xdr.ScVal.scvSymbol("gamma"), val: g2ToScVal(vk.vk_gamma_2 ?? vk.gamma) }),
    new xdr.ScMapEntry({ key: xdr.ScVal.scvSymbol("delta"), val: g2ToScVal(vk.vk_delta_2 ?? vk.delta) }),
    new xdr.ScMapEntry({ key: xdr.ScVal.scvSymbol("ic"),    val: xdr.ScVal.scvVec(icPoints) }),
  ]);
}
