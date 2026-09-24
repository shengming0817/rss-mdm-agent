// Real provider/Host integration runs in the native worker runtime.
import { spawnSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join } from "node:path";
const root = fileURLToPath(new URL("../", import.meta.url));
if (process.platform !== "darwin" || process.arch !== "arm64")
  throw Error("Provider acceptance currently requires macOS arm64");
const requiredNode = JSON.parse(readFileSync(join(root, "package.json")))
  .engines.node;
if (process.versions.node !== requiredNode)
  throw Error(`Node version must be ${requiredNode}`);
const files = ["ai-provider-conformance", "ai-recovery-integration"].flatMap(
  (suite) => {
    const dir = join(root, "tests", suite);
    const tests = readdirSync(dir)
      .filter((name) => name.endsWith(".test.mjs"))
      .sort();
    if (!tests.length) throw Error(`No integration tests in ${suite}`);
    return tests.map((name) => join(dir, name));
  },
);
const result = spawnSync(
  join(root, ".local-ci-runs/worker-runtime/bin/node"),
  ["--test", "--test-concurrency=1", "--test-timeout=120000", ...files],
  { cwd: root, stdio: "inherit" },
);
if (result.error) throw result.error;
process.exitCode = result.status ?? 1;
