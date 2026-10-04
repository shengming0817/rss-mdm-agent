import {
  cpSync,
  chmodSync,
  mkdirSync,
  writeFileSync,
  symlinkSync,
} from "node:fs";
import { resolve, join } from "node:path";
import { stageWorkerRuntime } from "./ai-host-artifacts.mjs";
import { preparedArtifact } from "./preparation.mjs";
const root = resolve(process.argv[2]);
mkdirSync(join(root, "bin"), { recursive: true });
if (process.argv.includes("--private-only")) {
  const { copyFileSync } = await import("node:fs");
  const suffix = process.platform === "win32" ? ".exe" : "";
  copyFileSync(process.execPath, join(root, "bin", "node" + suffix));
  const helper = join(root, "bin", "rss-private-storage" + suffix);
  copyFileSync(preparedArtifact("rss-private-storage"), helper);
  if (process.platform !== "win32") chmodSync(helper, 0o755);
} else {
  cpSync("packages/ai-host/dist", join(root, "worker"), { recursive: true });
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
