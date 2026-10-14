import { spawn } from "node:child_process";
import {
  fstatSync,
  lstatSync,
  realpathSync,
  mkdirSync,
  rmSync,
  writeFileSync,
  renameSync,
} from "node:fs";
import { isAbsolute, join, resolve } from "node:path";
import { stepResult } from "./ci-result.mjs";
// ref: Node.js doc/api/child_process.md@v24.14.1 (.cmd execution).
// Use pnpm's explicit JS CLI on Windows, as the existing packaging entry does.
export function resolveCommand(
  command,
  args,
  env = process.env,
  platform = process.platform,
) {
  if (platform !== "win32" || command !== "pnpm") return { command, args };
  const cli = env.npm_execpath;
  if (!cli || !/\.[cm]?js$/.test(cli))
    throw Error(
      "Run Windows build tools from pnpm so its pinned CLI is explicit.",
    );
  return { command: process.execPath, args: [cli, ...args] };
}
const npm = (name) => `@rss-mdm-agent/${name}`;
const groups = {
  "native driver authentication": ["desktop"],
  "desktop native acceptance syntax": ["desktop"],
  "desktop model fixture syntax": ["desktop"],
  "AI generated contracts": ["ai-contract"],
  "AI contract conformance": ["ai-contract"],
  "AI access conformance": ["ai-access", "ai-client", "ai-ui-bridge"],
  "AI SQLite recovery": ["ai-store-sqlite"],
  "Claude SDK adapter": ["ai-adapter-claude"],
  "Codex pinned protocol": ["ai-adapter-codex"],
  "Codex native adapter": ["ai-adapter-codex"],
  "DeepSeek Harness adapter": ["ai-adapter-deepseek"],
  "AI Host lifecycle": ["ai-host", "ai-host-app", "ai-client"],
  "AI Host local runtime": ["ai-host-app"],
  "AI provider acceptance": ["ai-host-app"],
  "frontend build": ["ui", "desktop"],
  "assistant product acceptance": ["desktop", "ai-host-app"],
  types: ["ui", "desktop"],
  components: ["ui", "desktop"],
  "self-service fixtures": ["desktop"],
};
const always = new Set(["CI runner tests", "docs and diff"]);
const anyCode = new Set([
  "Agent dependency boundary",
  "product harness tests",
  "frozen dependencies",
  "native service/process generated contracts",
  "frontend format",
  "AI access boundaries",
  "boundaries",
]);
export function planSteps(steps, impact) {
  const nodes = new Set(impact.nodePackages);
  const plan = steps.map(([name, command, original, requirements = []]) => {
    let args = [...original],
      selected;
    if (impact.full || always.has(name)) selected = true;
    else if (anyCode.has(name)) selected = impact.packages.length > 0;
    else if (name === "workspace build inputs") selected = nodes.size > 0;
    else if (name === "Windows collector pure logic")
      selected = impact.rustPackages.includes("agent-service");
    else if (name.startsWith("rust "))
      selected = impact.rustPackages.length > 0;
    else if (groups[name])
      selected = groups[name].some((p) => nodes.has(npm(p)));
    else selected = true; // New/unclassified gates must never silently disappear.
    if (!impact.full && args.includes("--workspace"))
      args.splice(
        args.indexOf("--workspace"),
        1,
        ...impact.rustPackages.flatMap((p) => ["-p", p]),
      );
    if (!impact.full && name === "workspace build inputs")
      args = [
        ...impact.nodePackages.flatMap((p) => ["--filter", `${p}...`]),
        "build",
      ];
    if (name === "workspace build inputs")
      requirements = impact.full
        ? ["node:*"]
        : impact.nodePackages.map((name) => `node:${name}`);
    return {
      name,
      command,
      args,
      selected,
      requirements: [...requirements],
      preparationOnly: ["workspace build inputs", "frontend build"].includes(
        name,
      ),
    };
  });
  const fullWorker = plan.some(
    (step) => step.selected && step.requirements.includes("worker"),
  );
  const installed = plan.some(
    (step) => step.selected && step.name === "frozen dependencies",
  );
  for (const step of plan) {
    if (fullWorker)
      step.requirements = step.requirements.map((key) =>
        key === "privateWorker" ? "worker" : key,
      );
    step.dependencies =
      installed &&
      step.selected &&
      (step.requirements.length || step.command === "pnpm") &&
      step.name !== "frozen dependencies"
        ? ["frozen dependencies"]
        : [];
  }
  return plan;
}

function leaseFds(env, cwd, stdio) {
  if (env?._AGENT_BUILD_LEASE === undefined) return null;
  try {
    const lease = JSON.parse(env._AGENT_BUILD_LEASE);
    if (
      !Array.isArray(lease.fds) ||
      lease.fds.length !== 2 ||
      new Set(lease.fds).size !== 2 ||
      lease.fds.some((fd) => !Number.isSafeInteger(fd) || fd < 3) ||
      !Array.isArray(lease.locks) ||
      lease.locks.length !== 2 ||
      lease.locks.some(
        (path) => typeof path !== "string" || !isAbsolute(path),
      ) ||
      typeof lease.worktree !== "string" ||
      !isAbsolute(lease.worktree) ||
      typeof lease.target !== "string" ||
      !isAbsolute(lease.target) ||
      resolve(env.CARGO_TARGET_DIR) !== lease.target ||
      realpathSync(cwd ?? process.cwd()) !== lease.worktree ||
      stdio !== "inherit"
    )
      throw Error("lease paths, descriptors or stdio do not match this run");
    for (let index = 0; index < 2; index++) {
      const held = fstatSync(lease.fds[index], { bigint: true });
      const lock = lstatSync(lease.locks[index], { bigint: true });
      if (
        !held.isFile() ||
        !lock.isFile() ||
        held.dev !== lock.dev ||
        held.ino !== lock.ino
      )
        throw Error("descriptor does not identify its lock file");
    }
    return lease.fds;
  } catch (error) {
    throw Error(`invalid build lease: ${error.message}`);
  }
}

