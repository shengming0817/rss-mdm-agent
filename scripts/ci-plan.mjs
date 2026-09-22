import { spawnSync } from "node:child_process";
import { mkdirSync, rmSync, writeFileSync, renameSync } from "node:fs";
import { join } from "node:path";
import { stepResult } from "./ci-result.mjs";
const npm = (name) => `@rss-mdm-agent/${name}`;
const groups = {
  "desktop native acceptance syntax": ["desktop"],
  "desktop credential acceptance syntax": ["desktop"],
  "AI generated contracts": ["ai-contract"],
  "AI contract conformance": ["ai-contract"],
  "AI packed consumer": ["ai-contract"],
  "AI access conformance": ["ai-access", "ai-client", "ai-ui-bridge"],
  "AI access browser consumer": ["ai-access", "ai-client", "ai-ui-bridge"],
  "AI SQLite recovery": ["ai-store-sqlite"],
  "AI SQLite packed consumer": ["ai-store-sqlite"],
  "Claude SDK adapter": ["ai-adapter-claude"],
  "Claude packed consumer": ["ai-adapter-claude"],
  "Codex pinned protocol": ["ai-adapter-codex"],
  "Codex native adapter": ["ai-adapter-codex"],
  "Codex packed consumer": ["ai-adapter-codex"],
  "DeepSeek Harness adapter": ["ai-adapter-deepseek"],
  "DeepSeek packed consumer": ["ai-adapter-deepseek"],
  "AI Host lifecycle": ["ai-host", "ai-host-app", "ai-client"],
  "AI Host packed consumer": ["ai-host", "ai-host-app", "ai-client"],
  "AI Host local runtime": ["ai-host-app"],
  "AI provider acceptance": ["ai-host-app"],
  "frontend build": ["ui", "desktop"],
  "assistant product acceptance": ["desktop", "ai-host-app"],
  types: ["ui", "desktop"],
  components: ["ui", "desktop"],
  "packed consumer": ["ui"],
  "self-service fixtures": ["desktop"],
};
const always = new Set(["CI runner tests", "docs and diff"]);
const anyCode = new Set([
  "product harness tests",
  "frozen dependencies",
  "frontend format",
  "AI access boundaries",
  "boundaries",
]);
export function planSteps(steps, impact) {
  const nodes = new Set(impact.nodePackages);
  return steps.map(([name, command, original]) => {
    let args = [...original],
      selected;
    if (impact.full || always.has(name)) selected = true;
    else if (anyCode.has(name)) selected = impact.packages.length > 0;
    else if (name === "workspace build inputs") selected = nodes.size > 0;
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
    return { name, command, args, selected };
  });
}

export function executeSteps(plan, root, execute = spawnSync) {
  const results = [];
  for (const { name, command, args, selected } of plan) {
    if (!selected) {
      console.log(`[ci] ${name}: SKIP`);
      results.push({
        name,
        command: [command, ...args],
        status: null,
        outcome: "skipped",
        reason: "unaffected",
      });
      continue;
    }
    console.log(`\n[ci] ${name}`);
    let result;
    try {
      result = execute(command, args, { cwd: root, stdio: "inherit" });
    } catch (error) {
      result = { status: null, error };
    }
    results.push({
      ...stepResult(name, [command, ...args], result),
      outcome: result.status === 0 ? "passed" : "failed",
    });
  }
  return results;
}

// Explicitly owned disposable CI evidence; native/credential/provider smoke
// receipts and development runtimes have separate owners and must be preserved.
const evidence = [
  "latest.json",
  "selection.json",
  "ai-consumer.json",
  "ai-store-consumer.json",
  "ai-host-consumer.json",
  "claude-consumer.json",
  "codex-consumer.json",
  "deepseek-consumer.json",
  "rust-consumers.json",
  "ai-provider-matrix.json",
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
