import { isAbsolute } from "node:path";
import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import { randomUUID } from "node:crypto";
import { PrivateLink } from "./private-link.js";
import type { Ready, Scope } from "./process-contract.js";
import { validScope } from "./launch-fence.js";
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
import { Channel, PeerFailure } from "./channel.js";
import { Output } from "./queue.js";
import { Deadline } from "./deadline.js";
import type { WorkerLaunchFenceStore } from "./launch-fence.js";

export interface WorkerRuntime {
  readonly launcher: string;
  readonly manifestDigest: string;
}
export function scopeAbsent(runtime: WorkerRuntime, scope: Scope): boolean {
  if (!validScope(scope) || !isAbsolute(runtime.launcher)) return false;
  const result = spawnSync(
    runtime.launcher,
    ["absent", JSON.stringify(scope)],
    { timeout: 2000, stdio: "ignore", windowsHide: true },
  );
  return result.status === 0;
}
/** Deadline-bound probe. No synchronous spawn can freeze the Host control loop. */
export async function scopeAbsentWithin(
  runtime: WorkerRuntime,
  scope: Scope,
  budget: Budget,
): Promise<boolean> {
  if (
    !validScope(scope) ||
    !isAbsolute(runtime.launcher) ||
    budget.signal.aborted ||
    budget.timeoutMs <= 0
  )
    return false;
  return new Promise((resolve) => {
    const child = spawn(runtime.launcher, ["absent", JSON.stringify(scope)], {
      stdio: "ignore",
      windowsHide: true,
    });
    let expired = false;
    const cancel = () => {
      expired = true;
      child.kill("SIGKILL");
    };
    const timer = setTimeout(cancel, Math.min(2000, budget.timeoutMs));
    const finish = (absent: boolean) => {
      clearTimeout(timer);
      budget.signal.removeEventListener("abort", cancel);
      resolve(!expired && absent);
    };
    budget.signal.addEventListener("abort", cancel, { once: true });
    if (budget.signal.aborted) cancel();
    child.once("error", () => finish(false));
    child.once("exit", (code) => finish(code === 0));
  });
}
const pause = (ms: number) =>
  new Promise<void>((resolve) => setTimeout(resolve, ms));