// ref: Node.js v24.14.1 lib/internal/child_process.js (numeric stdio descriptors).
export function runCommand(
  command,
  args,
  { signal, capture = false, ...options },
) {
  return new Promise((resolve) => {
    if (signal?.aborted) {
      resolve({ status: null, signal: signal.reason });
      return;
    }
    const childEnv = options.env ?? process.env;
    const invocation = resolveCommand(command, args, childEnv);
    const fds = leaseFds(childEnv, options.cwd, options.stdio);
    const managed = fds !== null;
    let stdio = managed
      ? Array.from({ length: Math.max(...fds) + 1 }, (_, index) =>
          index < 3 ? "inherit" : "ignore",
        )
      : options.stdio;
    if (managed) for (const fd of fds) stdio[fd] = fd;
    if (capture) {
      stdio = Array.isArray(stdio) ? [...stdio] : ["ignore", "pipe", "inherit"];
      stdio[1] = "pipe";
    }
    const child = spawn(invocation.command, invocation.args, {
      ...options,
      env: childEnv,
      stdio,
      detached: options.detached ?? (process.platform !== "win32" && !managed),
    });
    let failure, timer;
    let stdout = "";
    if (capture)
      child.stdout.on("data", (bytes) => {
        stdout += bytes;
      });
    const stop = (kind) => {
      if (!child.pid) return;
      try {
        if (
          process.platform === "win32" ||
          managed ||
          options.detached === false
        )
          child.kill(kind);
        else process.kill(-child.pid, kind);
      } catch (error) {
        if (error.code !== "ESRCH") failure = error;
      }
    };
    const cancel = () => {
      stop("SIGTERM");
      timer ??= setTimeout(() => stop("SIGKILL"), 5000);
    };
    signal?.addEventListener("abort", cancel, { once: true });
    if (signal?.aborted) cancel();
    child.once("spawn", () => {
      if (signal?.aborted) cancel();
    });
    child.once("error", (error) => {
      failure = error;
    });
    child.once("close", (status, exitSignal) => {
      if (signal?.aborted) stop("SIGKILL");
      clearTimeout(timer);
      signal?.removeEventListener("abort", cancel);
      resolve({
        status,
        signal: exitSignal,
        error: failure,
        ...(capture ? { stdout } : {}),
      });
    });
  });
}

export async function executeSteps(
  plan,
  root,
  execute = runCommand,
  { signal, env = process.env, onStart = () => {}, onResult = () => {} } = {},
) {
  const results = [];
  const record = (result) => {
    results.push(result);
    onResult(result);
  };
  for (const {
    name,
    command,
    args,
    selected,
    requirements = [],
    dependencies = [],
    preparationOnly = false,
  } of plan) {
    const blocked = dependencies.find(
      (dependency) =>
        results.find((result) => result.name === dependency)?.outcome !==
        "passed",
    );
    if (!selected || signal?.aborted || blocked) {
      console.log(`[ci] ${name}: SKIP`);
      record({
        name,
        command: [command, ...args],
        status: null,
        outcome: "skipped",
        reason: signal?.aborted
          ? "cancelled"
          : blocked
            ? `prerequisite:${blocked}`
            : "unaffected",
      });
      continue;
    }
    console.log(`\n[ci] ${name}`);
    onStart(name);
    let result;
    try {
      result = await execute(command, args, {
        cwd: root,
        stdio: "inherit",
        env,
        signal,
        requirements,
        preparationOnly,
      });
    } catch (error) {
      result = { status: null, error };
    }
    record({
      ...stepResult(
        name,
        result.execution === "preparation" ? [] : [command, ...args],
        result,
      ),
      ...(result.execution
        ? { execution: result.execution, preparedInputs: result.preparedInputs }
        : {}),
      ...(result.blocked ? { reason: result.blocked } : {}),
      outcome: result.blocked
        ? "skipped"
        : signal?.aborted
          ? "cancelled"
          : result.status === 0
            ? "passed"
            : "failed",
    });
  }
  return results;
}

// Explicitly owned disposable CI evidence; native/credential/provider smoke
// receipts and development runtimes have separate owners and must be preserved.
const evidence = [
  "latest.json",
  "selection.json",
  "ai-host-runtime",
  "settings-480.png",
];
export function prepareEvidence(root) {
  const out = join(root, ".local-ci-runs");
  mkdirSync(out, { recursive: true });
  for (const name of evidence)
    rmSync(join(out, name), { recursive: true, force: true });
}
export function publishPlan(root, plan, preview = false) {
  writeReceipt(root, preview ? "plan.json" : "selection.json", plan);
}
export function writeReceipt(root, name, value) {
  const out = join(root, ".local-ci-runs");
  mkdirSync(out, { recursive: true });
  const temporary = join(out, `${name}.${process.pid}.tmp`);
  try {
    writeFileSync(temporary, JSON.stringify(value, null, 2) + "\n");
    renameSync(temporary, join(out, name));
  } finally {
    rmSync(temporary, { force: true });
  }
}
