// ref: Node.js lib/child_process.js@v24.14.1
import { spawn } from "node:child_process";
import { join } from "node:path";

// Tauri, Vite and the app share a dedicated group, including when an IDE only
// signals this wrapper. Reap the leader and bound cleanup of remaining children.
export function runDesktop(root, directory, args = []) {
  return new Promise((resolve, reject) => {
    const child = spawn("pnpm", ["exec", "tauri", "dev", ...args], {
      cwd: join(root, "apps/desktop"),
      stdio: "inherit",
      detached: true,
      env: { ...process.env, RSS_AI_HOST_RUNTIME: directory },
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
    process.on("SIGINT", interrupt);
    process.on("SIGTERM", terminate);
    child.once("error", (error) => {
      clearInterval(timer);
      process.off("SIGINT", interrupt);
      process.off("SIGTERM", terminate);
      reject(error);
    });
    child.once("exit", (code, signal) => {
      exitCode =
        code ?? (signal === "SIGINT" ? 130 : signal === "SIGTERM" ? 143 : 1);
      stop("SIGTERM");
    });
  });
}
