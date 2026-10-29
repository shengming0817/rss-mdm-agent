// ref: Node.js lib/child_process.js@v24.14.1
import { spawn, execFileSync } from "node:child_process";
import { join } from "node:path";
import { once } from "node:events";
import { createInterface } from "node:readline";
import { writeFileSync } from "node:fs";
import { setTimeout as waitMilliseconds } from "node:timers/promises";

const noMatch = (error) =>
  error.status === 1 &&
  !error.signal &&
  !String(error.stdout ?? "").trim() &&
  !String(error.stderr ?? "").trim();
export function processField(pid, field, run = execFileSync) {
  try {
    const value = run("/bin/ps", ["-p", String(pid), "-o", field + "="], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
      timeout: 2000,
    }).trim();
    if (!value) throw Error("empty process identity observation");
    return value;
  } catch (error) {
    if (noMatch(error)) return undefined;
    throw error;
  }
}
export function processChildren(pid, run = execFileSync) {
  try {
    const values = run("/usr/bin/pgrep", ["-P", String(pid)], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
      timeout: 2000,
    })
      .trim()
      .split(/\s+/)
      .map(Number);
    if (values.some((pid) => !Number.isSafeInteger(pid) || pid <= 0))
      throw Error("invalid process enumeration");
    return values;
  } catch (error) {
    if (noMatch(error)) return [];
    throw error;
  }
}

