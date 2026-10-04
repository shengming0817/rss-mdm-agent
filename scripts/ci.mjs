import { spawnSync } from "node:child_process";
import { readFileSync, realpathSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { selectImpact } from "./ci-impact.mjs";
import { Preparation, root as sourceRoot } from "./preparation.mjs";
import { steps as defaultSteps } from "./ci-steps.mjs";
import {
  runCommand,
  resolveCommand,
  planSteps,
  executeSteps,
  prepareEvidence,
  publishPlan,
  writeReceipt,
} from "./ci-plan.mjs";

// One prerequisite owner, before any pnpm metadata/build/install command.
function toolchain(root, env, report) {
  const manifest = JSON.parse(
    readFileSync(resolve(root, "package.json"), "utf8"),
  );
  report.toolchain = {
    node: process.versions.node,
    requiredNode: manifest.engines.node,
    packageManager: manifest.packageManager,
  };
  if (process.versions.node !== manifest.engines.node)
    throw Error(`Node version must be ${manifest.engines.node}`);
  const pinned = /^pnpm@(\d+\.\d+\.\d+)$/.exec(manifest.packageManager);
  if (!pinned) throw Error("packageManager must pin an exact pnpm version");
  const invocation = resolveCommand("pnpm", ["--version"], env);
  const result = spawnSync(invocation.command, invocation.args, {
    cwd: root,
    env,
    encoding: "utf8",
    timeout: 10000,
  });
  const version = result.stdout?.trim();
  report.toolchain.pnpm = version;
  if (result.status !== 0 || version !== pinned[1])
    throw Error(
      `pnpm version must be ${pinned[1]}; received ${version || result.error?.message || "unavailable"}`,
    );
}

export async function runCI(
  root,
  {
    preview = process.env.CI_PLAN === "1",
    steps = defaultSteps,
    env = process.env,
    execute,
  } = {},
) {
  const controller = new AbortController();
  const impact = selectImpact(root);
  const report = {
    status: "running",
    sha: impact.head,
    platform: process.platform,
    arch: process.arch,
    node: process.version,
    timestamp: new Date().toISOString(),
    results: [],
  };
  let preparation;
  const publish = () => {
    if (!preview) writeReceipt(root, "latest.json", report);
  };
  const cancel = (signal) => {
    report.signal = signal;
    controller.abort(signal);
    publish();
  };
  const interrupt = () => cancel("SIGINT"),
    terminate = () => cancel("SIGTERM");
  process.on("SIGINT", interrupt);
  process.on("SIGTERM", terminate);
  try {
    if (!preview) {
      prepareEvidence(root);
      publish();
    }
    toolchain(root, env, report);
    report.results.push({
      name: "toolchain prerequisites",
      status: 0,
      outcome: "passed",
    });
    publish();
    report.impact = impact;
    const plan = {
      sha: impact.head,
      impact,
      steps: planSteps(steps, impact),
      toolchain: report.toolchain,
    };
    publishPlan(root, plan, preview);
    console.log(JSON.stringify(plan, null, 2));
    if (preview) return 0;
    // Custom runners used by dependency-free harnesses retain their injected executor.
    let executePrepared = execute;
    if (realpathSync(root) === realpathSync(sourceRoot) && !execute) {
      preparation = new Preparation(root, {
        env,
        signal: controller.signal,
        onCommand: (event) => {
          (report.preparationCommands ??= []).push(event);
          publish();
        },
        onPrepared: (event) => {
          (report.preparations ??= []).push(event);
          publish();
        },
      });
      executePrepared = async (
        command,
        args,
        { requirements, preparationOnly, ...options },
      ) => {
        try {
          await preparation.ensure(requirements);
          if (preparationOnly)
            return {
              status: 0,
              execution: "preparation",
              preparedInputs: requirements,
            };
          const result = await runCommand(command, args, {
            ...options,
            env: preparation.directory ? preparation.env : env,
          });
          preparation.assertInputs();
          return result;
        } catch (error) {
          return {
            status: error.blocked ? null : 1,
            error,
            ...(preparationOnly
              ? { execution: "preparation", preparedInputs: requirements }
              : {}),
            ...(error.blocked
              ? { blocked: `preparation:${error.blocked}` }
              : {}),
          };
        }
      };
    }
    await executeSteps(plan.steps, root, executePrepared, {
      env,
      signal: controller.signal,
      onStart: (name) => {
        report.activeGate = name;
        publish();
      },
      onResult: (result) => {
        report.results.push(result);
        delete report.activeGate;
        publish();
      },
    });
    report.status = controller.signal.aborted
      ? "cancelled"
      : report.results.some((r) => r.outcome === "failed")
        ? "failed"
        : "passed";
  } catch (error) {
    report.error = error.message;
    report.results.push({
      name: "CI runner",
      status: 1,
      outcome: "failed",
      error: error.message,
    });
    console.error(error.message);
    report.status = controller.signal.aborted ? "cancelled" : "failed";
    if (preview)
      publishPlan(
        root,
        { sha: impact.head, status: report.status, error: report.error },
        true,
      );
  } finally {
    preparation?.close();
    process.removeListener("SIGINT", interrupt);
    process.removeListener("SIGTERM", terminate);
    if (!preview) {
      report.finishedAt = new Date().toISOString();
      publish();
      console.table(
        report.results.map(({ name, outcome }) => ({
          name,
          result: outcome.toUpperCase(),
        })),
      );
    }
  }
  return report.status === "passed"
    ? 0
    : report.status === "cancelled"
      ? report.signal === "SIGINT"
        ? 130
        : 143
      : 1;
}

if (
  process.argv[1] &&
  fileURLToPath(import.meta.url) === resolve(process.argv[1])
) {
  process.exitCode = await runCI(
    fileURLToPath(new URL("../", import.meta.url)),
  );
}
