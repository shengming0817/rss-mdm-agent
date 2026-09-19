import { fileURLToPath } from "node:url";
import { spawn, type ChildProcess } from "node:child_process";
import { randomUUID } from "node:crypto";
import type { Duplex } from "node:stream";
import type {
  Binding,
  Budget,
  Command,
  CommandRecord,
  DispatchAttempt,
  Namespace,
  ProviderAgentPort,
  ProviderConfiguration,
  ProviderObservation,
  ProviderSessionBinding,
  Reconciliation,
  Result,
  Submission,
  ToolEndpoint,
} from "@rss-mdm-agent/ai-contract";
import { fail, ok } from "@rss-mdm-agent/ai-contract/transitions";
import { Channel } from "./channel.js";
import { Output } from "./queue.js";
import { Deadline } from "./deadline.js";
import type { WorkerLaunchFenceStore } from "./launch-fence.js";

export function groupEmpty(pgid: number): boolean {
  if (!Number.isSafeInteger(pgid) || pgid <= 1) return false;
  try {
    process.kill(-pgid, 0);
    return false;
  } catch (error) {
    return (error as NodeJS.ErrnoException).code === "ESRCH";
  }
}
const pause = (ms: number) =>
  new Promise<void>((resolve) => setTimeout(resolve, ms));
