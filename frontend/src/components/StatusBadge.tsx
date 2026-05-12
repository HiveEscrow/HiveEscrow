import type { TaskStatus } from "@/lib/contract";

const styles: Record<TaskStatus, string> = {
  Open:     "bg-hive-yellow/20 text-hive-yellow border border-hive-yellow/40",
  Claimed:  "bg-green-500/20 text-green-400 border border-green-500/40",
  Refunded: "bg-zinc-500/20 text-zinc-400 border border-zinc-500/40",
};

export function StatusBadge({ status }: { status: TaskStatus }) {
  return (
    <span className={`px-2 py-0.5 rounded text-xs font-mono ${styles[status]}`}>
      {status}
    </span>
  );
}
