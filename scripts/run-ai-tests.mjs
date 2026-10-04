// The existing AI test runner owns preparation; packages retain their own builds.
import { existsSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { testOwners } from "./ci-impact.mjs";
import { withPreparation, preparedArtifact, root } from "./preparation.mjs";
import { runCommand } from "./ci-plan.mjs";

const directories = process.argv.slice(2).filter((arg) => arg !== "--node");
const privateRuntime = !process.argv.includes("--node");
if (!directories.length) throw Error("AI test directories required");
const packages = testOwners
  .filter(
    ([directory, owner]) =>
      owner.startsWith("@rss-mdm-agent/") && directories.includes(directory),
  )
  .map(([, owner]) => `node:${owner}`);
if (!packages.length) throw Error("AI test owner required");
const requirements = [...new Set(packages)];
const host = directories.includes("tests/ai-host");
if (privateRuntime) requirements.push(host ? "worker" : "privateWorker");
if (host) requirements.push("execution", "hostFixture");
const files = directories.flatMap((directory) =>
  readdirSync(resolve(root, directory))
    .filter((name) => name.endsWith(".test.mjs"))
    .sort()
    .map((name) => join(directory, name)),
);
if (!files.length) throw Error("AI tests required");

process.exitCode = await withPreparation(
  requirements,
  async (env, signal, nested) => {
    for (const directory of directories) {
      const types = join(directory, "protocol.types.ts");
      if (!existsSync(resolve(root, types))) continue;
      const result = await runCommand(
        "pnpm",
        [
          "exec",
          "tsc",
          "--noEmit",
          "--strict",
          "--skipLibCheck",
          "--module",
          "NodeNext",
          "--target",
          "ES2022",
          types,
        ],
        {
          cwd: root,
          env,
          signal,
          stdio: "inherit",
          ...(nested ? { detached: false } : {}),
        },
      );
      if (result.status !== 0) return result.status ?? 1;
    }
    const result = await runCommand(
      privateRuntime
        ? join(
            preparedArtifact("privateWorker"),
            "bin",
            process.platform === "win32" ? "node.exe" : "node",
          )
        : process.execPath,
      ["--test", ...files],
      {
        cwd: root,
        env,
        signal,
        stdio: "inherit",
        ...(nested ? { detached: false } : {}),
      },
    );
    return result.status ?? 1;
  },
);
