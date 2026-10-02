// ref: Node.js lib/child_process.js@v24.14.1
import { spawn } from "node:child_process";
import { join } from "node:path";

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
