import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { stepResult } from "./ci-result.mjs";
import { sameCommittedSource, sourceState } from "./source-state.mjs";
const root = fileURLToPath(new URL("../", import.meta.url));
const start = sourceState(root);
const steps = [
  [
    "CI runner tests",
    "node",
    [
      "--test",
      "scripts/source-state.test.mjs",
      "scripts/ci-result.test.mjs",
      "scripts/rust-consumers.test.mjs",
      "scripts/execution-evolution.test.mjs",
    ],
  ],
  ["frozen dependencies", "pnpm", ["install", "--frozen-lockfile"]],
  ["AI generated contracts", "pnpm", ["check:ai-contract"]],
  ["AI contract conformance", "pnpm", ["test:ai-contract"]],
  ["AI packed consumer", "pnpm", ["check:ai-consumer"]],
  ["Claude SDK adapter", "pnpm", ["test:ai-claude"]],
  ["Claude packed consumer", "pnpm", ["check:claude-consumer"]],
  ["frontend build", "pnpm", ["build"]],
  ["types", "pnpm", ["typecheck"]],
  ["frontend format", "pnpm", ["format:check"]],
  ["components", "pnpm", ["test"]],
  ["boundaries", "pnpm", ["check:boundaries"]],
  ["packed consumer", "pnpm", ["check:consumer"]],
  ["docs and diff", "node", ["scripts/check-docs.mjs"]],
  ["rust fmt", "cargo", ["fmt", "--all", "--", "--check"]],
  ["rust build", "cargo", ["build", "--workspace", "--locked"]],
  ["rust test", "cargo", ["test", "--workspace", "--locked"]],
  ["self-service fixtures", "node", ["scripts/check-self-service.mjs"]],
  ["rust consumers", "node", ["scripts/check-rust-consumers.mjs"]],
  [
    "rust clippy",
    "cargo",
    [
      "clippy",
      "--workspace",
      "--all-targets",
      "--locked",
      "--",
      "-D",
      "warnings",
    ],
  ],
];
const results = [];
for (const [name, command, args] of steps) {
  console.log(`\n[ci] ${name}`);
  const result = spawnSync(command, args, { cwd: root, stdio: "inherit" });
  results.push(stepResult(name, [command, ...args], result));
}
const end = sourceState(root);
results.push({
  name: "committed source provenance",
  status: sameCommittedSource(start, end) ? 0 : 1,
});
mkdirSync(new URL("../.local-ci-runs/", import.meta.url), { recursive: true });
writeFileSync(
  new URL("../.local-ci-runs/latest.json", import.meta.url),
  JSON.stringify(
    {
      sha: start.head,
      source: { start, end },
      platform: process.platform,
      arch: process.arch,
      node: process.version,
      timestamp: new Date().toISOString(),
      results,
    },
    null,
    2,
  ),
);
console.table(
  results.map(({ name, status }) => ({
    name,
    result: status === 0 ? "PASS" : "FAIL",
  })),
);
process.exitCode = results.some((r) => r.status !== 0) ? 1 : 0;