export class WorkerPort implements ProviderAgentPort {
  readonly launchId = randomUUID();
  private child?: ChildProcess;
  private control?: Channel;
  private output?: Channel;
  private tools?: Channel;
  private readonly streams = new Map<string, Output<ProviderObservation>>();
  private registered = false;
  private exited = false;
  private closing = false;
  private released = false;
  private releaseTask?: Promise<Result<void>>;
  private reserved = false;
  private closingTask?: Promise<Result<{ processStopped: boolean }>>;
  private starting?: Promise<Result<void>>;
  private readonly startupAbort = new AbortController();
  private toolsAdmitted = false;
  onFailure?: () => void;
  constructor(
    private readonly store: WorkerLaunchFenceStore,
    private readonly namespace: Namespace,
    private readonly artifact: string,
    private readonly bridge?: ToolEndpoint,
  ) {}
  async start(
    configuration: ProviderConfiguration,
    budget: Budget,
  ): Promise<Result<void>> {
    if (this.closing || this.starting || this.child) return fail("unavailable");
    const task = this.launch(configuration, {
      ...budget,
      signal: AbortSignal.any([budget.signal, this.startupAbort.signal]),
    });
    this.starting = task;
    const result = await task;
    this.starting = undefined;
    if (!result.ok)
      await this.close({
        timeoutMs: 2000,
        signal: new AbortController().signal,
      });
    return result;
  }
  /** Called by the Host only after nominal admission and durable session publication. */
  admitTools(): void {
    if (!this.closing) this.toolsAdmitted = true;
  }
  private async launch(
    configuration: ProviderConfiguration,
    budget: Budget,
  ): Promise<Result<void>> {
    if (process.platform === "win32" || !this.artifact.startsWith("file:"))
      return fail("unsupported_capability");
    const check = () => {
      if (this.closing || budget.signal.aborted)
        throw new Error("worker startup cancelled");
    };
    try {
      check();
      const reserved = await this.store.reserveLaunch({
        namespace: this.namespace,
        launchId: this.launchId,
        artifact: this.artifact,
        phase: "reserved",
      });
      if (!reserved.ok) return reserved;
      this.reserved = true;
      check();
      this.child = spawn(
        process.execPath,
        [
          fileURLToPath(new URL("./bootstrap.js", import.meta.url)),
          this.launchId,
        ],
        {
          detached: true,
          stdio: ["ignore", "ignore", "ignore", "pipe", "pipe", "pipe"],
          // Provider credentials belong to its trusted resolver, never inherited accidentally.
          env: Object.fromEntries(
            ["PATH", "HOME", "TMPDIR", "SystemRoot"].flatMap((k) =>
              process.env[k] ? [[k, process.env[k]!]] : [],
            ),
          ),
        },
      );
      this.child.once("exit", () => {
        this.exited = true;
        this.failed();
      });
      this.child.once("error", () => {
        this.exited = true;
        this.failed();
      });
      this.control = new Channel(this.child.stdio[3] as Duplex, this.launchId);
      this.output = new Channel(this.child.stdio[4] as Duplex, this.launchId);
      this.tools = new Channel(
        (this.child.stdio as unknown as Duplex[])[5],
        this.launchId,
        async (method, data, b) => {
          if (
            method !== "propose" ||
            this.closing ||
            !this.toolsAdmitted ||
            !this.bridge
          )
            return fail("permission_denied");
          const proposal = data as {
            name: string;
            arguments: Record<string, unknown>;
          };
          if (
            typeof proposal?.name !== "string" ||
            !proposal.arguments ||
            typeof proposal.arguments !== "object" ||
            Array.isArray(proposal.arguments)
          )
            return fail("invalid_input");
          return this.bridge.propose(proposal, b);
        },
      );
      this.control.onClose = () => this.failed();
      this.output.onClose = () => this.failed();
      this.tools.onClose = () => this.failed();
      this.output.onEvent = (data) => {
        const event = data as {
          id: string;
          value?: ProviderObservation;
          end?: boolean;
        };
        const stream = this.streams.get(event.id);
        if (!stream) return;
        if (event.end) {
          stream.finish();
          this.streams.delete(event.id);
        } else if (!event.value || !stream.push(event.value)) {
          this.failed();
          this.control?.close();
        }
      };
      const hello = (await this.control.call("hello", null, budget)) as {
        pid: number;
        pgid: number;
        parentPid: number;
      };
      check();
      if (
        hello.pid !== this.child.pid ||
        hello.pgid !== hello.pid ||
        hello.parentPid !== process.pid ||
        hello.pgid === process.pid
      )
        throw new Error("worker ownership");
      const registered = await this.store.registerLaunch(
        this.namespace,
        this.launchId,
        hello.pid,
        hello.pgid,
      );
      if (!registered.ok) throw new Error(registered.error.code);
      this.registered = true;
      check();
      await this.control.call(
        "activate",
        { artifact: this.artifact, configuration },
        budget,
      );
      check();
      return ok(undefined);
    } catch {
      return fail("unavailable", "same_command");
    }
  }
  private failed() {
    for (const stream of this.streams.values()) stream.end();
    this.streams.clear();
    if (!this.closing) this.onFailure?.();
  }
  private call<T>(method: string, data: unknown, budget: Budget): Promise<T> {
    if (!this.control || this.closing)
      return Promise.reject(new Error("worker unavailable"));
    return this.control.call(method, data, budget) as Promise<T>;
  }
  createSession(
    configuration: ProviderConfiguration,
    budget: Budget,
  ): Promise<Result<ProviderSessionBinding>> {
    return this.call("createSession", [configuration], budget);
  }
  resume(
    binding: Binding,
    configuration: ProviderConfiguration,
    budget: Budget,
  ): Promise<Result<ProviderSessionBinding>> {
    return this.call("resume", [binding, configuration], budget);
  }
  dispatch(
    binding: Binding,
    command: Command,
    attempt: DispatchAttempt,
    budget: Budget,
  ): Promise<Submission> {
    return this.call("dispatch", [binding, command, attempt], budget);
  }
  reconcile(
    binding: Binding,
    record: CommandRecord,
    budget: Budget,
  ): Promise<Result<Reconciliation>> {
    return this.call("reconcile", [binding, record], budget);
  }
  async *observe(
    binding: Binding,
    budget: Budget,
  ): AsyncIterable<ProviderObservation> {
    const id = randomUUID(),
      stream = new Output<ProviderObservation>();
    this.streams.set(id, stream);
    const abort = () => stream.end();
    budget.signal.addEventListener("abort", abort, { once: true });
    try {
      await this.call("observe", [binding, id], budget);
      for await (const value of stream) {
        if (budget.signal.aborted) return;
        yield value;
      }
    } finally {
      stream.end();
      this.streams.delete(id);
      budget.signal.removeEventListener("abort", abort);
    }
  }
  close(budget: Budget): Promise<Result<{ processStopped: boolean }>> {
    if (this.closingTask) return this.closingTask;
    const task = this.stop(budget);
    this.closingTask = task;
    void task
      .finally(() => {
        if (this.closingTask === task) this.closingTask = undefined;
      })
      .catch(() => {});
    return task;
  }
  /** Revoke IPC and escalate only through the current owned ChildProcess handle. */
  terminate(): void {
    this.closing = true;
    this.toolsAdmitted = false;
    this.onFailure = undefined;
    this.startupAbort.abort();
    if (this.child?.pid && !this.exited) {
      try {
        if (this.registered) process.kill(-this.child.pid, "SIGKILL");
        else this.child.kill("SIGKILL");
      } catch {}
    }
    this.tools?.close();
    this.control?.close();
    this.output?.close();
    for (const stream of this.streams.values()) stream.end();
    this.streams.clear();
  }
  private async stop(
    budget: Budget,
  ): Promise<Result<{ processStopped: boolean }>> {
    if (this.released) return ok({ processStopped: true });
    this.closing = true;
    this.toolsAdmitted = false;
    this.startupAbort.abort();
    const deadline = new Deadline(budget);
    try {
      if (this.starting) await deadline.wait(() => this.starting!);
      try {
        if (this.control && !this.control.closed && !this.exited)
          await deadline.wait(() =>
            this.control!.call("close", [], {
              ...deadline.budget(),
              timeoutMs: Math.min(500, deadline.budget().timeoutMs),
            }),
          );
      } catch {}
      this.terminate();
      while (
        this.child &&
        (!this.exited || (this.registered && !groupEmpty(this.child.pid!)))
      )
        await deadline.wait(() => pause(10));
      if (this.reserved) {
        const released = await deadline.wait(
          () =>
            (this.releaseTask ??= this.store
              .releaseLaunch(this.namespace, this.launchId)
              .then(
                (result) => {
                  if (result.ok) this.released = true;
                  else this.releaseTask = undefined;
                  return result;
                },
                (error) => {
                  this.releaseTask = undefined;
                  throw error;
                },
              )),
        );
        if (!released.ok) return released;
      }
      this.released = true;
      return ok({ processStopped: true });
    } catch {
      return ok({ processStopped: false });
    } finally {
      this.terminate();
      deadline.dispose();
    }
  }
}
