import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
const root = fileURLToPath(new URL("../", import.meta.url));
const steps = [
  ["frozen dependencies", "pnpm", ["install", "--frozen-lockfile"]],
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
  results.push({
    name,
    command: [command, ...args],
    status: result.status,
    error: result.error?.message,
  });
}
const sha = spawnSync(
  process.platform === "win32" ? "git" : "/usr/bin/git",
  ["rev-parse", "HEAD"],
  { cwd: root, encoding: "utf8" },
).stdout.trim();
mkdirSync(new URL("../.local-ci-runs/", import.meta.url), { recursive: true });
writeFileSync(
  new URL("../.local-ci-runs/latest.json", import.meta.url),
  JSON.stringify(
    {
      sha,
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