// This owner creates the detached actor itself, before any readiness/registration wait.
// ref: Node.js child_process detached: killing the executor cannot reap another group.
export async function runOwnedScopeProbe(
  node,
  source,
  input,
  register,
  cleanup,
  signal,
  timeoutMs = 30000,
) {
  const actorSource = `const {spawn}=require('node:child_process');
    const child=spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{stdio:'ignore'});
    child.once('spawn',()=>console.log(JSON.stringify({pid:child.pid})));setInterval(()=>{},1000);`;
  const registrationDelay = input.actorRegistrationDelayMs ?? 0;
  if (
    !Number.isInteger(registrationDelay) ||
    registrationDelay < 0 ||
    registrationDelay > 5000
  )
    throw Error("actor registration delay invalid");
  const actor = spawn(node, ["-e", actorSource], {
    detached: true,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let child, proof, timer, rejectStop;
  const stopped = new Promise((_, reject) => {
    rejectStop = reject;
  });
  stopped.catch(() => {});
  const abort = () => rejectStop(Error("owned scope probe cancelled"));
  signal?.addEventListener("abort", abort, { once: true });
  timer = setTimeout(
    () => rejectStop(Error("owned scope probe deadline exceeded")),
    timeoutMs,
  );
  actor.once("error", rejectStop);
  const lines = createInterface({ input: actor.stdout });
  try {
    if (signal?.aborted) abort();
    const [line] = await Promise.race([once(lines, "line"), stopped]);
    const pid = JSON.parse(line).pid;
    if (!Number.isSafeInteger(pid) || pid <= 1)
      throw Error("actor descendant invalid");
    proof = {
      kind: "ownedScope",
      scope: { kind: "processGroup", root: actor.pid },
      anchor: { pid, start: processField(pid, "lstart") },
    };
    if (!proof.anchor.start || Number(processField(pid, "pgid")) !== actor.pid)
      throw Error("actor identity unavailable");
    if (input.actorRecoveryPath)
      writeFileSync(
        input.actorRecoveryPath,
        JSON.stringify({ ...proof, parent: actor.pid, child: pid }),
        { mode: 0o600, flag: "wx" },
      );
    if (registrationDelay)
      await Promise.race([waitMilliseconds(registrationDelay), stopped]);
    await Promise.race([register(proof, process.pid), stopped]);
    child = spawn(
      node,
      [
        "--input-type=module",
        "-e",
        source,
        JSON.stringify({ ...input, actor: proof }),
      ],
      {
        detached: true,
        stdio: ["ignore", "pipe", "pipe"],
      },
    );
    const result = new Promise((resolve, reject) => {
      let stdout = "",
        stderr = "";
      child.once("error", reject);
      child.stdout.on("data", (bytes) => {
        stdout += bytes;
        if (stdout.length > 1024 * 1024)
          reject(Error("scope probe output bound"));
      });
      child.stderr.on("data", (bytes) => {
        stderr = (stderr + bytes).slice(-65536);
      });
      child.once("close", (code) => {
        if (code !== 0) reject(Error("owned scope probe failed: " + stderr));
        else {
          try {
            resolve(JSON.parse(stdout));
          } catch (error) {
            reject(error);
          }
        }
      });
    });
    return await Promise.race([result, stopped]);
  } finally {
    clearTimeout(timer);
    signal?.removeEventListener("abort", abort);
    lines.close();
    try {
      if (
        child &&
        !(await reapOwnedProcessGroup(child, { graceMs: 0, terminateMs: 500 }))
          .confirmed
      )
        throw Error("owned scope executor cleanup unconfirmed");
    } finally {
      try {
        const parentAlive =
          actor.exitCode === null && actor.signalCode === null;
        let groupAlive = !!actor.pid;
        if (actor.pid) {
          try {
            process.kill(-actor.pid, 0);
          } catch (error) {
            if (error.code === "ESRCH") groupAlive = false;
            else throw error;
          }
        }
        if (
          groupAlive &&
          !parentAlive &&
          (!proof ||
            processField(proof.anchor.pid, "lstart") !== proof.anchor.start ||
            Number(processField(proof.anchor.pid, "pgid")) !== actor.pid)
        )
          throw Error("actor scope ownership unconfirmed");
        if (parentAlive || groupAlive) {
          if (
            !(
              await reapOwnedProcessGroup(actor, {
                graceMs: 0,
                terminateMs: 500,
              })
            ).confirmed
          )
            throw Error("actor group cleanup unconfirmed");
        } else {
          actor.stdout?.destroy();
          actor.stderr?.destroy();
        }
      } finally {
        await cleanup();
      }
    }
  }
}

// Tauri, Vite and the app share a dedicated group, including when an IDE only
// signals this wrapper. Reap the leader and bound cleanup of remaining children.
export function runDesktop(
  root,
  directory,
  args = [],
  environment = process.env,
) {
  const env = { ...environment };
  if (directory === undefined) delete env.RSS_AI_HOST_RUNTIME;
  else env.RSS_AI_HOST_RUNTIME = directory;
  return runPreparation(
    "pnpm",
    ["exec", "tauri", "dev", ...args],
    join(root, "apps/desktop"),
    env,
  );
}

// Event callbacks only report state. The same bounded reaper owns every signal/probe.
export async function runPreparation(
  command,
  args,
  cwd,
  environment = process.env,
  { spawnChild = spawn, signals = process, reap = reapOwnedProcessGroup } = {},
) {
  const child = spawnChild(command, args, {
    cwd,
    stdio: "inherit",
    detached: true,
    env: environment,
  });
  let cancellation = null,
    spawnError,
    pendingSignal;
  let notify;
  const stopped = new Promise((resolve) => {
    notify = resolve;
  });
  const onExit = () => notify();
  const onError = (error) => {
    spawnError = error.code ?? "spawn_failed";
    notify();
  };
  const handlers = new Map(
    ["SIGINT", "SIGTERM", "SIGHUP"].map((signal) => [
      signal,
      () => {
        cancellation ??= signal;
        pendingSignal = signal;
        notify();
      },
    ]),
  );
  child.once("exit", onExit);
  child.once("error", onError);
  for (const [signal, handler] of handlers) signals.on(signal, handler);
  try {
    await stopped;
    pendingSignal = undefined;
    const cleanup = await reap(child, {
      groupSignal: cancellation ?? "SIGTERM",
      terminateMs: 5000,
      killMs: 1000,
      takeSignal() {
        const signal = pendingSignal;
        pendingSignal = undefined;
        return signal;
      },
    });
    const exit = { code: child.exitCode, signal: child.signalCode };
    const code =
      !cleanup.confirmed || spawnError
        ? 1
        : cancellation
          ? signalExitCode(cancellation)
          : (exit.code ?? signalExitCode(exit.signal));
    if (!cleanup.confirmed)
      console.error(
        "[desktop dev] process group cleanup unconfirmed: " +
          JSON.stringify({ pid: child.pid, exit, cancellation, cleanup }),
      );
    return {
      code,
      exit,
      cancellation,
      cleanup,
      ...(spawnError && { error: spawnError }),
    };
  } finally {
    for (const [signal, handler] of handlers) signals.off(signal, handler);
    child.off("exit", onExit);
    child.off("error", onError);
  }
}

const signalExitCode = (signal) =>
  ({ SIGINT: 130, SIGTERM: 143, SIGHUP: 129 })[signal] ?? 1;

// The controlled worker gets owner-first grace. Preparation stops its group
// immediately. Both paths share signal/probe semantics and one monotonic budget.
// ref: Node.js v24.14.1 lib/internal/child_process.js (exit is not group termination).
export async function reapOwnedProcessGroup(
  child,
  {
    graceMs = 120000,
    terminateMs = 5000,
    killMs = 1500,
    groupSignal,
    takeSignal = () => undefined,
  } = {},
  {
    kill = process.kill,
    now = () => performance.now(),
    delay = waitMilliseconds,
  } = {},
) {
  const exited = () =>
    !child.pid || child.exitCode !== null || child.signalCode !== null;
  let forced = false,
    confirmed = false,
    error;
  const alive = () => {
    if (!child.pid) return false;
    try {
      kill(-child.pid, 0);
      return true;
    } catch (failure) {
      if (failure.code === "ESRCH") return false;
      // Permission denial and failed observation cannot prove absence.
      error ??= failure.code ?? "probe_failed";
      return true;
    }
  };
  const send = (signal, group) => {
    if (!child.pid) return;
    try {
      kill(group ? -child.pid : child.pid, signal);
    } catch (failure) {
      if (failure.code !== "ESRCH") error ??= failure.code ?? "signal_failed";
    }
  };
  const wait = async (budget, group = false) => {
    const deadline = now() + budget;
    while ((!exited() || (group && alive())) && now() < deadline) {
      const signal = takeSignal();
      if (signal) send(signal, true);
      await delay(Math.min(25, Math.max(0, deadline - now())));
    }
  };
  try {
    if (groupSignal) {
      // The leader may already be reaped; probe before sending to remaining group members.
      if (!exited() || alive()) {
        send(groupSignal, true);
        await wait(terminateMs, true);
      }
    } else {
      await wait(graceMs);
      if (!exited()) {
        forced = true;
        send("SIGTERM", false); // Let the worker run finally while its backend exists.
        await wait(terminateMs);
      }
      if (alive()) {
        forced = true;
        send("SIGTERM", true);
        await wait(terminateMs, true);
      }
    }
    if (!exited() || alive()) {
      forced = true;
      send("SIGKILL", true);
      await wait(killMs, true);
    }
    confirmed = exited() && !alive();
  } finally {
    // Unconfirmed termination must not leave pipe handles keeping the caller alive.
    child.stdin?.destroy();
    child.stdout?.destroy();
    child.stderr?.destroy();
    if (!confirmed) child.unref();
  }
  return { confirmed, forced, ...(error && { error }) };
}
