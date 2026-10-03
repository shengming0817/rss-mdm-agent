// ref: Node.js lib/child_process.js@v24.14.1
import { spawn, execFileSync } from "node:child_process";
import { join } from "node:path";
import { once } from "node:events";
import { createInterface } from "node:readline";

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
  const actorSource = `const {spawn}=require('node:child_process');const fs=require('node:fs');
    const control=JSON.parse(process.argv[1]);
    const child=spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{stdio:'ignore'});
    child.once('spawn',()=>{
      if(control.marker)fs.writeFileSync(control.marker,JSON.stringify({parent:process.pid,child:child.pid}),{mode:0o600,flag:'wx'});
      setTimeout(()=>console.log(JSON.stringify({pid:child.pid})),control.delay);
    });setInterval(()=>{},1000);`;
  const delay = input.actorReadinessDelayMs ?? 0;
  if (!Number.isInteger(delay) || delay < 0 || delay > 5000)
    throw Error("actor readiness delay invalid");
  const actor = spawn(
    node,
    [
      "-e",
      actorSource,
      JSON.stringify({ delay, marker: input.actorRecoveryPath }),
    ],
    {
      detached: true,
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
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
        !(await reapOwnedProcessGroup(child, 0, 500, 1500)).confirmed
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
          if (!(await reapOwnedProcessGroup(actor, 0, 500, 1500)).confirmed)
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

// Preparation shares the existing owned process-group lifecycle and live output.
export function runPreparation(command, args, cwd, environment = process.env) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd,
      stdio: "inherit",
      detached: true,
      env: environment,
    });
    let exitCode, timer, deadline;
    const alive = () => {
      try {
        process.kill(-child.pid, 0);
        return true;
      } catch (error) {
        if (error.code === "ESRCH") return false;
        throw error;
      }
    };
    const send = (signal) => {
      try {
        process.kill(-child.pid, signal);
      } catch (error) {
        if (error.code !== "ESRCH") throw error;
      }
    };
    const finish = () => {
      clearInterval(timer);
      process.off("SIGINT", interrupt);
      process.off("SIGTERM", terminate);
      process.off("SIGHUP", hangup);
      resolve(exitCode);
    };
    const stop = (signal) => {
      send(signal);
      if (timer) return;
      deadline = Date.now() + 5000;
      timer = setInterval(() => {
        if (!alive() && exitCode !== undefined) finish();
        else if (Date.now() >= deadline + 1000) {
          console.error("[desktop dev] process group cleanup unconfirmed");
          exitCode = 1;
          finish();
        } else if (Date.now() >= deadline) send("SIGKILL");
      }, 50);
    };
    const interrupt = () => stop("SIGINT");
    const terminate = () => stop("SIGTERM");
    const hangup = () => stop("SIGHUP");
    process.on("SIGINT", interrupt);
    process.on("SIGTERM", terminate);
    process.on("SIGHUP", hangup);
    child.once("error", (error) => {
      clearInterval(timer);
      process.off("SIGINT", interrupt);
      process.off("SIGTERM", terminate);
      process.off("SIGHUP", hangup);
      reject(error);
    });
    child.once("exit", (code, signal) => {
      exitCode =
        code ??
        (signal === "SIGINT"
          ? 130
          : signal === "SIGTERM"
            ? 143
            : signal === "SIGHUP"
              ? 129
              : 1);
      stop("SIGTERM");
    });
  });
}

// The controlled service worker owns its backend and private authorization channel.
// Give that owner time to clean up, then reap only its dedicated process group.
export async function reapOwnedProcessGroup(
  child,
  graceMs = 120000,
  terminateMs = 5000,
  killMs = 1500,
) {
  const { setTimeout: delay } = await import("node:timers/promises");
  const exited = () =>
    !child.pid || child.exitCode !== null || child.signalCode !== null;
  const alive = () => {
    if (!child.pid) return false;
    try {
      process.kill(-child.pid, 0);
      return true;
    } catch (error) {
      if (error.code === "ESRCH") return false;
      if (error.code === "EPERM") return true;
      throw error;
    }
  };
  const send = (signal, group) => {
    if (!child.pid) return;
    try {
      process.kill(group ? -child.pid : child.pid, signal);
    } catch (error) {
      if (error.code !== "ESRCH" && error.code !== "EPERM") throw error;
    }
  };
  const wait = async (budget, group = false) => {
    const deadline = performance.now() + budget;
    while ((!exited() || (group && alive())) && performance.now() < deadline)
      await delay(25);
  };
  let forced = false,
    confirmed = false;
  try {
    await wait(graceMs);
    if (!exited()) {
      forced = true;
      send("SIGTERM", false); // Python runs its finally while the backend still exists.
      await wait(terminateMs);
    }
    if (alive()) {
      forced = true;
      send("SIGTERM", true);
      await wait(terminateMs, true);
    }
    if (alive()) {
      send("SIGKILL", true);
      await wait(killMs, true);
    }
    confirmed = exited() && !alive();
  } finally {
    // A failed reap must not leave active pipe handles keeping this caller alive forever.
    child.stdin?.destroy();
    child.stdout?.destroy();
    child.stderr?.destroy();
    if (!confirmed) child.unref();
  }
  return { confirmed, forced };
}
