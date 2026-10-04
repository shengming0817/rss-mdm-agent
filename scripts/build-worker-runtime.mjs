import {
  cpSync,
  copyFileSync,
  chmodSync,
  mkdirSync,
  rmSync,
  writeFileSync,
  symlinkSync,
} from "node:fs";
import { resolve, join, dirname } from "node:path";
import { stageWorkerRuntime } from "./ai-host-artifacts.mjs";
import {
  preparedArtifact,
  preparedContext,
  root as repository,
  withPreparation,
} from "./preparation.mjs";
const requirements = ["native"];
if (process.argv.includes("--build-host"))
  requirements.unshift(
    "node:@rss-mdm-agent/ai-host-app",
    "node:@rss-mdm-agent/ai-client",
  );
process.exitCode = await withPreparation(requirements, () => {
  const destination = process.argv
    .slice(2)
    .find((arg) => !arg.startsWith("--"));
  const root = resolve(
    destination ?? join(repository, ".local-ci-runs/worker-runtime"),
  );
  if (destination && dirname(root) !== preparedContext().directory)
    throw Error("worker destination must belong to this preparation run");
  rmSync(root, { recursive: true, force: true });
  mkdirSync(join(root, "bin"), { recursive: true });
  if (process.argv.includes("--private-only")) {
    const suffix = process.platform === "win32" ? ".exe" : "";
    copyFileSync(process.execPath, join(root, "bin", "node" + suffix));
    const helper = join(root, "bin", "rss-private-storage" + suffix);
    copyFileSync(preparedArtifact("rss-private-storage"), helper);
    if (process.platform !== "win32") chmodSync(helper, 0o755);
  } else {
    cpSync(join(repository, "packages/ai-host/dist"), join(root, "worker"), {
      recursive: true,
    });
    writeFileSync(join(root, "package.json"), '{"type":"module"}\n');
    symlinkSync(
      join(repository, "packages/ai-host/node_modules"),
      join(root, "node_modules"),
      process.platform === "win32" ? "junction" : "dir",
    );
    stageWorkerRuntime(
      repository,
      root,
      process.execPath,
      "worker/bootstrap.js",
    );
  }

  return 0;
});
