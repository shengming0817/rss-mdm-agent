// ref: Node.js lib/child_process.js@v24.14.1
import { spawn, execFileSync } from "node:child_process";
import { join } from "node:path";

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

// The timeout owner receives each detached actor before allowing the fault to proceed.
export async function runOwnedScopeProbe(
  node,
  source,
  input,
  register,
  cleanup,
  signal,
  timeoutMs = 30000,
) {
  const child = spawn(
    node,
    ["--input-type=module", "-e", source, JSON.stringify(input)],
    {
      detached: true,
      stdio: ["ignore", "pipe", "pipe", "ipc"],
    },
  );
  let timer,
    stdout = "",
    stderr = "",
    registered = false;
  const stop = () => child.kill("SIGTERM");
  try {
    return await new Promise((resolve, reject) => {
      timer = setTimeout(() => {
        stop();
        reject(Error("owned scope probe deadline exceeded"));
      }, timeoutMs);
      const abort = () => {
        stop();
        reject(Error("owned scope probe cancelled"));
      };
      signal?.addEventListener("abort", abort, { once: true });
      child.once("close", (code) => {
        signal?.removeEventListener("abort", abort);
        if (code !== 0) reject(Error("owned scope probe failed: " + stderr));
        else {
          try {
            if (!registered) throw Error("scope handoff absent");
            resolve(JSON.parse(stdout));
          } catch (error) {
            reject(error);
          }
        }
      });
      child.once("error", reject);
      child.stdout.on("data", (bytes) => {
        stdout += bytes;
        if (stdout.length > 1024 * 1024) {
          stop();
          reject(Error("scope probe output bound"));
        }
      });
      child.stderr.on("data", (bytes) => {
        stderr = (stderr + bytes).slice(-65536);
      });
      child.on("message", async (proof) => {
        try {
          if (registered) throw Error("duplicate scope handoff");
          await register(proof, child.pid);
          registered = true;
          child.send({ scopeRecorded: true });
        } catch (error) {
          stop();
          reject(error);
        }
      });
      if (signal?.aborted) abort();
    });
  } finally {
    clearTimeout(timer);
    try {
      if (!(await reapOwnedProcessGroup(child, 0, 500, 1500)).confirmed)
        throw Error("owned scope executor cleanup unconfirmed");
    } finally {
      await cleanup();
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
