import {
  cpSync,
  copyFileSync,
  chmodSync,
  mkdirSync,
  writeFileSync,
  symlinkSync,
} from "node:fs";
import { resolve, join } from "node:path";
import { stageWorkerRuntime } from "./ai-host-artifacts.mjs";
import { preparedArtifact, withPreparation } from "./preparation.mjs";
process.exitCode = await withPreparation(["native"], () => {
  const destination = process.argv
    .slice(2)
    .find((arg) => !arg.startsWith("--"));
  const root = resolve(destination ?? ".local-ci-runs/worker-runtime");
  mkdirSync(join(root, "bin"), { recursive: true });
  if (process.argv.includes("--private-only")) {
    const suffix = process.platform === "win32" ? ".exe" : "";
    copyFileSync(process.execPath, join(root, "bin", "node" + suffix));
    const helper = join(root, "bin", "rss-private-storage" + suffix);
    copyFileSync(preparedArtifact("rss-private-storage"), helper);
    if (process.platform !== "win32") chmodSync(helper, 0o755);
  } else {
    cpSync("packages/ai-host/dist", join(root, "worker"), {
      recursive: true,
    });
    writeFileSync(join(root, "package.json"), '{"type":"module"}\n');
    symlinkSync(
      resolve("packages/ai-host/node_modules"),
      join(root, "node_modules"),
      process.platform === "win32" ? "junction" : "dir",
    );
    stageWorkerRuntime(
      resolve("."),
      root,
      process.execPath,
      "worker/bootstrap.js",
    );
  }

  return 0;
});
