import type { NativeEvent } from "./protocol.js";
type Outcome = NonNullable<NativeEvent["outcome"]>;
export function outcome(reason: string): Outcome | undefined {
  return (
    {
      completed: "completed",
      aborted: "cancelled",
      error: "failed",
      blocked: "refused",
      "max-tokens": "max_tokens",
    } as Record<string, Outcome>
  )[reason];
}
/** Native inbox claim establishes request ownership before pre-step can reject.
 * An interrupted closer is synthetic on cold reads and is never terminal proof. */
export function history(
  events: readonly any[],
  requestId: string,
): { status: "unknown" | "terminal"; outcome?: Outcome } {
  const queues: Record<string, any[]> = { "next-turn": [], "next-step": [] };
  const turns = new Map<number, Set<string>>();
  let open: number | undefined;
  for (const e of events) {
    if (e.type === "turn/start") {
      open = e.data.turn;
      turns.set(open!, new Set());
    }
    if (e.type === "agent/inbox/spliced") {
      const d = e.data,
        queue = queues[d.target];
      if (!queue) throw Error("invalid inbox target");
      const removed = queue.splice(d.start, d.removedCount ?? 0, ...d.inserted);
      if (open !== undefined && d.outcome !== "canceled")
        for (const m of removed)
          if (m.source?.rpcId) turns.get(open)!.add(m.source.rpcId);
    }
    if (e.type === "user/message" && open !== undefined && e.data.source?.rpcId)
      turns.get(open)!.add(e.data.source.rpcId);
    if (e.type === "turn/end") {
      if (turns.get(e.data.turn)?.has(requestId)) {
        const o = outcome(e.data.reason.kind);
        return o ? { status: "terminal", outcome: o } : { status: "unknown" };
      }
      open = undefined;
    }
  }
  return { status: "unknown" };
}
