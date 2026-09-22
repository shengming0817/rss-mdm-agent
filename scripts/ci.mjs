import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { sameCommittedSource } from "./source-state.mjs";
import { selectImpact, ciSourceState } from "./ci-impact.mjs";
import { steps } from "./ci-steps.mjs";
import {
  planSteps,
  executeSteps,
  prepareEvidence,
  publishPlan,
  writeReceipt,
} from "./ci-plan.mjs";
const root = fileURLToPath(new URL("../", import.meta.url));
const preview = process.env.CI_PLAN === "1";
// Invalidate prior success even when runtime/source prerequisites fail early.
if (!preview) prepareEvidence(root);
const requiredNode = JSON.parse(
  readFileSync(new URL("../package.json", import.meta.url), "utf8"),
).engines.node;
if (process.versions.node !== requiredNode) {
  console.error(
    `Node version must be ${requiredNode} for the verified SQLite runtime`,
  );
  process.exit(1);
}
const start = ciSourceState(root);
const impact = selectImpact(root);
const plan = {
  sha: start.head,
  impact,
  steps: planSteps(steps, impact),
  provenance: {
    selected: true,
    check:
      "Committed source HEAD, base/baseRef/baseOid and clean worktree must remain unchanged",
  },
};
publishPlan(root, plan, preview);
console.log(JSON.stringify(plan, null, 2));
if (preview) process.exit(0);
const results = executeSteps(plan.steps, root);
const end = ciSourceState(root);
const valid = sameCommittedSource(start, end);
results.push({
  name: "committed source provenance",
  status: valid ? 0 : 1,
  outcome: valid ? "passed" : "failed",
});
writeReceipt(root, "latest.json", {
  sha: start.head,
  source: { start, end },
  impact,
  platform: process.platform,
  arch: process.arch,
  node: process.version,
  timestamp: new Date().toISOString(),
  results,
});
console.table(
  results.map(({ name, outcome }) => ({ name, result: outcome.toUpperCase() })),
);
process.exitCode = results.some((r) => r.outcome === "failed") ? 1 : 0;
