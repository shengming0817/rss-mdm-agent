import { spawnSync } from "node:child_process";
import { join, resolve } from "node:path";
import { readdirSync } from "node:fs";
const tests = process.argv.slice(2).flatMap((directory) =>
  readdirSync(directory)
    .filter((name) => name.endsWith(".test.mjs"))
    .map((name) => join(directory, name)),
);
if (!tests.length) throw new Error("private runtime tests required");
const binary = resolve(
  ".local-ci-runs/worker-runtime/bin",
  process.platform === "win32" ? "node.exe" : "node",
);
const result = spawnSync(binary, ["--test", ...tests], { stdio: "inherit" });
if (result.error) throw new Error("fixed private test runtime unavailable");
process.exitCode = result.status ?? 1;
