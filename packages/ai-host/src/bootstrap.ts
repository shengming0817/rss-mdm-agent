import { Socket } from "node:net";
import { execFileSync } from "node:child_process";
import type {
  Binding,
  Budget,
  Command,
  CommandRecord,
  DispatchAttempt,
  ProviderAgentPort,
  ProviderConfiguration,
  ToolEndpoint,
} from "@rss-mdm-agent/ai-contract";
import type { WorkerFactory } from "./worker.js";
import { Channel } from "./channel.js";

const launchId = process.argv[2];
if (!launchId || process.platform === "win32") process.exit(2);
// This bootstrap imports no provider SDK and opens no credential before activation.
const pgid = Number(
  execFileSync("/bin/ps", ["-o", "pgid=", "-p", String(process.pid)], {
    encoding: "utf8",
  }).trim(),
);
let port: ProviderAgentPort | undefined,
  active = false,
  stopped = false;
const observations = new Set<AbortController>();
const output = new Channel(
  new Socket({ fd: 4, readable: true, writable: true }),
  launchId,
);
const tools = new Channel(
  new Socket({ fd: 5, readable: true, writable: true }),
  launchId,
);
const bridge: ToolEndpoint = {
  propose: async (proposal, budget) => {
    if (stopped)
      return { ok: false, error: { code: "unavailable", retry: "never" } };
    return (await tools.call("propose", proposal, budget)) as Awaited<
      ReturnType<ToolEndpoint["propose"]>
    >;
  },
};
const control = new Channel(
  new Socket({ fd: 3, readable: true, writable: true }),
  launchId,
  async (method, data, budget) => {
    if (stopped) throw new Error("worker stopping");
    if (method === "hello")
      return { pid: process.pid, pgid, parentPid: process.ppid };
    if (method === "activate") {
      if (active || pgid !== process.pid) throw new Error("invalid activation");
      active = true;
      const input = data as {
        artifact: string;
        activation?: unknown;
        previous: Binding | null;
        configuration: ProviderConfiguration;
      };
      if (!input.artifact.startsWith("file:"))
        throw new Error("worker artifact");
      const module = (await import(input.artifact)) as {
        createProvider: WorkerFactory;
      };
      port = await module.createProvider({
        configuration: input.configuration,
        previous: input.previous,
        activation: input.activation,
        ...(input.configuration.permissions === "host_mediated"
          ? { tools: bridge }
          : {}),
      });
      if (stopped || budget.signal.aborted) {
        await port.close({
          timeoutMs: 1000,
          signal: new AbortController().signal,
        });
        throw new Error("late activation");
      }
      return true;
    }
    if (!port) throw new Error("worker not activated");
    const args = data as unknown[];
    switch (method) {
      case "createSession":
        return port.createSession(args[0] as ProviderConfiguration, budget);
      case "resume":
        if (!port.resume) throw new Error("resume unsupported");
        return port.resume(
          args[0] as Binding,
          args[1] as ProviderConfiguration,
          budget,
        );
      case "dispatch":
        return port.dispatch(
          args[0] as Binding,
          args[1] as Command,
          args[2] as DispatchAttempt,
          budget,
        );
      case "reconcile":
        return port.reconcile(
          args[0] as Binding,
          args[1] as CommandRecord,
          budget,
        );
      case "observe": {
        if (observations.size >= 4) throw new Error("observation limit");
        const abort = new AbortController();
        observations.add(abort);
        const [binding, id] = args as [Binding, string];
        void (async () => {
          try {
            for await (const value of port!.observe(binding, {
              timeoutMs: 2147483647,
              signal: abort.signal,
            })) {
              if (stopped || abort.signal.aborted) break;
              output.event({ id, value });
            }
          } catch {
            output.close();
          } finally {
            observations.delete(abort);
            try {
              output.event({ id, end: true });
            } catch {}
          }
        })();
        return true;
      }
      case "close": {
        for (const abort of observations) abort.abort();
        return port.close(budget);
      }
      default:
        throw new Error("unknown method");
    }
  },
);
async function orphaned() {
  if (stopped) return;
  stopped = true;
  tools.close();
  output.close();
  for (const abort of observations) abort.abort();
  // Keep the group root alive until group-wide termination. Native children never
  // inherit fds 3–5, so their lifetime cannot mask loss of the Host pipe.
  const kill = () => {
    try {
      process.kill(-process.pid, "SIGKILL");
    } catch {
      process.exit(1);
    }
  };
  const deadline = setTimeout(kill, 1500);
  try {
    await port?.close({ timeoutMs: 1000, signal: AbortSignal.timeout(1000) });
  } catch {}
  clearTimeout(deadline);
  kill();
}
control.onClose = () => {
  void orphaned();
};
output.onClose = () => {
  void orphaned();
};
tools.onClose = () => {
  void orphaned();
};
process.on("SIGTERM", () => {
  void orphaned();
});
process.on("SIGINT", () => {
  void orphaned();
});
