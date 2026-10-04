// Real provider/Host integration runs in the native worker runtime.
import { preparedArtifact, withPreparation } from "./preparation.mjs";
import { runCommand } from "./ci-plan.mjs";
import { readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join } from "node:path";
const root = fileURLToPath(new URL("../", import.meta.url));
process.exitCode = await withPreparation(
  [
    "node:@rss-mdm-agent/ai-host-app",
    "node:@rss-mdm-agent/ai-client",
    "worker",
    "execution",
  ],
  async (env, signal, nested) => {
    if (process.platform !== "darwin" || process.arch !== "arm64")
      throw Error("Provider acceptance currently requires macOS arm64");
    const requiredNode = JSON.parse(readFileSync(join(root, "package.json")))
      .engines.node;
    if (process.versions.node !== requiredNode)
      throw Error(`Node version must be ${requiredNode}`);
    const files = [
      "ai-provider-conformance",
      "ai-recovery-integration",
    ].flatMap((suite) => {
      const dir = join(root, "tests", suite);
      const tests = readdirSync(dir)
        .filter((name) => name.endsWith(".test.mjs"))
        .sort();
      if (!tests.length) throw Error(`No integration tests in ${suite}`);
      return tests.map((name) => join(dir, name));
    });
    const result = await runCommand(
      join(preparedArtifact("worker"), "bin/node"),
      ["--test", "--test-concurrency=1", "--test-timeout=120000", ...files],
      {
        cwd: root,
        env,
        signal,
        stdio: "inherit",
        ...(nested ? { detached: false } : {}),
      },
    );
    if (result.error) throw result.error;
    return result.status ?? 1;
  },
);
