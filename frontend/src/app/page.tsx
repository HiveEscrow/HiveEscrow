"use client";

import { useState } from "react";
import { TaskCard } from "@/components/TaskCard";
import { getTask, type EscrowTask } from "@/lib/contract";

export default function Dashboard() {
  const [taskId, setTaskId] = useState("");
  const [tasks, setTasks] = useState<EscrowTask[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function lookup() {
    const id = BigInt(taskId.trim());
    setLoading(true);
    setError(null);
    try {
      const task = await getTask(id);
      if (!task) {
        setError(`Task #${id} not found or archived.`);
      } else {
        setTasks((prev) => {
          const exists = prev.find((t) => t.taskId === id);
          return exists ? prev : [task, ...prev];
        });
      }
    } catch {
      setError("Failed to fetch task.");
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-2xl font-semibold mb-1">HiveEscrow</h1>
        <p className="text-zinc-500 text-sm">M2M escrow protocol · Stellar / Soroban</p>
      </div>

      {/* Task lookup */}
      <div className="flex gap-2">
        <input
          type="number"
          min={0}
          placeholder="Task ID"
          value={taskId}
          onChange={(e) => setTaskId(e.target.value)}
          className="flex-1 bg-hive-card border border-hive-border rounded px-3 py-2 text-sm focus:outline-none focus:border-hive-yellow"
        />
        <button
          onClick={lookup}
          disabled={!taskId || loading}
          className="px-4 py-2 bg-hive-yellow text-black text-sm font-semibold rounded hover:bg-yellow-400 disabled:opacity-40 transition-colors"
        >
          {loading ? "…" : "Lookup"}
        </button>
      </div>

      {error && <p className="text-red-400 text-sm">{error}</p>}

      {tasks.length > 0 && (
        <div className="space-y-3">
          {tasks.map((t) => (
            <TaskCard key={t.taskId.toString()} task={t} />
          ))}
        </div>
      )}

      {tasks.length === 0 && !error && (
        <p className="text-zinc-600 text-sm">Enter a task ID to look it up, or create a new task.</p>
      )}
    </div>
  );
}