export class WorkerPort implements ProviderAgentPort {
  readonly launchId = randomUUID();
  private child?: ChildProcess;
  private link?: PrivateLink;
  private scope?: Scope;
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
    private readonly runtime: WorkerRuntime,
    private readonly store: WorkerLaunchFenceStore,
    private readonly namespace: Namespace,
    private readonly artifact: string,
    private readonly bridge?: ToolEndpoint,
    private activation?: unknown,
  ) {}
  async start(
    configuration: ProviderConfiguration,
    budget: Budget,
    previous: Binding | null = null,
  ): Promise<Result<void>> {
    if (this.closing || this.starting || this.child) return fail("unavailable");
    const task = this.launch(
      configuration,
      {
        ...budget,
        signal: AbortSignal.any([budget.signal, this.startupAbort.signal]),
      },
      previous,
    );
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
  /** Called after nominal admission. Execution bridges additionally require durable session publication; verification bridges expose only their harmless probe. */
  admitTools(): void {
    if (!this.closing) this.toolsAdmitted = true;
  }
  private async launch(
    configuration: ProviderConfiguration,
    budget: Budget,
    previous: Binding | null,
  ): Promise<Result<void>> {
    if (
      !isAbsolute(this.runtime.launcher) ||
      !/^[a-f0-9]{64}$/.test(this.runtime.manifestDigest) ||
      !this.artifact.startsWith("file:")
    )
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
        runtimeDigest: this.runtime.manifestDigest,
        phase: "reserved",
      });
      if (!reserved.ok) return reserved;
      this.reserved = true;
      check();
      this.child = spawn(this.runtime.launcher, ["launch", this.launchId], {
        stdio: ["pipe", "pipe", "pipe"],
        windowsHide: true,
        env: Object.fromEntries(
          [
            "PATH",
            "HOME",
            "TMPDIR",
            "SystemRoot",
            "USERPROFILE",
            "LOCALAPPDATA",
            "TEMP",
          ].flatMap((k) => (process.env[k] ? [[k, process.env[k]!]] : [])),
        ),
      });
      const ownership = new Promise<Ready>((resolve, reject) => {
        let text = "",
          received = false;
        const timer = setTimeout(
          () => reject(new Error("worker startup deadline")),
          Math.min(budget.timeoutMs, 5000),
        );
        const failed = () => {
          clearTimeout(timer);
          reject(new Error("worker startup failed"));
        };
        this.child!.once("exit", failed).once("error", failed);
        this.child!.stderr!.on("data", (bytes: Buffer) => {
          if (received) return;
          text += bytes.toString("utf8");
          if (text.length > 8192) {
            failed();
            return;
          }
          const end = text.indexOf("\n");
          if (end < 0) return;
          received = true;
          clearTimeout(timer);
          try {
            resolve(JSON.parse(text.slice(0, end)));
          } catch {
            failed();
          }
        });
      });
      // Install a rejection handler immediately, before any asynchronous startup stage.
      void ownership.catch(() => {});
      this.link = new PrivateLink(
        this.child.stdout!,
        this.child.stdin!,
        "worker",
      );
      this.child.once("exit", () => {
        this.exited = true;
        this.failed();
      });
      this.child.once("error", () => {
        this.exited = true;
        this.failed();
      });
      this.control = new Channel(this.link.lane("control"), this.launchId);
      this.output = new Channel(this.link.lane("events"), this.launchId);
      this.tools = new Channel(
        this.link.lane("tools"),
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
      const ready = await ownership;
      check();
      if (
        ready.version !== 1 ||
        ready.launchId !== this.launchId ||
        ready.launcherPid !== this.child.pid ||
        !Number.isSafeInteger(ready.workerPid) ||
        ready.workerPid <= 1 ||
        !validScope(ready.scope) ||
        ready.artifact !== this.runtime.manifestDigest ||
        (process.platform === "win32"
          ? ready.scope.kind !== "jobObject" ||
            ready.scope.name !== "Local\\rss-mdm-worker-" + this.launchId
          : ready.scope.kind !== "processGroup" ||
            ready.scope.root !== ready.launcherPid)
      )
        throw new Error("worker ownership");
      this.scope = ready.scope;
      const hello = (await this.control.call("hello", null, budget)) as {
        pid: number;
        parentPid: number;
      };
      if (
        hello.pid !== ready.workerPid ||
        hello.parentPid !== ready.launcherPid
      )
        throw new Error("worker ownership");
      const registered = await this.store.registerLaunch(
        this.namespace,
        this.launchId,
        ready.scope,
      );
      if (!registered.ok) throw new Error(registered.error.code);
      this.registered = true;
      check();
      await this.control.call(
        "activate",
        {
          artifact: this.artifact,
          configuration,
          previous,
          ...(this.activation === undefined
            ? {}
            : { activation: this.activation }),
        },
        budget,
      );
      this.activation = undefined;
      check();
      return ok(undefined);
    } catch (error) {
      return error instanceof PeerFailure
        ? fail(error.code)
        : fail("unavailable", "same_command");
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
        if (this.scope?.kind === "processGroup")
          process.kill(-this.scope.root, "SIGKILL");
        else this.child.kill("SIGKILL");
      } catch {}
    }
    this.link?.close();
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
      while (this.child) {
        const absent = this.scope
          ? await deadline.wait(() =>
              scopeAbsentWithin(this.runtime, this.scope!, deadline.budget()),
            )
          : !this.registered;
        if (this.exited && absent) break;
        await deadline.wait(() => pause(10));
      }
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
