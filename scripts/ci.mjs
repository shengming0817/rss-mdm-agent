import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { sameCommittedSource } from "./source-state.mjs";
import { selectImpact, ciSourceState } from "./ci-impact.mjs";
import { steps as defaultSteps } from "./ci-steps.mjs";
import {
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
  const result = spawnSync("pnpm", ["--version"], {
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
  const start = ciSourceState(root);
  const report = {
    status: "running",
    sha: start.head,
    source: { start },
    platform: process.platform,
    arch: process.arch,
    node: process.version,
    timestamp: new Date().toISOString(),
    results: [],
  };
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
    const impact = selectImpact(root);
    report.impact = impact;
    const plan = {
      sha: start.head,
      impact,
      steps: planSteps(steps, impact),
      toolchain: report.toolchain,
      provenance: {
        selected: true,
        check:
          "Committed source HEAD, base/baseRef/baseOid and clean worktree must remain unchanged",
      },
    };
    publishPlan(root, plan, preview);
    console.log(JSON.stringify(plan, null, 2));
    if (preview) return 0;
    await executeSteps(plan.steps, root, execute, {
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
        { sha: start.head, status: report.status, error: report.error },
        true,
      );
  } finally {
    process.removeListener("SIGINT", interrupt);
    process.removeListener("SIGTERM", terminate);
    if (!preview) {
      const end = ciSourceState(root);
      report.source.end = end;
      const valid = sameCommittedSource(start, end);
      report.results.push({
        name: "committed source provenance",
        status: valid ? 0 : 1,
        outcome: valid ? "passed" : "failed",
      });
      if (!valid && report.status === "passed") report.status = "failed";
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
