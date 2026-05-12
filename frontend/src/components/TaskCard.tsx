import Link from "next/link";
import type { EscrowTask } from "@/lib/contract";
import { StatusBadge } from "./StatusBadge";

function fmt(address: string) {
  return `${address.slice(0, 6)}…${address.slice(-4)}`;
}

function deadline(unix: bigint) {
  return new Date(Number(unix) * 1000).toLocaleString();
}

export function TaskCard({ task }: { task: EscrowTask }) {
  return (
    <Link href={`/task/${task.taskId}`}>
      <div className="rounded-lg border border-hive-border bg-hive-card p-4 hover:border-hive-yellow/50 transition-colors cursor-pointer">
        <div className="flex items-center justify-between mb-3">
          <span className="text-xs text-zinc-500">Task #{task.taskId.toString()}</span>
          <StatusBadge status={task.status} />
        </div>

        <div className="space-y-1 text-sm">
          <Row label="Employer" value={fmt(task.employer)} />
          <Row label="Worker"   value={fmt(task.worker)} />
          <Row label="Amount"   value={`${(Number(task.amount) / 1e7).toFixed(2)} XLM`} />
          <Row label="Deadline" value={deadline(task.deadline)} />
        </div>
      </div>
    </Link>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between">
      <span className="text-zinc-500">{label}</span>
      <span className="text-zinc-200 font-mono">{value}</span>
    </div>
  );
}
